//! FFI module — re-exports the cxx bridge for C++ interop
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod bridge;
pub use bridge::ffi::*;
