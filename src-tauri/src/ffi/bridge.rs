//! Rust ↔ C++ FFI bridge — cxx bindings for CIF parsing and coordinate transforms

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
        fn get_spacegroup(
            lattice: *const f64,
            positions: *const f64,
            types: *const i32,
            n_atoms: usize,
            symprec: f64
        ) -> i32;
    }
}
