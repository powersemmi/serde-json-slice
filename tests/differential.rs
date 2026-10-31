//! This crate's output against `serde_json`'s, value by value and on random values.

use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};

use proptest::collection::{btree_map, vec as vec_of};
use proptest::num::{f32 as any_f32, f64 as any_f64};
use proptest::option::of;
use proptest::prelude::*;
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};
use serde_json_slice::{EncodeError, to_slice};

const CAPACITY: usize = 1 << 16;

/// Serializes `value` with both crates and asserts the same bytes, or an error from both.
fn same<Value: Serialize + ?Sized>(value: &Value) {
    let mut buf = vec![0u8; CAPACITY];
    let ours = to_slice(value, &mut buf).map(|len| buf[..len].to_vec());
    let theirs = serde_json::to_vec(value);
    match (&ours, &theirs) {
        (Ok(ours), Ok(theirs)) => assert_eq!(
            String::from_utf8_lossy(ours),
            String::from_utf8_lossy(theirs),
            "the output differs from serde_json"
        ),
        (Err(ours), Err(theirs)) => assert_eq!(
            ours.to_string(),
            theirs.to_string(),
            "the error differs from serde_json"
        ),
        _ => panic!("ours {ours:?}, serde_json {theirs:?}"),
    }
}

#[derive(Serialize)]
struct Unit;

#[derive(Serialize)]
struct Newtype(u16);

#[derive(Serialize)]
struct TupleStruct(i8, &'static str, Option<u8>);

// The braces are the subject: a struct with no fields is an empty object, a unit struct is null.
#[allow(clippy::empty_structs_with_brackets)]
#[derive(Serialize)]
struct Empty {}

// The brackets are the subject: an empty tuple or struct variant is `[]` or `{}` under its tag.
#[allow(clippy::empty_enum_variants_with_brackets)]
#[derive(Serialize)]
enum Inner {
    Unit,
    Newtype(i32),
    Tuple(u8, Option<bool>),
    Struct { left: f32, right: Vec<u8> },
    EmptyTuple(),
    EmptyStruct {},
}

#[derive(Serialize)]
enum Outer {
    Wrap(Inner),
    Pair(Inner, Inner),
    Named { inner: Inner, list: Vec<Inner> },
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
enum KeyVariant {
    North,
    South,
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct KeyNewtype(u8);

#[test]
fn scalars_match() {
    same(&());
    same(&true);
    same(&false);
    same(&Unit);
    same(&Newtype(7));
    same(&TupleStruct(-1, "x", None));
    same(&Empty {});
    same(&(i8::MIN, i16::MIN, i32::MIN, i64::MIN, i128::MIN));
    same(&(i8::MAX, i16::MAX, i32::MAX, i64::MAX, i128::MAX));
    same(&(u8::MAX, u16::MAX, u32::MAX, u64::MAX, u128::MAX));
    same(&(0u8, 0i128, 0u128));
}

#[test]
fn floats_match_with_non_finite_as_null() {
    for value in [
        0.0,
        -0.0,
        1.0,
        0.1,
        -2.5e-300,
        1e300,
        f64::MIN_POSITIVE,
        5e-324,
        f64::MAX,
        f64::MIN,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        same(&value);
    }
    for value in [
        0.0f32,
        1.5,
        0.1,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::NAN,
        f32::INFINITY,
    ] {
        same(&value);
    }
}

#[test]
fn every_variant_kind_nested_matches() {
    let inners = || {
        vec![
            Inner::Unit,
            Inner::Newtype(-3),
            Inner::Tuple(1, Some(false)),
            Inner::Struct {
                left: 2.5,
                right: vec![1, 2],
            },
            Inner::EmptyTuple(),
            Inner::EmptyStruct {},
        ]
    };
    for inner in inners() {
        same(&inner);
        same(&Outer::Wrap(inner));
    }
    same(&Outer::Pair(Inner::Unit, Inner::Newtype(1)));
    same(&Outer::Named {
        inner: Inner::Tuple(0, None),
        list: inners(),
    });
    same(&Some(Outer::Wrap(Inner::Unit)));
}

#[test]
fn options_and_sequences_match() {
    same(&None::<u8>);
    same(&Some(Some(3u8)));
    same(&Some(None::<u8>));
    same(&Vec::<u8>::new());
    same(&[[1u8, 2], [3, 4]]);
    same(&vec![vec![], vec![()]]);
    same(&((), (1u8,), ("a", 'b', 2.5f64)));
}

#[test]
fn maps_with_string_and_scalar_keys_match() {
    same(&BTreeMap::<String, u8>::new());
    same(&BTreeMap::from([
        ("b".to_owned(), 1),
        ("a\"\n".to_owned(), 2),
    ]));
    same(&BTreeMap::from([(-1i64, "x"), (i64::MAX, "y")]));
    same(&BTreeMap::from([(u128::MAX, 0u8), (0, 1)]));
    same(&BTreeMap::from([(i8::MIN, 0u8), (i8::MAX, 1)]));
    same(&BTreeMap::from([(true, 1u8), (false, 0)]));
    same(&BTreeMap::from([('k', 1u8), ('"', 2)]));
    same(&BTreeMap::from([
        (KeyVariant::North, 1u8),
        (KeyVariant::South, 2),
    ]));
    same(&BTreeMap::from([(KeyNewtype(4), 1u8)]));
    same(&BTreeMap::from([(Some(3u8), 1u8)]));
}

/// A map with one key, for key types `BTreeMap` cannot hold.
struct OneKey<Key>(Key);

impl<Key: Serialize> Serialize for OneKey<Key> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.0, &0)?;
        map.end()
    }
}

