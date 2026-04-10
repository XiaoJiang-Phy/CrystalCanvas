// [Overview: Frontend TypeScript interfaces mirroring the Rust backend's CrystalState data structures for safe IPC communication.]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

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
    intrinsic_sites: number; // True atom count before boundary mirroring
}

/**
 * Response from collision detection or other structural validaiton.
 */
export interface CollisionError {
    message: string;
    distance: number;
    indices: [number, number];
}

// =========================================================================
// Structural Analysis Data Types (M10)
// =========================================================================

export interface BondInfo {
    atom_i: number;
    atom_j: number;
    distance: number;
}

export interface CoordinationInfo {
    center_idx: number;
    element: string;
    coordination_number: number;
    neighbor_indices: number[];
    neighbor_distances: number[];
    polyhedron_type: string;
}

export interface BondLengthStat {
    element_a: string;
    element_b: string;
    count: number;
    min: number;
    max: number;
    mean: number;
}

export interface BondAnalysisResult {
    bonds: BondInfo[];
    coordination: CoordinationInfo[];
    bond_length_stats: BondLengthStat[];
    distortion_indices: number[];
    threshold_factor: number;
}

export interface PhononModeSummary {
    index: number;
    frequency_cm1: number;
    is_imaginary: boolean;
    q_point: [number, number, number];
}

// =========================================================================
// Reciprocal Space Data Types (Phase 6)
// =========================================================================

export interface BzInfo {
    bravais_type: string;
    spacegroup: number;
    vertices_count: number;
    edges_count: number;
    faces_count: number;
}

export interface KPathPointUi {
    label: string;
    coord_frac: [number, number, number];
}

export interface KPathInfo {
    points: KPathPointUi[];
    segments: string[][];
}
