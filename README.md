# serde-json-slice

A `no_std`, no-alloc JSON serializer for [serde] that writes into a caller's buffer and never
panics. Every value either encodes or returns an error.

```rust
use serde::Serialize;

#[derive(Serialize)]
struct Reading {
    sensor: char,
    celsius: f32,
}

fn main() -> Result<(), serde_json_slice::Error> {
    let mut buf = [0u8; 64];
    let len = serde_json_slice::to_slice(&Reading { sensor: 'A', celsius: 21.5 }, &mut buf)?;
    assert_eq!(&buf[..len], br#"{"sensor":"A","celsius":21.5}"#);
    Ok(())
}
```

## Why

[`serde-json-core`] 0.7.0, the usual choice for JSON without a heap, reaches `unreachable!()` and
panics on ordinary values: a `char`, a tuple variant such as `V(u8, u8)`, a map whose keys are not
strings (`BTreeMap<u32, _>`), and a `Serialize` impl that returns `Error::custom`. The issue is
[rust-embedded-community/serde-json-core#48][issue], open since 2021. On a device a panic is a
reset, so this crate serializes those values, or returns an error where `serde_json` returns one.

## Guarantees

- No panic. A buffer that is too small, a map key JSON cannot express and an error raised by the
  value's own `Serialize` impl each return an [`Error`]. Nothing is written past the buffer.
- No allocation. The crate is `#![no_std]` and uses neither `alloc` nor `std` by default.
- The same bytes as `serde_json::to_vec` for every value `serde_json` serializes: a `char` as a
  string, enums externally tagged (`{"V":[1,2]}`), integer, bool and float map keys quoted,
  non-finite floats as `null`, and the same string escapes. The test suite checks this
  differentially against `serde_json`, including on random values.
- No `unsafe` code (`#![forbid(unsafe_code)]`).

## Writing into your own buffer

[`to_writer`] serializes into any [`Write`] sink, so the output lands in the caller's buffer type
without a copy through a stack buffer. The crate implements `Write` for `&mut [u8]` (the slice
advances past what was written) and, with the `alloc` feature, for `Vec<u8>`. A sink is a generic
parameter, so each call is monomorphized: no `dyn` call and no allocation beyond what the sink
itself does.

## Features

- `alloc` (off by default): implements `Write` for `Vec<u8>`.

## MSRV

Rust 1.95. The minimum supported version rises only in a minor release.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.

[serde]: https://serde.rs
[`serde-json-core`]: https://crates.io/crates/serde-json-core
[issue]: https://github.com/rust-embedded-community/serde-json-core/issues/48
[`Error`]: https://docs.rs/serde-json-slice/latest/serde_json_slice/enum.Error.html
[`to_writer`]: https://docs.rs/serde-json-slice/latest/serde_json_slice/fn.to_writer.html
[`Write`]: https://docs.rs/serde-json-slice/latest/serde_json_slice/trait.Write.html
