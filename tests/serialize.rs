//! The behaviours the serializer promises, one test each.

use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::iter::once;

use serde::ser::Error as _;
use serde::{Serialize, Serializer};
use serde_json_slice::{EncodeError, Serializer as JsonSerializer, to_slice, to_writer};

#[derive(Serialize)]
struct Sample {
    id: u32,
    name: String,
}

fn encode<Value: Serialize + ?Sized>(value: &Value) -> Result<Vec<u8>, EncodeError> {
    let mut buf = [0u8; 1024];
    let len = to_slice(value, &mut buf)?;
    Ok(buf[..len].to_vec())
}

#[test]
fn a_struct_encodes() {
    let value = Sample {
        id: 7,
        name: "boiler".into(),
    };
    assert_eq!(encode(&value), Ok(br#"{"id":7,"name":"boiler"}"#.to_vec()));
}

#[test]
fn a_writer_appends_after_what_the_sink_holds() {
    let mut buf = [0u8; 32];
    buf[0] = b'x';
    let mut rest = &mut buf[1..];
    to_writer(
        &mut rest,
        &Sample {
            id: 1,
            name: "a".into(),
        },
    )
    .unwrap();
    let left = rest.len();
    let written = buf.len() - left;
    assert_eq!(&buf[..written], br#"x{"id":1,"name":"a"}"#);
}

#[test]
fn a_message_larger_than_the_buffer_is_a_buffer_full_error() {
    let value = Sample {
        id: 1,
        name: "a".repeat(40),
    };
    let mut small = [0u8; 16];
    let err = to_slice(&value, &mut small).unwrap_err();
    assert_eq!(err, EncodeError::BufferFull);
    assert_eq!(err.to_string(), "the JSON output does not fit the buffer");
    let mut large = [0u8; 64];
    assert_eq!(to_slice(&value, &mut large), Ok(58));
}

#[test]
fn a_too_small_buffer_writes_nothing_past_its_end() {
    let value = Sample {
        id: 1,
        name: "a".repeat(40),
    };
    let mut backing = [0xAAu8; 64];
    let (head, tail) = backing.split_at_mut(16);
    assert_eq!(to_slice(&value, head), Err(EncodeError::BufferFull));
    assert!(tail.iter().all(|&byte| byte == 0xAA), "{tail:?}");
}

#[test]
fn an_escaped_string_is_written_escaped() {
    assert_eq!(encode("a\nb\"c"), Ok(br#""a\nb\"c""#.to_vec()));
}

#[test]
fn an_escaped_string_needs_room_for_its_escapes() {
    let mut buf = [0u8; 7];
    assert_eq!(to_slice("aaaa\n", &mut buf), Err(EncodeError::BufferFull));
    let mut buf = [0u8; 8];
    assert_eq!(to_slice("aaaa\n", &mut buf), Ok(8));
}

#[derive(Serialize)]
struct WithChar {
    mark: char,
    quote: char,
}

#[test]
fn a_char_field_is_a_string() {
    let value = WithChar {
        mark: 'é',
        quote: '"',
    };
    assert_eq!(
        encode(&value),
        Ok(r#"{"mark":"é","quote":"\""}"#.as_bytes().to_vec())
    );
}

struct Refuses;

impl Serialize for Refuses {
    fn serialize<S: Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        Err(S::Error::custom("no thanks"))
    }
}

#[test]
fn a_serialize_impl_that_fails_is_a_custom_error() {
    let err = encode(&Refuses).unwrap_err();
    let EncodeError::Custom(message) = &err else {
        panic!("expected a custom error, got {err:?}");
    };
    assert_eq!(message.as_str(), "no thanks");
    assert_eq!(err.to_string(), "no thanks");
}

#[derive(Serialize)]
enum Mode {
    Off,
    Level(u8),
    Pair(u8, i16),
    Zone { id: u8, name: String },
}

#[derive(Serialize)]
struct Rich {
    flag: bool,
    small: i8,
    big: u64,
    wide: i128,
    ratio: f64,
    text: String,
    maybe: Option<u32>,
    none: Option<u32>,
    unit: (),
    list: Vec<u16>,
    pair: (u8, String),
    modes: Vec<Mode>,
    names: BTreeMap<String, u8>,
    numbered: BTreeMap<u32, bool>,
}

#[test]
fn the_output_is_what_serde_json_writes() {
    let value = Rich {
        flag: true,
        small: -5,
        big: u64::MAX,
        wide: i128::MIN,
        ratio: 0.1,
        text: "tab\t quote\" back\\ nl\n bell\u{7} uni\u{e9}".into(),
        maybe: Some(3),
        none: None,
        unit: (),
        list: vec![1, 2, 3],
        pair: (9, "x".into()),
        modes: vec![
            Mode::Off,
            Mode::Level(2),
            Mode::Pair(1, -1),
            Mode::Zone {
                id: 4,
                name: "z".into(),
            },
        ],
        names: [("a".into(), 1), ("b".into(), 2)].into_iter().collect(),
        numbered: [(7, true), (9, false)].into_iter().collect(),
    };
    assert_eq!(encode(&value), Ok(serde_json::to_vec(&value).unwrap()));
}

#[test]
fn a_non_finite_float_is_null_and_a_composite_map_key_is_an_error() {
    assert_eq!(encode(&f64::NAN), Ok(b"null".to_vec()));
    assert_eq!(encode(&1.5_f32), Ok(b"1.5".to_vec()));
    let keyed: BTreeMap<(u8, u8), u8> = once(((1, 2), 3)).collect();
    assert_eq!(encode(&keyed), Err(EncodeError::KeyMustBeAString));
}

#[test]
fn a_map_with_numeric_keys_quotes_them() {
    let map: BTreeMap<u32, bool> = once((7, true)).collect();
    assert_eq!(encode(&map), Ok(br#"{"7":true}"#.to_vec()));
}

#[test]
fn a_non_finite_float_key_is_an_error() {
    #[derive(Serialize)]
    struct Key(f64);
    struct Map(f64);
    impl Serialize for Map {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeMap;
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry(&Key(self.0), &0)?;
            map.end()
        }
    }
    assert_eq!(encode(&Map(1.5)), Ok(br#"{"1.5":0}"#.to_vec()));
    assert_eq!(
        encode(&Map(f64::INFINITY)),
        Err(EncodeError::FloatKeyMustBeFinite)
    );
}

/// A value serialized through `collect_str`.
struct Collected<Text>(Text);

impl<Text: Display> Serialize for Collected<Text> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

#[test]
fn a_collected_string_longer_than_the_buffer_is_a_buffer_full_error() {
    let mut buf = [0u8; 8];
    assert_eq!(
        to_slice(&Collected("more than eight"), &mut buf),
        Err(EncodeError::BufferFull)
    );
}

/// A `Display` impl that fails on its own, which `core::fmt` permits.
struct FailingDisplay;

impl Display for FailingDisplay {
    fn fmt(&self, _formatter: &mut Formatter<'_>) -> fmt::Result {
        Err(fmt::Error)
    }
}

#[test]
fn a_display_impl_that_fails_is_a_custom_error() {
    let err = encode(&Collected(FailingDisplay)).unwrap_err();
    let EncodeError::Custom(message) = &err else {
        panic!("expected a custom error, got {err:?}");
    };
    assert_eq!(
        message.as_str(),
        "a Display implementation returned an error"
    );
}

#[test]
fn the_serializer_returns_its_sink_with_the_rest_of_the_buffer() {
    let mut buf = [0u8; 8];
    let mut serializer = JsonSerializer::new(&mut buf[..]);
    [1u8, 2].serialize(&mut serializer).unwrap();
    assert_eq!(
        serializer.into_inner().len(),
        3,
        "five of eight bytes are written"
    );
    assert_eq!(&buf[..5], b"[1,2]");
}
