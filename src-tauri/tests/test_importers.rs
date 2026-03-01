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
}

#[test]
fn test_load_pdb() {
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
    
    // Test conversion from cartesian (1.0, 1.0, 1.0) to fractional in a 10.0 cell
    assert!((state.fract_x[0] - 0.1).abs() < 1e-4);
    assert!((state.fract_y[0] - 0.1).abs() < 1e-4);
    assert!((state.fract_z[0] - 0.1).abs() < 1e-4);
}
