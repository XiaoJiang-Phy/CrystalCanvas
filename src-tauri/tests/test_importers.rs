//! Integration tests for format importers (XYZ, PDB)
use crystal_canvas::io::import::load_file;
use std::fs;

#[test]
fn test_load_xyz() {
    let path = "test_input.xyz";
    let content = "2
Test XYZ
O 0.0 0.0 0.0
C 1.0 1.0 1.0
";
    fs::write(path, content).unwrap();
    
    let state = load_file(path).unwrap();
    fs::remove_file(path).unwrap();
    
    assert_eq!(state.name, "Test XYZ");
    assert_eq!(state.num_atoms(), 2);
    assert_eq!(state.elements[0], "O");
    assert_eq!(state.elements[1], "C");
    assert_eq!(state.atomic_numbers[0], 8);
    assert_eq!(state.atomic_numbers[1], 6);

    // Box calculation
    // min is 0.0, max is 1.0
    // padding = 10.0, dx = 1.0 + 10.0 = 11.0
    // center shift: min_x = 0.0, fx = (0.0 - 0.0 + 5.0) / 11.0
    assert!((state.cell_a - 11.0).abs() < 1e-6);
    assert!((state.fract_x[0] - 5.0/11.0).abs() < 1e-6);
    assert!((state.fract_x[1] - 6.0/11.0).abs() < 1e-6);
}

#[test]
fn test_load_pdb_with_cryst1() {
    let path = "test_input.pdb";
    let content = "\
CRYST1   10.000   10.000   10.000  90.00  90.00  90.00 P 1           1
ATOM      1  C   UNL     1       1.000   1.000   1.000  1.00  0.00           C  
HETATM    2  O   HOH     2       5.000   5.000   5.000  1.00  0.00           O  
";
    fs::write(path, content).unwrap();
    
    let state = load_file(path).unwrap();
    fs::remove_file(path).unwrap();
    
    assert_eq!(state.num_atoms(), 2);
    assert_eq!(state.cell_a, 10.0);
    assert_eq!(state.elements[0], "C");
    assert_eq!(state.elements[1], "O");
    assert_eq!(state.atomic_numbers[0], 6);
    assert_eq!(state.atomic_numbers[1], 8);
    
    assert!((state.fract_x[0] - 0.1).abs() < 1e-4);
    assert!((state.fract_y[0] - 0.1).abs() < 1e-4);
    assert!((state.fract_z[0] - 0.1).abs() < 1e-4);
}

#[test]
fn test_load_pdb_no_cryst1() {
    let path = "test_input_no_cryst1.pdb";
    let content = "\
ATOM      1  N   UNL     1       0.000   0.000   0.000  1.00  0.00           N  
ATOM      2  O   UNL     1       1.000   0.000   0.000  1.00  0.00           O  
";
    fs::write(path, content).unwrap();

    let state = load_file(path).unwrap();
    fs::remove_file(path).unwrap();

    assert_eq!(state.num_atoms(), 2);
    assert_eq!(state.cell_a, 11.0); // 1.0 (max-min) + 10.0 padding
    assert_eq!(state.cell_b, 10.0);
    assert_eq!(state.cell_c, 10.0);

    // First atom at x=0.0 -> box min is 0.0 -> shifted by +5.0 -> 5.0 / 11.0
    // Second atom at x=1.0 -> 6.0 / 11.0
    assert!((state.fract_x[0] - 5.0/11.0).abs() < 1e-4);
    assert!((state.fract_x[1] - 6.0/11.0).abs() < 1e-4);
}
