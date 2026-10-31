//! A `no_std`, no-alloc JSON serializer for serde that writes into a caller's buffer and never
//! panics.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
