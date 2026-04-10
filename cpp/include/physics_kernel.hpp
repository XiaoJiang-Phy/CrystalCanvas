// [Overview: Header file defining the FFI interface for physics computation
// kernels.] Physics engine thin wrapper for CXX bridge and C++ tests (Spglib
// error handling) Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
#pragma once

#include <cstddef>
#include <cstdint>

// Spglib API expects double arrays
// We expose a safe C-like function that handles exceptions/errors inside

/// Identify the spacegroup number of a given crystal.
///
/// @param lattice 3x3 matrix packed in ColMajor [9]
/// @param positions Nx3 array of fractional coordinates [n_atoms * 3]
/// @param types Array of atomic types (integers representing elements)
/// [n_atoms]
/// @param n_atoms Total number of atoms
/// @param symprec Tolerance for symmetry matching
/// @return Spacegroup number (0 if failed)
int get_spacegroup(const double *lattice, const double *positions,
                   const int *types, size_t n_atoms, double symprec);

/// Niggli-reduce a lattice in-place. Returns 0 on success, nonzero on failure.
/// @param lattice 3x3 ColMajor lattice (modified in-place)
/// @param symprec Tolerance (Å)
int niggli_reduce(double* lattice, double symprec);

/// Delaunay-reduce a lattice in-place. Returns 0 on success, nonzero on failure.
/// @param lattice 3x3 ColMajor lattice (modified in-place)
/// @param symprec Tolerance (Å)
int delaunay_reduce(double* lattice, double symprec);

/// Standardize cell (conventional or primitive).
/// @param lattice 3x3 ColMajor lattice (modified in-place)
/// @param positions Nx3 array of fractional coordinates (modified in-place)
/// @param types Array of atomic types (modified in-place)
/// @param n_atoms Total number of atoms (original count)
/// @param capacity Maximum number of atoms the positions and types arrays can hold
/// @param to_primitive If nonzero, find primitive cell; otherwise refine to conventional.
/// @param symprec Tolerance for symmetry matching
/// @return Number of atoms in standardized cell, or 0 on failure.
int standardize_cell(double* lattice, double* positions, int* types,
                     size_t n_atoms, size_t capacity, int to_primitive, double symprec);


/// Output geometry payload for supercell operations to transmit back to Rust
struct SupercellResult {
  double new_lattice[9];
  int n_atoms_new;
  // Data managed conceptually by cxx::Vec in Rust, or we can write a plain C
  // array Here we'll just let the caller provide the pre-allocated buffers
  // based on n_atoms_new But since the number of atoms grows, it's safer for
  // C++ to return a struct and ownership via cxx, or we provide a two-step API
  // (1: get size, 2: fill buffer). For simplicity with cxx, we'll design an API
  // that Rust calls to fill a mutable vector However, since this is a pure C
  // header we'll use a two-step API or memory allocated by std::vector
};

/// Get the number of atoms for a specific supercell expansion
/// @param n_atoms Original number of atoms
/// @param expansion 3x3 expansion matrix (integer)
/// @return Number of new atoms (n_atoms * determinant(expansion))
int get_supercell_size(size_t n_atoms, const int32_t *expansion);

/// Build a supercell.
/// @param lattice Input 3x3 lattice
/// @param positions Input fractional positions (n_atoms x 3)
/// @param types Input atomic types (n_atoms)
/// @param n_atoms Number of original atoms
/// @param expansion 3x3 integer expansion matrix
/// @param out_lattice Output 3x3 lattice
/// @param out_positions Output fractional positions (pre-allocated)
/// @param out_types Output array of atomic types (pre-allocated)
void build_supercell(const double *lattice, const double *positions,
                     const int *types, size_t n_atoms, const int32_t *expansion,
                     double *out_lattice, double *out_positions,
                     int *out_types);

/// Get the number of atoms for a specific slab cleavage
/// @param lattice Original 3x3 lattice
/// @param miller Miller indices (h,k,l) for the cleavage plane
/// @param layers Number of atomic layers to generate along the new c-axis
/// @param vacuum_A Vacuum thickness in Angstroms
/// @param n_atoms Original number of atoms
/// @return Number of new atoms
int get_slab_size(const double *lattice, const int32_t *miller, int layers,
                  double vacuum_A, size_t n_atoms);

/// Two-step API: first query size, then fill buffers.
/// get_slab_size_v2 returns an upper-bound atom count.
[[nodiscard]] int get_slab_size_v2(
    const double* lattice, const int32_t* miller,
    int n_layers, size_t n_atoms);

/// Build a slab by cleaving the crystal along a Miller plane.
/// @param lattice Input 3x3 lattice (ColMajor [9])
/// @param positions Input fractional positions (n_atoms x 3)
/// @param types Input atomic types (n_atoms)
/// @param n_atoms Number of original atoms
/// @param miller Miller indices [h,k,l]
/// @param layers Number of layers
/// @param vacuum_A Vacuum padding in Angstroms
/// @param out_lattice Output 3x3 lattice ([9])
/// @param out_positions Output fractional positions (pre-allocated)
/// @param out_types Output array of atomic types (pre-allocated)
void build_slab(const double *lattice, const double *positions,
                const int *types, size_t n_atoms, const int32_t *miller,
                int layers, double vacuum_A, double *out_lattice,
                double *out_positions, int *out_types);

