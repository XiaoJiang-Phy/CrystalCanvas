//! FFI module — re-exports the cxx bridge for C++ interop

pub mod bridge;
pub use bridge::ffi::*;
