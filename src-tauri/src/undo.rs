//! Undo/Redo stack for the CrystalCanvas editor
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::crystal_state::CrystalState;
use std::collections::VecDeque;

/// A lightweight snapshot of the crystal state that excludes heavy caching and data payload.
#[derive(Clone)]
pub struct StructuralSnapshot {
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
    pub measurements: Vec<crate::crystal_state::MeasurementOverlay>,
}

impl StructuralSnapshot {
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
            measurements: cs.measurements.clone(),
        }
    }

    pub fn restore_for_rollback(self, cs: &mut CrystalState) {
        self.restore_structural_fields(cs);
    }

    pub fn restore_for_history(self, cs: &mut CrystalState) {
        self.restore_structural_fields(cs);
        cs.invalidate_structure_bound_data();
    }

    pub fn into_crystal_state(self) -> CrystalState {
        let mut cs = CrystalState::default();
        self.restore_structural_fields(&mut cs);
        cs
    }

    pub fn swap_structural_fields(&mut self, cs: &mut CrystalState) {
        std::mem::swap(&mut self.name, &mut cs.name);
        std::mem::swap(&mut self.spacegroup_hm, &mut cs.spacegroup_hm);
        std::mem::swap(&mut self.spacegroup_number, &mut cs.spacegroup_number);
        std::mem::swap(&mut self.is_2d, &mut cs.is_2d);
        std::mem::swap(&mut self.vacuum_axis, &mut cs.vacuum_axis);
        std::mem::swap(&mut self.intrinsic_sites, &mut cs.intrinsic_sites);
        std::mem::swap(&mut self.version, &mut cs.version);
        std::mem::swap(&mut self.cell_a, &mut cs.cell_a);
        std::mem::swap(&mut self.cell_b, &mut cs.cell_b);
        std::mem::swap(&mut self.cell_c, &mut cs.cell_c);
        std::mem::swap(&mut self.cell_alpha, &mut cs.cell_alpha);
        std::mem::swap(&mut self.cell_beta, &mut cs.cell_beta);
        std::mem::swap(&mut self.cell_gamma, &mut cs.cell_gamma);
        std::mem::swap(&mut self.labels, &mut cs.labels);
        std::mem::swap(&mut self.elements, &mut cs.elements);
        std::mem::swap(&mut self.fract_x, &mut cs.fract_x);
        std::mem::swap(&mut self.fract_y, &mut cs.fract_y);
        std::mem::swap(&mut self.fract_z, &mut cs.fract_z);
        std::mem::swap(&mut self.occupancies, &mut cs.occupancies);
        std::mem::swap(&mut self.atomic_numbers, &mut cs.atomic_numbers);
        std::mem::swap(&mut self.cart_positions, &mut cs.cart_positions);
        std::mem::swap(&mut self.selected_atoms, &mut cs.selected_atoms);
        std::mem::swap(&mut self.measurements, &mut cs.measurements);
    }

    fn restore_structural_fields(self, cs: &mut CrystalState) {
        cs.name = self.name;
        cs.spacegroup_hm = self.spacegroup_hm;
        cs.spacegroup_number = self.spacegroup_number;
        cs.is_2d = self.is_2d;
        cs.vacuum_axis = self.vacuum_axis;
        cs.intrinsic_sites = self.intrinsic_sites;
        cs.version = self.version;
        cs.cell_a = self.cell_a;
        cs.cell_b = self.cell_b;
        cs.cell_c = self.cell_c;
        cs.cell_alpha = self.cell_alpha;
        cs.cell_beta = self.cell_beta;
        cs.cell_gamma = self.cell_gamma;
        cs.labels = self.labels;
        cs.elements = self.elements;
        cs.fract_x = self.fract_x;
        cs.fract_y = self.fract_y;
        cs.fract_z = self.fract_z;
        cs.occupancies = self.occupancies;
        cs.atomic_numbers = self.atomic_numbers;
        cs.cart_positions = self.cart_positions;
        cs.selected_atoms = self.selected_atoms;
        cs.measurements = self.measurements;
    }
}