/// Build slab with deduplication and vacuum injection.
/// @return Actual number of unique atoms written to out_positions/out_types.
[[nodiscard]] int build_slab_v2(
    const double* lattice, const double* positions,
    const int* types, size_t n_atoms,
    const int32_t* miller, int n_layers, double vacuum_a,
    double* out_lattice, double* out_positions, int* out_types);

/// Identify distinct atomic layers along the slab normal.
/// @param positions Fractional positions (n_atoms x 3, flat)
/// @param n_atoms Number of existing atoms
/// @param lattice 3x3 input lattice (ColMajor [9])
/// @param layer_tolerance_a Tolerance for grouping atoms into the same layer (Å)
/// @param out_layer_centers Pre-allocated array for layer z-coordinates (Å)
/// @param max_layers Maximum layers to store
/// @return Number of distinct layers detected
[[nodiscard]] int cluster_slab_layers(
    const double* positions, size_t n_atoms,
    const double* lattice,
    double layer_tolerance_a,
    double* out_layer_centers, size_t max_layers);

/// Shift slab termination to expose a different surface layer.
/// Modifies positions in-place.
/// @param positions Fractional positions (n_atoms x 3, flat)
/// @param n_atoms Number of existing atoms
/// @param lattice 3x3 input lattice (ColMajor [9])
/// @param target_layer_idx Target layer index to shift to Z=0
/// @param layer_centers Pre-calculated layer centers
/// @param n_layers Number of layers
void shift_slab_termination(
    double* positions, size_t n_atoms,
    const double* lattice, int target_layer_idx,
    const double* layer_centers, int n_layers);

/// Check if a new atom overlaps with existing atoms using Minimum Image
/// Convention
/// @param lattice 3x3 input lattice (ColMajor [9])
/// @param positions Input fractional positions of existing atoms (n_atoms x 3)
/// @param n_atoms Number of existing atoms
/// @param new_frac_pos Fractional position of the new atom [3]
/// @param threshold_A Overlap distance threshold in Angstroms
/// @return true if overlap is detected (distance < threshold)
bool check_overlap_mic(const double *lattice, const double *positions,
                       size_t n_atoms, const double *new_frac_pos,
                       double threshold_A);

/// Compute all chemical bonds in a structure using covalent-radius dynamic
/// thresholding. Supports Minimum Image Convention (MIC) if lattice is provided.
///
/// @param lattice 3x3 input lattice (ColMajor [9]). If nullptr, MIC is not
/// applied.
/// @param cart_positions Cartesian positions (num_atoms x 3, flat)
/// @param frac_positions Fractional positions (num_atoms x 3, flat)
/// @param covalent_radii Covalent radii per atom in Angstroms [num_atoms]
/// @param num_atoms Number of atoms
/// @param threshold_factor Scale factor for covalent sum (typically 1.2)
/// @param min_bond_length Minimum distance to form a bond (Å, avoids
/// self-bonds)
/// @param out_atom_i Output: first atom index of each bond [max_bonds]
/// @param out_atom_j Output: second atom index of each bond [max_bonds]
/// @param out_distances Output: bond distances [max_bonds]
/// @param max_bonds Maximum number of bonds to store
/// @return Number of bonds found
int compute_bonds(const double *lattice, const double *cart_positions,
                  const double *frac_positions, const double *covalent_radii,
                  size_t num_atoms, double threshold_factor,
                  double min_bond_length, int32_t *out_atom_i,
                  int32_t *out_atom_j, double *out_distances, size_t max_bonds);

/// Find all neighbors in the coordination shell of a specific center atom.
/// Supports Minimum Image Convention (MIC) if lattice is provided.
///
/// @param lattice 3x3 input lattice (ColMajor [9])
/// @param cart_positions Cartesian positions (num_atoms x 3, flat)
/// @param frac_positions Fractional positions (num_atoms x 3, flat)
/// @param covalent_radii Covalent radii per atom in Angstroms [num_atoms]
/// @param num_atoms Number of atoms
/// @param center_idx Index of the center atom
/// @param threshold_factor Scale factor for covalent sum (typically 1.2)
/// @param min_bond_length Minimum distance (Å)
/// @param out_neighbor_indices Neighbor atom indices [max_neighbors]
/// @param out_distances Distances to neighbors [max_neighbors]
/// @param max_neighbors Maximum neighbors to store
/// @return Number of neighbors found
int find_coordination_shell(const double *lattice, const double *cart_positions,
                            const double *frac_positions,
                            const double *covalent_radii, size_t num_atoms,
                            size_t center_idx, double threshold_factor,
                            double min_bond_length,
                            int32_t *out_neighbor_indices,
                            double *out_distances, size_t max_neighbors);
