//! [Overview: Command Bus router that dispatches validated instructions to the physics engine]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::crystal_state::CrystalState;
use crate::io::export::{export_lammps_data, export_poscar, export_qe_input};
use crate::llm::command::{CrystalCommand, ExportFormat};

/// Standardize an element symbol input (e.g. "fe" -> "Fe", " c " -> "C").
pub fn format_element_symbol(symbol: &str) -> String {
    let s = symbol.trim();
    if s.is_empty() {
        return String::new();
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap().to_ascii_uppercase();
    if let Some(second) = chars.next() {
        format!("{}{}", first, second.to_ascii_lowercase())
    } else {
        first.to_string()
    }
}

/// Look up atomic number from element symbol for correct rendering.
pub fn element_to_atomic_number(symbol: &str) -> u8 {
    match symbol {
        "H" => 1,
        "He" => 2,
        "Li" => 3,
        "Be" => 4,
        "B" => 5,
        "C" => 6,
        "N" => 7,
        "O" => 8,
        "F" => 9,
        "Ne" => 10,
        "Na" => 11,
        "Mg" => 12,
        "Al" => 13,
        "Si" => 14,
        "P" => 15,
        "S" => 16,
        "Cl" => 17,
        "Ar" => 18,
        "K" => 19,
        "Ca" => 20,
        "Sc" => 21,
        "Ti" => 22,
        "V" => 23,
        "Cr" => 24,
        "Mn" => 25,
        "Fe" => 26,
        "Co" => 27,
        "Ni" => 28,
        "Cu" => 29,
        "Zn" => 30,
        "Ga" => 31,
        "Ge" => 32,
        "As" => 33,
        "Se" => 34,
        "Br" => 35,
        "Kr" => 36,
        "Rb" => 37,
        "Sr" => 38,
        "Y" => 39,
        "Zr" => 40,
        "Nb" => 41,
        "Mo" => 42,
        "Tc" => 43,
        "Ru" => 44,
        "Rh" => 45,
        "Pd" => 46,
        "Ag" => 47,
        "Cd" => 48,
        "In" => 49,
        "Sn" => 50,
        "Sb" => 51,
        "Te" => 52,
        "I" => 53,
        "Xe" => 54,
        "Cs" => 55,
        "Ba" => 56,
        "La" => 57,
        "Ce" => 58,
        "Pr" => 59,
        "Nd" => 60,
        "Pm" => 61,
        "Sm" => 62,
        "Eu" => 63,
        "Gd" => 64,
        "Tb" => 65,
        "Dy" => 66,
        "Ho" => 67,
        "Er" => 68,
        "Tm" => 69,
        "Yb" => 70,
        "Lu" => 71,
        "Hf" => 72,
        "Ta" => 73,
        "W" => 74,
        "Re" => 75,
        "Os" => 76,
        "Ir" => 77,
        "Pt" => 78,
        "Au" => 79,
        "Hg" => 80,
        "Tl" => 81,
        "Pb" => 82,
        "Bi" => 83,
        "Po" => 84,
        "At" => 85,
        "Rn" => 86,
        "Fr" => 87,
        "Ra" => 88,
        "Ac" => 89,
        "Th" => 90,
        "Pa" => 91,
        "U" => 92,
        _ => 0,
    }
}

/// Executes a validated command, modifying the state in place or performing side effects.
pub fn execute_command(command: CrystalCommand, state: &mut CrystalState) -> Result<(), String> {
    match command {
        CrystalCommand::DeleteAtoms(params) => {
            // Indices should be sorted in descending order to avoid shifting issues when removing multiple,
            // but the underlying CrystalState::delete_atoms expects a slice and handles it.
            // Wait, looking at current `delete_atoms`:
            // It expects a list of indices, preferably sorted descending.
            let mut sorted_indices = params
                .indices
                .iter()
                .map(|&x| x as usize)
                .collect::<Vec<_>>();
            sorted_indices.sort_unstable_by(|a, b| b.cmp(a));
            state.delete_atoms(&sorted_indices);
        }
        CrystalCommand::AddAtom(params) => {
            let atomic_number = element_to_atomic_number(&params.element);
            state
                .try_add_atom(&params.element, atomic_number, params.frac_pos)
                .map_err(|e| format!("Collision Error: {:?}", e))?;
        }
        CrystalCommand::Substitute(params) => {
            let indices = params
                .indices
                .iter()
                .map(|&x| x as usize)
                .collect::<Vec<_>>();
            let atomic_number = element_to_atomic_number(&params.new_element);
            state.substitute_atoms(&indices, &params.new_element, atomic_number);
        }
        CrystalCommand::CleaveSlab(params) => {
            let new_state =
                state.generate_slab(params.miller, params.layers as i32, params.vacuum_a)?;
            *state = new_state;
        }
        CrystalCommand::MakeSupercell(params) => {
            let m = params.matrix;
            // Convert JSON row-major [[a,b,c],[d,e,f],[g,h,i]] to ColMajor [i32; 9]
            let col_major = [
                m[0][0], m[1][0], m[2][0], m[0][1], m[1][1], m[2][1], m[0][2], m[1][2], m[2][2],
            ];
            let new_state = state.generate_supercell(&col_major)?;
            *state = new_state;
        }
        CrystalCommand::ExportFile(params) => match params.format {
            ExportFormat::Poscar => {
                export_poscar(state, &params.path).map_err(|e| e.to_string())?;
            }
            ExportFormat::Lammps => {
                export_lammps_data(state, &params.path).map_err(|e| e.to_string())?;
            }
            ExportFormat::Qe => {
                export_qe_input(state, &params.path).map_err(|e| e.to_string())?;
            }
        },
        CrystalCommand::Batch(params) => {
            // Atomic rollback strategy:
            // Clone the state, execute all commands. If all succeed, update the actual state.
            let mut shadow_state = state.clone();
            for cmd in params.commands {
                if let Err(e) = execute_command(cmd, &mut shadow_state) {
                    return Err(format!("Batch execution failed on a command: {}", e));
                }
            }
            *state = shadow_state;
        }
    }
    Ok(())
}
