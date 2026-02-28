// Physics engine thin wrapper for CXX bridge and C++ tests (Spglib error handling)
#pragma once

#include <cstddef>
#include <cstdint>

// Spglib API expects double arrays
// We expose a safe C-like function that handles exceptions/errors inside

/// Identify the spacegroup number of a given crystal.
///
/// @param lattice 3x3 matrix packed in row-major [9]
/// @param positions Nx3 array of fractional coordinates [n_atoms * 3]
/// @param types Array of atomic types (integers representing elements) [n_atoms]
/// @param n_atoms Total number of atoms
/// @param symprec Tolerance for symmetry matching
/// @return Spacegroup number (0 if failed)
int get_spacegroup(
    const double* lattice,
    const double* positions,
    const int* types,
    size_t n_atoms,
    double symprec
);

/// Output geometry payload for supercell operations to transmit back to Rust
struct SupercellResult {
    double new_lattice[9];
    int n_atoms_new;
    // Data managed conceptually by cxx::Vec in Rust, or we can write a plain C array
    // Here we'll just let the caller provide the pre-allocated buffers based on n_atoms_new
    // But since the number of atoms grows, it's safer for C++ to return a struct and ownership
    // via cxx, or we provide a two-step API (1: get size, 2: fill buffer).
    // For simplicity with cxx, we'll design an API that Rust calls to fill a mutable vector
    // However, since this is a pure C header we'll use a two-step API or memory allocated by std::vector
};

/// Get the number of atoms for a specific supercell expansion
/// @param n_atoms Original number of atoms
/// @param expansion 3x3 expansion matrix (integer)
/// @return Number of new atoms (n_atoms * determinant(expansion))
int get_supercell_size(size_t n_atoms, const int32_t* expansion);

/// Build a supercell.
/// @param lattice Input 3x3 lattice
/// @param positions Input fractional positions (n_atoms x 3)
/// @param types Input atomic types (n_atoms)
/// @param n_atoms Number of original atoms
/// @param expansion 3x3 integer expansion matrix
/// @param out_lattice Output 3x3 lattice
/// @param out_positions Output fractional positions (pre-allocated)
/// @param out_types Output array of atomic types (pre-allocated)
void build_supercell(
    const double* lattice,
    const double* positions,
    const int* types,
    size_t n_atoms,
    const int32_t* expansion,
    double* out_lattice,
    double* out_positions,
    int* out_types
);

/// Get the number of atoms for a specific slab cleavage
/// @param lattice Original 3x3 lattice
/// @param miller Miller indices (h,k,l) for the cleavage plane
/// @param layers Number of atomic layers to generate along the new c-axis
/// @param vacuum_A Vacuum thickness in Angstroms
/// @param n_atoms Original number of atoms
/// @return Number of new atoms
int get_slab_size(
    const double* lattice,
    const int32_t* miller,
    int layers,
    double vacuum_A,
    size_t n_atoms
);

/// Build a slab by cleaving the crystal along a Miller plane.
/// @param lattice Input 3x3 lattice (row-major [9])
/// @param positions Input fractional positions (n_atoms x 3)
/// @param types Input atomic types (n_atoms)
/// @param n_atoms Number of original atoms
/// @param miller Miller indices [h,k,l]
/// @param layers Number of layers
/// @param vacuum_A Vacuum padding in Angstroms
/// @param out_lattice Output 3x3 lattice ([9])
/// @param out_positions Output fractional positions (pre-allocated)
/// @param out_types Output array of atomic types (pre-allocated)
void build_slab(
    const double* lattice,
    const double* positions,
    const int* types,
    size_t n_atoms,
    const int32_t* miller,
    int layers,
    double vacuum_A,
    double* out_lattice,
    double* out_positions,
    int* out_types
);
