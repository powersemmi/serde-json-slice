//! Where the serializer writes.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::mem::take;

use crate::EncodeError;

/// A sink the serializer appends JSON to.
///
/// The serializer calls [`Write::write_all`] with short pieces of output, in order. A sink is a
/// generic parameter of [`to_writer`](crate::to_writer), so every call is resolved at compile
/// time.
///
/// Implemented for `&mut [u8]`, which advances past what it wrote, for `&mut W` of any sink, and
/// with the `alloc` feature for `Vec<u8>`.
///
/// # Examples
///
/// A sink that writes into a fixed buffer of the caller's own type:
///
/// ```
/// use serde_json_slice::{EncodeError, Write, to_writer};
///
/// struct Frame {
///     bytes: [u8; 32],
///     len: usize,
/// }
///
/// impl Write for Frame {
///     fn write_all(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
///         let end = self.len + bytes.len();
///         let target = self.bytes.get_mut(self.len..end).ok_or(EncodeError::BufferFull)?;
///         target.copy_from_slice(bytes);
///         self.len = end;
///         Ok(())
///     }
/// }
///
/// fn main() -> Result<(), EncodeError> {
///     let mut frame = Frame { bytes: [0; 32], len: 0 };
///     to_writer(&mut frame, &[1, 2, 3])?;
///     assert_eq!(&frame.bytes[..frame.len], b"[1,2,3]");
///     Ok(())
/// }
/// ```
pub trait Write {
    /// Appends all of `bytes`.
    ///
    /// # Errors
    ///
    /// [`EncodeError::BufferFull`] when the sink has no room for them. What the sink holds after an
    /// error is unspecified, and the serializer stops at the first error.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde_json_slice::{EncodeError, Write};
    ///
    /// fn main() -> Result<(), EncodeError> {
    ///     let mut buf = [0u8; 4];
    ///     let mut rest = &mut buf[..];
    ///     rest.write_all(b"ab")?;
    ///     assert_eq!(rest.len(), 2);
    ///     assert_eq!(rest.write_all(b"cde"), Err(EncodeError::BufferFull));
    ///     Ok(())
    /// }
    /// ```
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), EncodeError>;
}

/// Copies into the front of the slice and advances it, as `std::io::Write` does for `&mut [u8]`.
/// A piece that does not fit whole is not written at all.
impl Write for &mut [u8] {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        if bytes.len() > self.len() {
            return Err(EncodeError::BufferFull);
        }
        let (target, rest) = take(self).split_at_mut(bytes.len());
        target.copy_from_slice(bytes);
        *self = rest;
        Ok(())
    }
}

impl<Sink: Write + ?Sized> Write for &mut Sink {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        (**self).write_all(bytes)
    }
}

/// Grows the vector with `try_reserve`, so a failed allocation is [`EncodeError::BufferFull`] rather
/// than an abort.
#[cfg(feature = "alloc")]
impl Write for Vec<u8> {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        self.try_reserve(bytes.len())
            .map_err(|_| EncodeError::BufferFull)?;
        self.extend_from_slice(bytes);
        Ok(())
    }
}
