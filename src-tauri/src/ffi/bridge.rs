//! Rust ↔ C++ FFI bridge — cxx bindings for CIF parsing and coordinate transforms
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(dead_code)]

#[cxx::bridge]
pub mod ffi {
    /// FFI-safe 3D coordinate (f32) for GPU-oriented data transfer
    #[derive(Clone, Debug, PartialEq)]
    struct FfiVec3f {
        x: f32,
        y: f32,
        z: f32,
    }

    /// FFI-safe atom site data from C++ parser
    struct FfiAtomSite {
        label: String,
        element_symbol: String,
        fract_x: f64,
        fract_y: f64,
        fract_z: f64,
        occ: f64,
        atomic_number: u8,
    }

    /// FFI-safe crystal structure data from C++ parser
    struct FfiCrystalData {
        name: String,
        // Unit cell parameters (angstroms, degrees)
        a: f64,
        b: f64,
        c: f64,
        alpha: f64,
        beta: f64,
        gamma: f64,
        // Space group
        spacegroup_hm: String,
        spacegroup_number: i32,
        // Atom sites
        sites: Vec<FfiAtomSite>,
    }

    #[allow(clippy::missing_safety_doc, clippy::too_many_arguments)]
    unsafe extern "C++" {
        include!("crystal_parser.hpp");
        include!("physics_kernel.hpp");

        /// Parse a CIF file and return crystal data.
        /// Returns Err if the file cannot be read or parsed.
        fn parse_cif_file(path: &str) -> Result<FfiCrystalData>;

        /// Translate all positions by a uniform offset.
        /// Each coordinate component (x, y, z) is shifted by `offset`.
        /// Returns a new Vec with translated positions.
        fn translate_positions(positions: &Vec<FfiVec3f>, offset: f32) -> Vec<FfiVec3f>;

        /// Identify the spacegroup number of a given crystal using Spglib.
        /// Returns Spacegroup number (0 if failed)
        unsafe fn get_spacegroup(
            lattice: *const f64,
            positions: *const f64,
            types: *const i32,
            n_atoms: usize,
            symprec: f64,
        ) -> i32;

        unsafe fn niggli_reduce(lattice: *mut f64, symprec: f64) -> i32;
        unsafe fn delaunay_reduce(lattice: *mut f64, symprec: f64) -> i32;
        unsafe fn standardize_cell(
            lattice: *mut f64,
            positions: *mut f64,
            types: *mut i32,
            n_atoms: usize,
            capacity: usize,
            to_primitive: i32,
            symprec: f64,
        ) -> i32;

        /// Get the number of atoms for a specific supercell expansion
        /// Returns Number of new atoms (n_atoms * determinant(expansion))
        unsafe fn get_supercell_size(n_atoms: usize, expansion: *const i32) -> i32;

        /// Build a supercell.
        unsafe fn build_supercell(
            lattice: *const f64,
            positions: *const f64,
            types: *const i32,
            n_atoms: usize,
            expansion: *const i32,
            out_lattice: *mut f64,
            out_positions: *mut f64,
            out_types: *mut i32,
        );

        /// Get slab size (deprecated — use get_slab_size_v2)
        #[deprecated(note = "Use get_slab_size_v2 for correct deduplication")]
        unsafe fn get_slab_size(
            lattice: *const f64,
            miller: *const i32,
            layers: i32,
            vacuum_a: f64,
            n_atoms: usize,
        ) -> i32;

        /// Build slab (deprecated — use build_slab_v2)
        #[deprecated(note = "Use build_slab_v2 for correct deduplication")]
        unsafe fn build_slab(
            lattice: *const f64,
            positions: *const f64,
            types: *const i32,
            n_atoms: usize,
            miller: *const i32,
            layers: i32,
            vacuum_a: f64,
            out_lattice: *mut f64,
            out_positions: *mut f64,
            out_types: *mut i32,
        );

        /// Upper-bound atom count for slab v2
        unsafe fn get_slab_size_v2(
            lattice: *const f64,
            miller: *const i32,
            n_layers: i32,
            n_atoms: usize,
        ) -> i32;

        /// Build slab with deduplication and vacuum injection
        unsafe fn build_slab_v2(
            lattice: *const f64,
            positions: *const f64,
            types: *const i32,
            n_atoms: usize,
            miller: *const i32,
            n_layers: i32,
            vacuum_a: f64,
            out_lattice: *mut f64,
            out_positions: *mut f64,
            out_types: *mut i32,
        ) -> i32;

        /// Identify distinct atomic layers along the slab normal
        unsafe fn cluster_slab_layers(
            positions: *const f64,
            n_atoms: usize,
            lattice: *const f64,
            layer_tolerance_a: f64,
            out_layer_centers: *mut f64,
            max_layers: usize,
        ) -> i32;

        /// Shift slab termination to expose a different surface layer
        unsafe fn shift_slab_termination(
            positions: *mut f64,
            n_atoms: usize,
            lattice: *const f64,
            target_layer_idx: i32,
            layer_centers: *const f64,
            n_layers: i32,
        );

        /// Check MIC Overlap
        unsafe fn check_overlap_mic(
            lattice: *const f64,
            positions: *const f64,
            n_atoms: usize,
            new_frac_pos: *const f64,
            threshold_a: f64,
        ) -> bool;

        /// Compute all chemical bonds using covalent-radius dynamic thresholding
        unsafe fn compute_bonds(
            lattice: *const f64,
            cart_positions: *const f64,
            frac_positions: *const f64,
            covalent_radii: *const f64,
            num_atoms: usize,
            threshold_factor: f64,
            min_bond_length: f64,
            out_atom_i: *mut i32,
            out_atom_j: *mut i32,
            out_distances: *mut f64,
            max_bonds: usize,
        ) -> i32;

        /// Find coordination shell neighbors for a specific center atom
        unsafe fn find_coordination_shell(
            lattice: *const f64,
            cart_positions: *const f64,
            frac_positions: *const f64,
            covalent_radii: *const f64,
            num_atoms: usize,
            center_idx: usize,
            threshold_factor: f64,
            min_bond_length: f64,
            out_neighbor_indices: *mut i32,
            out_distances: *mut f64,
            max_neighbors: usize,
        ) -> i32;
    }
}

