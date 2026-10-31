//! The `Vec<u8>` sink of the `alloc` feature.
#![cfg(feature = "alloc")]

use serde::Serialize;
use serde_json_slice::to_writer;

#[derive(Serialize)]
enum Event {
    Moved { x: i32, y: i32 },
}

#[test]
fn a_vec_grows_to_hold_the_output_after_what_it_held() {
    let mut out = b"> ".to_vec();
    to_writer(&mut out, &Event::Moved { x: 1, y: -2 }).unwrap();
    let mut expected = b"> ".to_vec();
    expected.extend(serde_json::to_vec(&Event::Moved { x: 1, y: -2 }).unwrap());
    assert_eq!(out, expected);
}
