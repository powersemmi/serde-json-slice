//! The JSON serializer.

use core::fmt::{self, Display, Write as _};
use core::mem::take;

use serde::Serialize;
use serde::ser::{
    self, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant,
};

use crate::key::KeySerializer;
use crate::{EncodeError, Write};

/// A serde [`Serializer`](ser::Serializer) that writes compact JSON into a [`Write`] sink.
///
/// [`to_slice`](crate::to_slice) and [`to_writer`](crate::to_writer) cover the usual cases; the
/// serializer itself is for code that drives a serde serializer directly.
///
/// # Examples
///
/// ```
/// use serde::Serialize;
/// use serde_json_slice::{EncodeError, Serializer};
///
/// #[derive(Serialize)]
/// struct Reading {
///     channel: u8,
///     volts: f32,
/// }
///
/// fn main() -> Result<(), EncodeError> {
///     let mut buf = [0u8; 32];
///     let mut serializer = Serializer::new(&mut buf[..]);
///     Reading { channel: 2, volts: 3.3 }.serialize(&mut serializer)?;
///     let left = serializer.into_inner().len();
///     let written = buf.len() - left;
///     assert_eq!(&buf[..written], br#"{"channel":2,"volts":3.3}"#);
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct Serializer<Sink> {
    sink: Sink,
}

impl<Sink: Write> Serializer<Sink> {
    /// Creates a serializer that writes into `sink`.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde::Serialize;
    /// use serde_json_slice::{EncodeError, Serializer};
    ///
    /// fn main() -> Result<(), EncodeError> {
    ///     let mut buf = [0u8; 8];
    ///     let mut serializer = Serializer::new(&mut buf[..]);
    ///     Some(42).serialize(&mut serializer)?;
    ///     assert_eq!(&buf[..2], b"42");
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    pub const fn new(sink: Sink) -> Self {
        Self { sink }
    }

    /// Returns the sink, holding everything written so far.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde::Serialize;
    /// use serde_json_slice::{EncodeError, Serializer};
    ///
    /// fn main() -> Result<(), EncodeError> {
    ///     let mut buf = [0u8; 8];
    ///     let mut serializer = Serializer::new(&mut buf[..]);
    ///     true.serialize(&mut serializer)?;
    ///     assert_eq!(serializer.into_inner().len(), 4, "four of eight bytes are left");
    ///     Ok(())
    /// }
    /// ```
    pub fn into_inner(self) -> Sink {
        self.sink
    }

    pub(crate) fn push(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        self.sink.write_all(bytes)
    }

    pub(crate) fn integer(&mut self, value: impl itoa::Integer) -> Result<(), EncodeError> {
        self.push(itoa::Buffer::new().format(value).as_bytes())
    }

    pub(crate) fn finite_float(&mut self, value: impl zmij::Float) -> Result<(), EncodeError> {
        self.push(zmij::Buffer::new().format_finite(value).as_bytes())
    }

    pub(crate) fn string(&mut self, text: &str) -> Result<(), EncodeError> {
        self.push(b"\"")?;
        self.string_contents(text)?;
        self.push(b"\"")
    }

    /// Writes `value`'s `Display` output as a JSON string, escaping it piece by piece.
    pub(crate) fn display_string(
        &mut self,
        value: &(impl Display + ?Sized),
    ) -> Result<(), EncodeError> {
        self.push(b"\"")?;
        let mut escaper = Escaper {
            serializer: self,
            failed: None,
        };
        if write!(escaper, "{value}").is_err() {
            return Err(escaper.failed.unwrap_or_else(|| {
                ser::Error::custom("a Display implementation returned an error")
            }));
        }
        self.push(b"\"")
    }

    /// Writes `text` escaped the way `serde_json` escapes it: the quote, the backslash, and the
    /// control characters, the common ones in their short form and the rest as `\u00XX`.
    fn string_contents(&mut self, text: &str) -> Result<(), EncodeError> {
        let bytes = text.as_bytes();
        let mut start = 0;
        for (index, &byte) in bytes.iter().enumerate() {
            let control;
            let escape: &[u8] = match byte {
                b'"' => b"\\\"",
                b'\\' => b"\\\\",
                b'\n' => b"\\n",
                b'\r' => b"\\r",
                b'\t' => b"\\t",
                0x08 => b"\\b",
                0x0c => b"\\f",
                0x00..=0x1f => {
                    control = unicode_escape(byte);
                    &control
                }
                _ => continue,
            };
            // Every escaped byte is ASCII, so `start..index` lies on character boundaries.
            self.push(bytes.get(start..index).unwrap_or_default())?;
            self.push(escape)?;
            start = index + 1;
        }
        self.push(bytes.get(start..).unwrap_or_default())
    }

