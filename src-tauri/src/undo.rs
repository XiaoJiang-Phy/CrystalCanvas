//! Undo/Redo stack for the CrystalCanvas editor
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::crystal_state::CrystalState;
use std::collections::VecDeque;

/// A lightweight snapshot of the crystal state that excludes heavy caching and data payload.
#[derive(Clone)]
pub struct LightweightState {
    pub name: String,
    pub spacegroup_hm: String,
    pub spacegroup_number: i32,
    pub is_2d: bool,
    pub vacuum_axis: Option<usize>,
    pub intrinsic_sites: usize,
    pub version: u32,
    
    pub cell_a: f64,
    pub cell_b: f64,
    pub cell_c: f64,
    pub cell_alpha: f64,
    pub cell_beta: f64,
    pub cell_gamma: f64,
    
    pub labels: Vec<String>,
    pub elements: Vec<String>,
    pub fract_x: Vec<f64>,
    pub fract_y: Vec<f64>,
    pub fract_z: Vec<f64>,
    pub occupancies: Vec<f64>,
    pub atomic_numbers: Vec<u8>,
    pub cart_positions: Vec<[f32; 3]>,
    
    pub selected_atoms: Vec<usize>,
}

impl LightweightState {
    pub fn from_crystal_state(cs: &CrystalState) -> Self {
        Self {
            name: cs.name.clone(),
            spacegroup_hm: cs.spacegroup_hm.clone(),
            spacegroup_number: cs.spacegroup_number,
            is_2d: cs.is_2d,
            vacuum_axis: cs.vacuum_axis,
            intrinsic_sites: cs.intrinsic_sites,
            version: cs.version,
            
            cell_a: cs.cell_a,
            cell_b: cs.cell_b,
            cell_c: cs.cell_c,
            cell_alpha: cs.cell_alpha,
            cell_beta: cs.cell_beta,
            cell_gamma: cs.cell_gamma,
            
            labels: cs.labels.clone(),
            elements: cs.elements.clone(),
            fract_x: cs.fract_x.clone(),
            fract_y: cs.fract_y.clone(),
            fract_z: cs.fract_z.clone(),
            occupancies: cs.occupancies.clone(),
            atomic_numbers: cs.atomic_numbers.clone(),
            cart_positions: cs.cart_positions.clone(),
            
            selected_atoms: cs.selected_atoms.clone(),
        }
    }
}

pub struct UndoStack {
    past: VecDeque<LightweightState>,
    future: VecDeque<LightweightState>,
    pub max_depth: usize,
}

impl UndoStack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            past: VecDeque::with_capacity(max_depth),
            future: VecDeque::with_capacity(max_depth),
            max_depth,
        }
    }

    /// Push a pre-mutation snapshot. Clears futures.
    pub fn push(&mut self, state: LightweightState) {
        self.future.clear();
        self.past.push_back(state);
        if self.past.len() > self.max_depth {
            self.past.pop_front();
        }
    }

    /// Move back in history. We receive the `current_state` to store in `future`.
    pub fn undo(&mut self, current_state: LightweightState) -> Option<LightweightState> {
        if let Some(prev) = self.past.pop_back() {
            self.future.push_front(current_state);
            Some(prev)
        } else {
            None
        }
    }

    /// Move forward in history. We receive the `current_state` to store in `past`.
    pub fn redo(&mut self, current_state: LightweightState) -> Option<LightweightState> {
        if let Some(next) = self.future.pop_front() {
            self.past.push_back(current_state);
            Some(next)
        } else {
            None
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
    }
}
