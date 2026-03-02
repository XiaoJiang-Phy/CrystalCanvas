//! [Overview: Extracts core crystal state information to build the LLM context and compress prompts]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::crystal_state::CrystalState;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Serialize)]
pub struct CrystalContext {
    pub name: String,
    pub num_atoms: usize,
    pub elements: Vec<String>,
    pub spacegroup_hm: String,
    pub lattice_params: LatticeParams,
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
pub fn build_crystal_context(state: &CrystalState) -> CrystalContext {
    let mut unique_elements = HashSet::new();
    for el in &state.elements {
        unique_elements.insert(el.clone());
    }

    let mut elements: Vec<String> = unique_elements.into_iter().collect();
    elements.sort();

    CrystalContext {
        name: state.name.clone(),
        num_atoms: state.num_atoms(),
        elements,
        spacegroup_hm: state.spacegroup_hm.clone(),
        lattice_params: LatticeParams {
            a: state.cell_a,
            b: state.cell_b,
            c: state.cell_c,
            alpha: state.cell_alpha,
            beta: state.cell_beta,
            gamma: state.cell_gamma,
        },
    }
}