    fn variant_open(&mut self, variant: &str) -> Result<(), EncodeError> {
        self.push(b"{")?;
        self.string(variant)?;
        self.push(b":")
    }
}

/// The `\u00XX` escape of a control byte, with lowercase hex digits as `serde_json` writes them.
const fn unicode_escape(byte: u8) -> [u8; 6] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    [
        b'\\',
        b'u',
        b'0',
        b'0',
        HEX[(byte >> 4) as usize],
        HEX[(byte & 0x0f) as usize],
    ]
}

/// Escapes `fmt` output into the serializer, remembering why it stopped.
struct Escaper<'ser, Sink> {
    serializer: &'ser mut Serializer<Sink>,
    failed: Option<EncodeError>,
}

impl<Sink: Write> fmt::Write for Escaper<'_, Sink> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.serializer.string_contents(text).map_err(|error| {
            self.failed = Some(error);
            fmt::Error
        })
    }
}

/// An open JSON array or object, which a `Serialize` impl fills element by element.
///
/// The [`Serializer`] hands it out from `serialize_seq`, `serialize_map`, `serialize_struct` and
/// the other compound methods; it closes the array or object on `end`.
///
/// # Examples
///
/// ```
/// use serde::ser::{SerializeSeq, Serializer as _};
/// use serde_json_slice::{EncodeError, Serializer};
///
/// fn main() -> Result<(), EncodeError> {
///     let mut buf = [0u8; 16];
///     let mut serializer = Serializer::new(&mut buf[..]);
///     let mut seq = (&mut serializer).serialize_seq(None)?;
///     seq.serialize_element("on")?;
///     seq.serialize_element(&1)?;
///     seq.end()?;
///     assert_eq!(&buf[..8], br#"["on",1]"#);
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct Compound<'ser, Sink> {
    serializer: &'ser mut Serializer<Sink>,
    first: bool,
    close: &'static [u8],
}

impl<'ser, Sink: Write> Compound<'ser, Sink> {
    fn open(
        serializer: &'ser mut Serializer<Sink>,
        open: &[u8],
        close: &'static [u8],
    ) -> Result<Self, EncodeError> {
        serializer.push(open)?;
        Ok(Self {
            serializer,
            first: true,
            close,
        })
    }

    fn separate(&mut self) -> Result<(), EncodeError> {
        if !take(&mut self.first) {
            self.serializer.push(b",")?;
        }
        Ok(())
    }

    fn element<Value: Serialize + ?Sized>(&mut self, value: &Value) -> Result<(), EncodeError> {
        self.separate()?;
        value.serialize(&mut *self.serializer)
    }

    fn field<Value: Serialize + ?Sized>(
        &mut self,
        key: &str,
        value: &Value,
    ) -> Result<(), EncodeError> {
        self.separate()?;
        self.serializer.string(key)?;
        self.serializer.push(b":")?;
        value.serialize(&mut *self.serializer)
    }

    fn finish(self) -> Result<(), EncodeError> {
        self.serializer.push(self.close)
    }
}

macro_rules! serialize_integer {
    ($($method:ident($type:ty);)*) => {$(
        fn $method(self, value: $type) -> Result<(), EncodeError> {
            self.integer(value)
        }
    )*};
}

macro_rules! serialize_float {
    ($($method:ident($type:ty);)*) => {$(
        fn $method(self, value: $type) -> Result<(), EncodeError> {
            if value.is_finite() {
                self.finite_float(value)
            } else {
                self.push(b"null")
            }
        }
    )*};
}

impl<'ser, Sink: Write> ser::Serializer for &'ser mut Serializer<Sink> {
    type Ok = ();
    type Error = EncodeError;
    type SerializeSeq = Compound<'ser, Sink>;
    type SerializeTuple = Compound<'ser, Sink>;
    type SerializeTupleStruct = Compound<'ser, Sink>;
    type SerializeTupleVariant = Compound<'ser, Sink>;
    type SerializeMap = Compound<'ser, Sink>;
    type SerializeStruct = Compound<'ser, Sink>;
    type SerializeStructVariant = Compound<'ser, Sink>;

