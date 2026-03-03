//! [M10 Manual Check] Verify TiO2 Rutile polyhedra recognition
use crystal_canvas::crystal_state::CrystalState;
use crystal_canvas::ffi;

fn test_data_path(filename: &str) -> String {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    format!("{manifest}/../tests/data/{filename}")
}

#[test]
fn test_rutile_tio6_octahedra() {
    let path = test_data_path("rutile.cif");
    let data = ffi::parse_cif_file(&path).expect("Failed to parse rutile.cif");
    let mut state = CrystalState::from_ffi(data);

    // Compute bonds with threshold factor 1.05 (excludes Ti-Ti bonds while keeping axial Ti-O)
    state.compute_bond_analysis(1.05);

    let bond_analysis = state.bond_analysis.as_ref().expect("Bond analysis should be computed");

    // Ti (only check intrinsic sites, as mirrored atoms don't have coordination info)
    let ti_indices: Vec<usize> = state.elements.iter().take(state.intrinsic_sites).enumerate()
        .filter(|(_, e)| *e == "Ti")
        .map(|(i, _)| i)
        .collect();

    assert!(!ti_indices.is_empty(), "Rutile must have Ti atoms");

    for &ti_idx in &ti_indices {
        let coord = &bond_analysis.coordination[ti_idx];
        
        // Rutile Ti is 6-coordinated
        assert_eq!(coord.coordination_number, 6, "Ti in Rutile should be 6-coordinated (TiO6)");
        assert_eq!(coord.polyhedron_type, "Octahedron", "CN=6 should be classified as Octahedron");
    }

    // Oxygen (only check intrinsic sites)
    let o_indices: Vec<usize> = state.elements.iter().take(state.intrinsic_sites).enumerate()
        .filter(|(_, e)| *e == "O")
        .map(|(i, _)| i)
        .collect();

    for &o_idx in &o_indices {
        let coord = &bond_analysis.coordination[o_idx];
        assert_eq!(coord.coordination_number, 3, "O in Rutile should be 3-coordinated");
        assert_eq!(coord.polyhedron_type, "Trigonal Planar");
    }
}
