#![doc = include_str!("../README.md")]
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod error;
mod key;
mod ser;
mod write;

use serde::Serialize;

pub use error::{CustomMessage, EncodeError};
pub use ser::{Compound, Serializer};
pub use write::Write;

/// Serializes `value` as compact JSON into `buf` and returns the number of bytes written.
///
/// The output is what `serde_json::to_vec` writes for the same value.
///
/// # Errors
///
/// - [`EncodeError::BufferFull`] when the JSON does not fit `buf`. Nothing is written past its end;
///   what `buf` holds before its end is unspecified.
/// - [`EncodeError::KeyMustBeAString`] or [`EncodeError::FloatKeyMustBeFinite`] for a map key JSON cannot
///   write, where `serde_json` refuses it too.
/// - [`EncodeError::Custom`] when the value's `Serialize` impl returns an error.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
///
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// enum Command {
///     Move(i16, i16),
///     Stop,
/// }
///
/// fn main() -> Result<(), serde_json_slice::EncodeError> {
///     let mut buf = [0u8; 64];
///     let len = serde_json_slice::to_slice(&Command::Move(3, -4), &mut buf)?;
///     assert_eq!(&buf[..len], br#"{"Move":[3,-4]}"#);
///
///     let levels: BTreeMap<u8, Command> = [(1, Command::Stop)].into_iter().collect();
///     let len = serde_json_slice::to_slice(&levels, &mut buf)?;
///     assert_eq!(&buf[..len], br#"{"1":"Stop"}"#);
///     Ok(())
/// }
/// ```
pub fn to_slice<Value: Serialize + ?Sized>(
    value: &Value,
    buf: &mut [u8],
) -> Result<usize, EncodeError> {
    let capacity = buf.len();
    let mut rest = buf;
    to_writer(&mut rest, value)?;
    Ok(capacity - rest.len())
}

/// Serializes `value` as compact JSON into `sink`.
///
/// The output is what `serde_json::to_writer` writes for the same value. Each piece of output
/// goes to the sink as it is produced, with no intermediate buffer.
///
/// # Errors
///
/// - [`EncodeError::BufferFull`] when the sink has no room left.
/// - [`EncodeError::KeyMustBeAString`] or [`EncodeError::FloatKeyMustBeFinite`] for a map key JSON cannot
///   write, where `serde_json` refuses it too.
/// - [`EncodeError::Custom`] when the value's `Serialize` impl returns an error.
///
/// # Examples
///
/// ```
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Reading {
///     sensor: char,
///     celsius: f32,
/// }
///
/// fn main() -> Result<(), serde_json_slice::EncodeError> {
///     let mut frame = [0u8; 64];
///     let mut rest = &mut frame[..];
///     serde_json_slice::to_writer(&mut rest, &Reading { sensor: 'A', celsius: 21.5 })?;
///     let len = 64 - rest.len();
///     assert_eq!(&frame[..len], br#"{"sensor":"A","celsius":21.5}"#);
///     Ok(())
/// }
/// ```
pub fn to_writer<Sink: Write, Value: Serialize + ?Sized>(
    sink: Sink,
    value: &Value,
) -> Result<(), EncodeError> {
    value.serialize(&mut Serializer::new(sink))
}
