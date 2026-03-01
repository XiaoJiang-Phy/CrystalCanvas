// Thin C++ wrapper for Gemmi CIF parsing — declares the FFI function for cxx bridge
// Copyright (c) 2026 CrystalCanvas Contributors. MIT OR Apache-2.0.
//
// NOTE: FfiAtomSite and FfiCrystalData structs are defined by cxx code generation
// from bridge.rs. This header only declares the function signature.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

#pragma once
#include "crystal-canvas/src/ffi/bridge.rs.h"

// Parse a CIF file at the given path and return crystal data.
// Throws std::runtime_error on failure (converted to rust::Error by cxx).
FfiCrystalData parse_cif_file(rust::Str path);

// Translate all atom positions by a uniform offset.
// Each coordinate component (x, y, z) is shifted by `offset`.
// Returns a new Vec with translated positions.
rust::Vec<FfiVec3f> translate_positions(
    rust::Vec<FfiVec3f> const& positions, float offset);
