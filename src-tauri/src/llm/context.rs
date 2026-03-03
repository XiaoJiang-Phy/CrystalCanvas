//! [Overview: Extracts core crystal state information to build the LLM context and compress prompts]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::crystal_state::CrystalState;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct AtomContext {
    pub index: usize,
    pub element: String,
    pub frac_pos: [f64; 3],
}

#[derive(Serialize)]
pub struct CrystalContext {
    pub name: String,
    pub num_atoms: usize,
    pub element_composition: HashMap<String, usize>,
    pub spacegroup_hm: String,
    pub lattice_params: LatticeParams,
    pub representative_atoms: Vec<AtomContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_indices: Option<Vec<usize>>,
}

#[derive(Serialize)]
pub struct LatticeParams {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
}

/// Builds a compressed context summary of the crystal state.
pub fn build_crystal_context(
    state: &CrystalState,
    selected_indices: Option<&[usize]>,
) -> CrystalContext {
    let mut comp = HashMap::new();
    for el in &state.elements {
        *comp.entry(el.clone()).or_insert(0) += 1;
    }

    // Heuristic: Gather up to a small number of atoms per element (e.g., 5) to act as
    // "Asymmetric Unit fractionals proxy" to avoid token overflow.
    let mut representative_atoms = Vec::new();
    let mut seen_elements_count = HashMap::new();
    let max_per_element = 5;

    for i in 0..state.num_atoms() {
        let el = &state.elements[i];
        let count = seen_elements_count.entry(el.clone()).or_insert(0);
        if *count < max_per_element {
            representative_atoms.push(AtomContext {
                index: i,
                element: el.clone(),
                frac_pos: [state.fract_x[i], state.fract_y[i], state.fract_z[i]],
            });
            *count += 1;
        }
    }

    CrystalContext {
        name: state.name.clone(),
        num_atoms: state.num_atoms(),
        element_composition: comp,
        spacegroup_hm: state.spacegroup_hm.clone(),
        lattice_params: LatticeParams {
            a: state.cell_a,
            b: state.cell_b,
            c: state.cell_c,
            alpha: state.cell_alpha,
            beta: state.cell_beta,
            gamma: state.cell_gamma,
        },
        representative_atoms,
        selected_indices: selected_indices.map(|s| s.to_vec()),
    }
}
