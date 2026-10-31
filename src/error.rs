//! Why a value did not serialize.

use core::error;
use core::fmt::{self, Display, Formatter, Write};
use core::str;

use serde::ser;

/// Why a value did not serialize.
///
/// # Examples
///
/// ```
/// use serde_json_slice::{EncodeError, to_slice};
///
/// let mut buf = [0u8; 4];
/// match to_slice("too long for four bytes", &mut buf) {
///     Err(EncodeError::BufferFull) => {}
///     other => panic!("expected a full buffer, got {other:?}"),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodeError {
    /// The output does not fit the buffer or the sink.
    BufferFull,
    /// A map key is neither a string, a number, a bool, a char nor a unit variant.
    KeyMustBeAString,
    /// A float map key is NaN or infinite.
    FloatKeyMustBeFinite,
    /// The value's own `Serialize` impl returned an error, or a `Display` impl it passed to
    /// `collect_str` failed.
    Custom(CustomMessage),
}

impl Display for EncodeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferFull => formatter.write_str("the JSON output does not fit the buffer"),
            Self::KeyMustBeAString => formatter.write_str("key must be a string"),
            Self::FloatKeyMustBeFinite => {
                formatter.write_str("float key must be finite (got NaN or +/-inf)")
            }
            Self::Custom(message) => formatter.write_str(message.as_str()),
        }
    }
}

impl error::Error for EncodeError {}

impl ser::Error for EncodeError {
    fn custom<Message: Display>(message: Message) -> Self {
        Self::Custom(CustomMessage::new(&message))
    }
}

/// The text of a custom error, kept inline without allocation.
///
/// It holds the first [`CustomMessage::CAPACITY`] bytes of the message, cut at a character
/// boundary; a longer message is truncated.
///
/// # Examples
///
/// ```
/// use serde::ser::Error as _;
/// use serde::{Serialize, Serializer};
/// use serde_json_slice::{EncodeError, to_slice};
///
/// struct Unsupported;
///
/// impl Serialize for Unsupported {
///     fn serialize<S: Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
///         Err(S::Error::custom("this value has no JSON form"))
///     }
/// }
///
/// let mut buf = [0u8; 16];
/// if let Err(EncodeError::Custom(message)) = to_slice(&Unsupported, &mut buf) {
///     assert_eq!(message.as_str(), "this value has no JSON form");
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomMessage {
    bytes: [u8; Self::CAPACITY],
    len: usize,
}

impl CustomMessage {
    /// The most bytes of a message kept; the rest is dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde::ser::Error as _;
    /// use serde_json_slice::{CustomMessage, EncodeError};
    ///
    /// let long = "x".repeat(CustomMessage::CAPACITY + 10);
    /// if let EncodeError::Custom(message) = EncodeError::custom(&long) {
    ///     assert_eq!(message.as_str().len(), CustomMessage::CAPACITY);
    /// }
    /// ```
    pub const CAPACITY: usize = 64;

    pub(crate) fn new(message: &impl Display) -> Self {
        let mut kept = Self {
            bytes: [0; Self::CAPACITY],
            len: 0,
        };
        // The writer never fails: it drops what does not fit instead.
        let _ = write!(
            Truncating {
                message: &mut kept,
                stopped: false,
            },
            "{message}"
        );
        kept
    }

    /// The kept text of the message.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde::ser::Error as _;
    /// use serde_json_slice::EncodeError;
    ///
    /// if let EncodeError::Custom(message) = EncodeError::custom("sensor offline") {
    ///     assert_eq!(message.as_str(), "sensor offline");
    /// }
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        let kept = self.bytes.get(..self.len).unwrap_or_default();
        // Only whole characters are ever copied in, so the kept bytes are always UTF-8.
        str::from_utf8(kept).unwrap_or_default()
    }
}

/// Copies whole characters into a message until the first one that does not fit, so the kept
/// text is always a prefix of the message.
struct Truncating<'message> {
    message: &'message mut CustomMessage,
    stopped: bool,
}

impl Write for Truncating<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.stopped {
            return Ok(());
        }
        let start = self.message.len;
        let room = CustomMessage::CAPACITY - start;
        let mut end = text.len().min(room);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        self.stopped = end < text.len();
        let (fits, _) = text.as_bytes().split_at(end);
        for (slot, &byte) in self.message.bytes.iter_mut().skip(start).zip(fits) {
            *slot = byte;
        }
        self.message.len = start + end;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use core::iter::repeat_n;

    use super::*;

    #[test]
    fn a_long_message_is_cut_at_a_character_boundary() {
        // 63 ASCII bytes then a two-byte character: the character does not fit whole.
        let mut text = repeat_n('a', 63).collect::<StackText>();
        text.push('é');
        let message = CustomMessage::new(&text);
        assert_eq!(message.as_str().len(), 63, "{}", message.as_str());
    }

    #[test]
    fn nothing_is_kept_after_the_first_piece_that_does_not_fit() {
        // The two-byte character does not fit after 63 bytes, but the one-byte "-" after it would.
        let mut text = repeat_n('a', 63).collect::<StackText>();
        text.push('é');
        let message = CustomMessage::new(&format_args!("{text}-"));
        assert_eq!(message.as_str().len(), 63, "{}", message.as_str());
        assert!(!message.as_str().contains('-'), "{}", message.as_str());
    }

    #[test]
    fn a_message_written_in_pieces_is_joined() {
        let message = CustomMessage::new(&format_args!("{}-{}", "left", 42));
        assert_eq!(message.as_str(), "left-42", "the pieces join in order");
    }

    /// A string on the stack, so the tests need no allocator.
    struct StackText {
        bytes: [u8; 128],
        len: usize,
    }

    impl StackText {
        fn push(&mut self, character: char) {
            let encoded = character.encode_utf8(&mut self.bytes[self.len..]).len();
            self.len += encoded;
        }
    }

    impl FromIterator<char> for StackText {
        fn from_iter<Chars: IntoIterator<Item = char>>(chars: Chars) -> Self {
            let mut text = Self {
                bytes: [0; 128],
                len: 0,
            };
            for character in chars {
                text.push(character);
            }
            text
        }
    }

    impl Display for StackText {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
            formatter.write_str(str::from_utf8(&self.bytes[..self.len]).map_err(|_| fmt::Error)?)
        }
    }
}
