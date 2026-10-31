//! The values `serde-json-core` 0.7.0 cannot serialize, each beside this crate encoding it.
//!
//! The upstream tests record its behaviour (rust-embedded-community/serde-json-core#48): it
//! panics on three of the values and writes invalid JSON for the fourth. A release of
//! `serde-json-core` that fixes them turns these tests red.

use std::collections::BTreeMap;
use std::iter::once;

use serde::ser::Error as _;
use serde::{Serialize, Serializer};
use serde_json_slice::{EncodeError, to_slice};

#[derive(Serialize)]
struct WithChar {
    mark: char,
}

#[derive(Serialize)]
enum Tuple {
    V(u8, u8),
}

struct Refuses;

impl Serialize for Refuses {
    fn serialize<S: Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        Err(S::Error::custom("no thanks"))
    }
}

fn numeric_keys() -> BTreeMap<u32, u8> {
    once((1, 2)).collect()
}

fn upstream<Value: Serialize>(value: &Value) {
    let mut buf = [0u8; 64];
    let _ = serde_json_core::to_slice(value, &mut buf);
}

fn ours<Value: Serialize>(value: &Value) -> Result<Vec<u8>, EncodeError> {
    let mut buf = [0u8; 64];
    let len = to_slice(value, &mut buf)?;
    Ok(buf[..len].to_vec())
}

#[test]
#[should_panic(expected = "internal error: entered unreachable code")]
fn upstream_panics_on_a_char() {
    upstream(&WithChar { mark: 'x' });
}

#[test]
fn a_char_encodes() {
    assert_eq!(
        ours(&WithChar { mark: 'x' }),
        Ok(br#"{"mark":"x"}"#.to_vec())
    );
}

#[test]
#[should_panic(expected = "internal error: entered unreachable code")]
fn upstream_panics_on_a_tuple_variant() {
    upstream(&Tuple::V(1, 2));
}

#[test]
fn a_tuple_variant_encodes() {
    assert_eq!(ours(&Tuple::V(1, 2)), Ok(br#"{"V":[1,2]}"#.to_vec()));
}

#[test]
fn upstream_writes_a_numeric_map_key_unquoted() {
    let mut buf = [0u8; 64];
    let len = serde_json_core::to_slice(&numeric_keys(), &mut buf).unwrap();
    assert_eq!(&buf[..len], b"{1:2}");
    assert!(
        serde_json::from_slice::<serde_json::Value>(&buf[..len]).is_err(),
        "an unquoted key is not JSON"
    );
}

#[test]
fn a_numeric_map_key_encodes() {
    assert_eq!(ours(&numeric_keys()), Ok(br#"{"1":2}"#.to_vec()));
}

#[test]
#[should_panic(expected = "internal error: entered unreachable code")]
fn upstream_panics_on_a_custom_error() {
    upstream(&Refuses);
}

#[test]
fn a_custom_error_is_returned() {
    let Err(EncodeError::Custom(message)) = ours(&Refuses) else {
        panic!("expected a custom error");
    };
    assert_eq!(message.as_str(), "no thanks");
}