#[test]
fn float_keys_match_and_keys_json_cannot_express_fail_as_in_serde_json() {
    same(&OneKey(1.5f64));
    same(&OneKey(-0.0f32));
    same(&OneKey(f64::NAN));
    same(&OneKey(f32::INFINITY));
    same(&OneKey((1u8, 2u8)));
    same(&OneKey(()));
    same(&OneKey(Unit));
    same(&OneKey(None::<u8>));
    same(&OneKey(vec![1u8]));
    same(&OneKey(BTreeMap::from([(1u8, 1u8)])));
    same(&OneKey(Inner::Newtype(1)));
    same(&OneKey(Inner::Tuple(1, None)));
    same(&OneKey(Inner::Struct {
        left: 0.0,
        right: vec![],
    }));
    same(&OneKey(Empty {}));
    same(&OneKey(TupleStruct(1, "", None)));
    same(&OneKey(Bytes(b"k")));
}

/// Serializes through `serialize_bytes`, which JSON writes as an array of numbers.
struct Bytes(&'static [u8]);

impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.0)
    }
}

/// Serializes through `collect_str`, in several `fmt` pieces.
struct Collected(&'static str);

impl Display for Collected {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        for piece in self.0.split_inclusive('|') {
            formatter.write_str(piece)?;
        }
        Ok(())
    }
}

impl Serialize for Collected {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[test]
fn bytes_and_collected_strings_match() {
    same(&Bytes(b""));
    same(&Bytes(&[0, 127, 255]));
    same(&Collected("plain|with \"quote\"|and\ttab|\u{1}"));
    same(&OneKey(Collected("key|\n|é")));
}

#[test]
fn every_ascii_character_and_unicode_matches() {
    let ascii: String = (0u8..=0x7f).map(char::from).collect();
    same(&ascii);
    for byte in 0u8..=0x7f {
        same(&char::from(byte));
    }
    same(&"é ü 漢字 \u{2028}\u{2029} \u{feff} 🦀 \u{10ffff}");
    for character in ['é', '漢', '🦀', '\u{10ffff}', '\u{7f}'] {
        same(&character);
    }
}

#[derive(Debug, Clone, Serialize)]
enum Node {
    Unit,
    Bool(bool),
    Int(i64),
    Wide(u128),
    Double(f64),
    Single(f32),
    Text(String),
    Char(char),
    Maybe(Option<Box<Self>>),
    List(Vec<Self>),
    Pair(Box<Self>, i8),
    Record { name: String, inner: Box<Self> },
    Named(BTreeMap<String, Self>),
    Numbered(BTreeMap<i32, Self>),
}

fn node() -> impl Strategy<Value = Node> {
    let leaf = prop_oneof![
        Just(Node::Unit),
        any::<bool>().prop_map(Node::Bool),
        any::<i64>().prop_map(Node::Int),
        any::<u128>().prop_map(Node::Wide),
        any_f64::ANY.prop_map(Node::Double),
        any_f32::ANY.prop_map(Node::Single),
        any::<String>().prop_map(Node::Text),
        any::<char>().prop_map(Node::Char),
    ];
    leaf.prop_recursive(4, 48, 6, |inner| {
        prop_oneof![
            of(inner.clone()).prop_map(|node| Node::Maybe(node.map(Box::new))),
            vec_of(inner.clone(), 0..6).prop_map(Node::List),
            (inner.clone(), any::<i8>())
                .prop_map(|(node, small)| Node::Pair(Box::new(node), small)),
            (any::<String>(), inner.clone()).prop_map(|(name, node)| Node::Record {
                name,
                inner: Box::new(node),
            }),
            btree_map(any::<String>(), inner.clone(), 0..4).prop_map(Node::Named),
            btree_map(any::<i32>(), inner, 0..4).prop_map(Node::Numbered),
        ]
    })
}

proptest! {
    #[test]
    fn random_values_match_serde_json(value in node()) {
        same(&value);
    }

    #[test]
    fn a_buffer_one_byte_short_fails_and_stays_inside(value in node(), cut in 1usize..64) {
        let expected = serde_json::to_vec(&value).unwrap();
        let mut backing = vec![0xAAu8; expected.len() + 64];
        let short = expected.len().saturating_sub(cut);
        let (head, tail) = backing.split_at_mut(short);
        prop_assert_eq!(to_slice(&value, head), Err(EncodeError::BufferFull));
        prop_assert!(tail.iter().all(|&byte| byte == 0xAA), "a byte past the buffer changed");
        let (exact, _) = backing.split_at_mut(expected.len());
        prop_assert_eq!(to_slice(&value, exact), Ok(expected.len()));
        prop_assert_eq!(&*exact, expected.as_slice());
    }
}