pub struct UndoStack {
    past: VecDeque<StructuralSnapshot>,
    future: VecDeque<StructuralSnapshot>,
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
    pub fn push(&mut self, state: StructuralSnapshot) {
        self.future.clear();
        self.past.push_back(state);
        if self.past.len() > self.max_depth {
            self.past.pop_front();
        }
    }

    /// Move back in history. We receive the `current_state` to store in `future`.
    pub fn undo(&mut self, current_state: StructuralSnapshot) -> Option<StructuralSnapshot> {
        if let Some(prev) = self.past.pop_back() {
            self.future.push_front(current_state);
            Some(prev)
        } else {
            None
        }
    }

    pub fn undo_candidate_mut(&mut self) -> Option<&mut StructuralSnapshot> {
        self.past.back_mut()
    }

    pub fn commit_undo(&mut self) -> bool {
        if let Some(current) = self.past.pop_back() {
            self.future.push_front(current);
            true
        } else {
            false
        }
    }

    /// Move forward in history. We receive the `current_state` to store in `past`.
    pub fn redo(&mut self, current_state: StructuralSnapshot) -> Option<StructuralSnapshot> {
        if let Some(next) = self.future.pop_front() {
            self.past.push_back(current_state);
            Some(next)
        } else {
            None
        }
    }

    pub fn redo_candidate_mut(&mut self) -> Option<&mut StructuralSnapshot> {
        self.future.front_mut()
    }

    pub fn commit_redo(&mut self) -> bool {
        if let Some(current) = self.future.pop_front() {
            self.past.push_back(current);
            true
        } else {
            false
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_dummy_state(version: u32) -> StructuralSnapshot {
        let mut cs = CrystalState::default();
        cs.version = version;
        StructuralSnapshot::from_crystal_state(&cs)
    }

    #[test]
    fn test_undo_redo() {
        let mut stack = UndoStack::new(2);
        
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        
        // Push 3 states into depth 2 (testing ejection of oldest for extreme dimension limits)
        stack.push(create_dummy_state(1));
        stack.push(create_dummy_state(2));
        stack.push(create_dummy_state(3));
        
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.past.len(), 2);
        assert_eq!(stack.past[0].version, 2);
        assert_eq!(stack.past[1].version, 3);
        
        // Undo
        let prev = stack.undo(create_dummy_state(4)).unwrap();
        assert_eq!(prev.version, 3);
        assert!(stack.can_undo());
        assert!(stack.can_redo());
        
        let prev2 = stack.undo(prev).unwrap();
        assert_eq!(prev2.version, 2);
        assert!(!stack.can_undo()); // Depth 2 means we only remember 2 past states
        assert!(stack.can_redo());
        
        // Undo over limit (empty) - testing extreme index out of bound conditions
        assert!(stack.undo(prev2.clone()).is_none());
        
        // Redo
        let next = stack.redo(prev2).unwrap();
        assert_eq!(next.version, 3);
        let next2 = stack.redo(next).unwrap();
        assert_eq!(next2.version, 4);
        
        // Redo over limit - testing bounds
        assert!(stack.redo(next2).is_none());
    }

    #[test]
    fn test_undo_push_clears_future() {
        let mut stack = UndoStack::new(10);
        stack.push(create_dummy_state(1));
        stack.push(create_dummy_state(2));
        
        // Move back
        stack.undo(create_dummy_state(3)).unwrap();
        assert!(stack.can_redo());
        
        // Push should obliterate future states
        stack.push(create_dummy_state(4));
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_undo_excludes_volumetric() {
        let mut cs = CrystalState::default();
        cs.cart_positions.push([0.0, 0.0, 0.0]); // Add some pseudo data
        
        let ls = StructuralSnapshot::from_crystal_state(&cs);
        // By definition, StructuralSnapshot does NOT mirror the full memory map of CrystalState.
        // We assert its structural memory size is deterministic and ignores large allocs.
        let ls_size = std::mem::size_of::<StructuralSnapshot>();
        assert!(ls_size < 500); // Expecting ~350 bytes for pointers
        
        // The properties requested (atom count) must align
        assert_eq!(ls.cart_positions.len(), 1);
    }
}
