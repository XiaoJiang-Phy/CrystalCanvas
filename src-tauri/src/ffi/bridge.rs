//! CIF file parsing FFI bridge — cxx bindings between Rust and C++ Gemmi wrapper

#[cxx::bridge]
pub mod ffi {
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

        /// Parse a CIF file and return crystal data.
        /// Returns Err if the file cannot be read or parsed.
        fn parse_cif_file(path: &str) -> Result<FfiCrystalData>;
    }
}
