// Thin C++ wrapper for Gemmi CIF parsing — declares the FFI function for cxx bridge
// Copyright (c) 2026 CrystalCanvas Contributors. MIT OR Apache-2.0.
//
// NOTE: FfiAtomSite and FfiCrystalData structs are defined by cxx code generation
// from bridge.rs. This header only declares the function signature.

#pragma once
#include "crystal-canvas/src/ffi/bridge.rs.h"

// Parse a CIF file at the given path and return crystal data.
// Throws std::runtime_error on failure (converted to rust::Error by cxx).
FfiCrystalData parse_cif_file(rust::Str path);
