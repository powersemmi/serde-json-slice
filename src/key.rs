//! Map keys, which JSON writes as strings.

use core::fmt::Display;

use serde::Serialize;
use serde::ser::{self, Impossible};

use crate::{EncodeError, Serializer, Write};

/// Writes a map key the way `serde_json` does: a string as it is; a number, a bool or a char as
/// quoted text; a finite float through the same formatting as a value. Anything else is refused.
pub(crate) struct KeySerializer<'ser, Sink> {
    serializer: &'ser mut Serializer<Sink>,
}

impl<'ser, Sink: Write> KeySerializer<'ser, Sink> {
    pub(crate) const fn new(serializer: &'ser mut Serializer<Sink>) -> Self {
        Self { serializer }
    }

    fn quoted(
        self,
        write: impl FnOnce(&mut Serializer<Sink>) -> Result<(), EncodeError>,
    ) -> Result<(), EncodeError> {
        self.serializer.push(b"\"")?;
        write(self.serializer)?;
        self.serializer.push(b"\"")
    }
}

macro_rules! key_integer {
    ($($method:ident($type:ty);)*) => {$(
        fn $method(self, value: $type) -> Result<(), EncodeError> {
            self.quoted(|serializer| serializer.integer(value))
        }
    )*};
}

macro_rules! key_float {
    ($($method:ident($type:ty);)*) => {$(
        fn $method(self, value: $type) -> Result<(), EncodeError> {
            if !value.is_finite() {
                return Err(EncodeError::FloatKeyMustBeFinite);
            }
            self.quoted(|serializer| serializer.finite_float(value))
        }
    )*};
}

macro_rules! key_refused {
    ($($method:ident($($arg:ty),*) -> $ret:ty;)*) => {$(
        fn $method(self, $(_: $arg),*) -> Result<$ret, EncodeError> {
            Err(EncodeError::KeyMustBeAString)
        }
    )*};
}

impl<Sink: Write> ser::Serializer for KeySerializer<'_, Sink> {
    type Ok = ();
    type Error = EncodeError;
    type SerializeSeq = Impossible<(), EncodeError>;
    type SerializeTuple = Impossible<(), EncodeError>;
    type SerializeTupleStruct = Impossible<(), EncodeError>;
    type SerializeTupleVariant = Impossible<(), EncodeError>;
    type SerializeMap = Impossible<(), EncodeError>;
    type SerializeStruct = Impossible<(), EncodeError>;
    type SerializeStructVariant = Impossible<(), EncodeError>;

    key_integer! {
        serialize_i8(i8); serialize_i16(i16); serialize_i32(i32); serialize_i64(i64);
        serialize_i128(i128); serialize_u8(u8); serialize_u16(u16); serialize_u32(u32);
        serialize_u64(u64); serialize_u128(u128);
    }

    key_float! {
        serialize_f32(f32); serialize_f64(f64);
    }

    fn serialize_bool(self, value: bool) -> Result<(), EncodeError> {
        self.quoted(|serializer| serializer.push(if value { b"true" } else { b"false" }))
    }

    fn serialize_char(self, value: char) -> Result<(), EncodeError> {
        self.serializer.string(value.encode_utf8(&mut [0; 4]))
    }

    fn serialize_str(self, value: &str) -> Result<(), EncodeError> {
        self.serializer.string(value)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<(), EncodeError> {
        self.serializer.string(variant)
    }

    fn serialize_newtype_struct<Value: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &Value,
    ) -> Result<(), EncodeError> {
        value.serialize(self)
    }

    fn serialize_some<Value: Serialize + ?Sized>(self, value: &Value) -> Result<(), EncodeError> {
        value.serialize(self)
    }

    fn collect_str<Text: Display + ?Sized>(self, value: &Text) -> Result<(), EncodeError> {
        self.serializer.display_string(value)
    }

    key_refused! {
        serialize_bytes(&[u8]) -> ();
        serialize_none() -> ();
        serialize_unit() -> ();
        serialize_unit_struct(&'static str) -> ();
        serialize_seq(Option<usize>) -> Impossible<(), EncodeError>;
        serialize_tuple(usize) -> Impossible<(), EncodeError>;
        serialize_tuple_struct(&'static str, usize) -> Impossible<(), EncodeError>;
        serialize_tuple_variant(&'static str, u32, &'static str, usize) -> Impossible<(), EncodeError>;
        serialize_map(Option<usize>) -> Impossible<(), EncodeError>;
        serialize_struct(&'static str, usize) -> Impossible<(), EncodeError>;
        serialize_struct_variant(&'static str, u32, &'static str, usize) -> Impossible<(), EncodeError>;
    }

    fn serialize_newtype_variant<Value: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &Value,
    ) -> Result<(), EncodeError> {
        Err(EncodeError::KeyMustBeAString)
    }
}