    serialize_integer! {
        serialize_i8(i8); serialize_i16(i16); serialize_i32(i32); serialize_i64(i64);
        serialize_i128(i128); serialize_u8(u8); serialize_u16(u16); serialize_u32(u32);
        serialize_u64(u64); serialize_u128(u128);
    }

    serialize_float! {
        serialize_f32(f32); serialize_f64(f64);
    }

    fn serialize_bool(self, value: bool) -> Result<(), EncodeError> {
        self.push(if value { b"true" } else { b"false" })
    }

    fn serialize_char(self, value: char) -> Result<(), EncodeError> {
        self.string(value.encode_utf8(&mut [0; 4]))
    }

    fn serialize_str(self, value: &str) -> Result<(), EncodeError> {
        self.string(value)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<(), EncodeError> {
        let mut seq = Compound::open(self, b"[", b"]")?;
        for byte in value {
            seq.element(byte)?;
        }
        seq.finish()
    }

    fn serialize_none(self) -> Result<(), EncodeError> {
        self.push(b"null")
    }

    fn serialize_some<Value: Serialize + ?Sized>(self, value: &Value) -> Result<(), EncodeError> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), EncodeError> {
        self.push(b"null")
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), EncodeError> {
        self.push(b"null")
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<(), EncodeError> {
        self.string(variant)
    }

    fn serialize_newtype_struct<Value: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &Value,
    ) -> Result<(), EncodeError> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<Value: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &Value,
    ) -> Result<(), EncodeError> {
        self.variant_open(variant)?;
        value.serialize(&mut *self)?;
        self.push(b"}")
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Compound<'ser, Sink>, EncodeError> {
        Compound::open(self, b"[", b"]")
    }

    fn serialize_tuple(self, _len: usize) -> Result<Compound<'ser, Sink>, EncodeError> {
        Compound::open(self, b"[", b"]")
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Compound<'ser, Sink>, EncodeError> {
        Compound::open(self, b"[", b"]")
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Compound<'ser, Sink>, EncodeError> {
        self.variant_open(variant)?;
        Compound::open(self, b"[", b"]}")
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Compound<'ser, Sink>, EncodeError> {
        Compound::open(self, b"{", b"}")
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Compound<'ser, Sink>, EncodeError> {
        Compound::open(self, b"{", b"}")
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Compound<'ser, Sink>, EncodeError> {
        self.variant_open(variant)?;
        Compound::open(self, b"{", b"}}")
    }

    fn collect_str<Text: Display + ?Sized>(self, value: &Text) -> Result<(), EncodeError> {
        self.display_string(value)
    }
}

macro_rules! sequence_impl {
    ($($trait:ident::$method:ident;)*) => {$(
        impl<Sink: Write> $trait for Compound<'_, Sink> {
            type Ok = ();
            type Error = EncodeError;

            fn $method<Value: Serialize + ?Sized>(&mut self, value: &Value) -> Result<(), EncodeError> {
                self.element(value)
            }

            fn end(self) -> Result<(), EncodeError> {
                self.finish()
            }
        }
    )*};
}

sequence_impl! {
    SerializeSeq::serialize_element;
    SerializeTuple::serialize_element;
    SerializeTupleStruct::serialize_field;
    SerializeTupleVariant::serialize_field;
}

macro_rules! struct_impl {
    ($($trait:ident;)*) => {$(
        impl<Sink: Write> $trait for Compound<'_, Sink> {
            type Ok = ();
            type Error = EncodeError;

            fn serialize_field<Value: Serialize + ?Sized>(
                &mut self,
                key: &'static str,
                value: &Value,
            ) -> Result<(), EncodeError> {
                self.field(key, value)
            }

            fn end(self) -> Result<(), EncodeError> {
                self.finish()
            }
        }
    )*};
}

struct_impl! {
    SerializeStruct;
    SerializeStructVariant;
}

impl<Sink: Write> SerializeMap for Compound<'_, Sink> {
    type Ok = ();
    type Error = EncodeError;

    fn serialize_key<Key: Serialize + ?Sized>(&mut self, key: &Key) -> Result<(), EncodeError> {
        self.separate()?;
        key.serialize(KeySerializer::new(&mut *self.serializer))
    }

    fn serialize_value<Value: Serialize + ?Sized>(
        &mut self,
        value: &Value,
    ) -> Result<(), EncodeError> {
        self.serializer.push(b":")?;
        value.serialize(&mut *self.serializer)
    }

    fn end(self) -> Result<(), EncodeError> {
        self.finish()
    }
}
