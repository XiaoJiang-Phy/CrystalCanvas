// [Overview: Internal header exposing C++ native API for unit testing without polluting FFI]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
#pragma once

#include <Eigen/Dense>

/// Compute the surface-oriented transformation matrix P.
/// col 0, 1 = in-plane (shortest lattice vectors with G . v = 0)
/// col 2 = out-of-plane (via Extended Euclidean, inclination-optimised)
///
/// @param lattice 3x3 ColMajor lattice matrix
/// @param h Miller index h
/// @param k Miller index k
/// @param l Miller index l
/// @return Integer transformation matrix P with det(P) = 1
[[nodiscard]] Eigen::Matrix3i get_surface_basis(
    const Eigen::Ref<const Eigen::Matrix3d>& lattice,
    int h, int k, int l);
