// [Overview: Frontend TypeScript interfaces mirroring the Rust backend's CrystalState data structures for safe IPC communication.]

/**
 * Mirror of the Rust `CrystalState` struct.
 * Represents the single source of truth for the crystal structure.
 */
export interface CrystalState {
    name: string;
    
    // Unit cell parameters
    cell_a: number;
    cell_b: number;
    cell_c: number;
    cell_alpha: number;
    cell_beta: number;
    cell_gamma: number;

    // Symmetry info
    spacegroup_hm: string;
    spacegroup_number: number;

    // Atom data (SoA Layout for performance)
    labels: string[];      // e.g., ["Fe1", "O2"]
    elements: string[];    // e.g., ["Fe", "O"]
    atomic_numbers: number[]; 
    fract_x: number[];
    fract_y: number[];
    fract_z: number[];
    occupancies: number[];

    // Derived data (populated by backend)
    cart_positions: [number, number, number][]; // [[x, y, z], ...] in Angstroms

    // State metadata
    version: number; // Incremented on every modification
}

/**
 * Response from collision detection or other structural validaiton.
 */
export interface CollisionError {
    message: string;
    distance: number;
    indices: [number, number];
}
