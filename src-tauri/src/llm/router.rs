//! [功能概述：Command Bus 的中枢路由器，将验证后的指令分发至物理引擎]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::crystal_state::CrystalState;
use crate::llm::command::{CrystalCommand, ExportFormat};
use crate::io::export::{export_poscar, export_lammps_data, export_qe_input};

/// Executes a validated command, modifying the state in place or performing side effects.
pub fn execute_command(
    command: CrystalCommand,
    state: &mut CrystalState,
) -> Result<(), String> {
    match command {
        CrystalCommand::DeleteAtoms(params) => {
            // Indices should be sorted in descending order to avoid shifting issues when removing multiple,
            // but the underlying CrystalState::delete_atoms expects a slice and handles it.
            // Wait, looking at current `delete_atoms`:
            // It expects a list of indices, preferably sorted descending.
            let mut sorted_indices = params.indices.iter().map(|&x| x as usize).collect::<Vec<_>>();
            sorted_indices.sort_unstable_by(|a, b| b.cmp(a));
            state.delete_atoms(&sorted_indices);
        }
        CrystalCommand::AddAtom(params) => {
            // Default atomic number 0 if unknown, or we could look it up.
            // For MVP, we can just pass 0 or a dummy, as the engine might recalculate or we just rely on string.
            // We can derive atomic number.
            let atomic_number = 0; // The kernel or state.try_add_atom may just use element symbol
            state
                .try_add_atom(&params.element, atomic_number, params.frac_pos)
                .map_err(|e| format!("Collision Error: {:?}", e))?;
        }
        CrystalCommand::Substitute(params) => {
            let indices = params.indices.iter().map(|&x| x as usize).collect::<Vec<_>>();
            let atomic_number = 0;
            state.substitute_atoms(&indices, &params.new_element, atomic_number);
        }
        CrystalCommand::CleaveSlab(params) => {
            let new_state = state.generate_slab(params.miller, params.layers as i32, params.vacuum_a)?;
            *state = new_state;
        }
        CrystalCommand::MakeSupercell(params) => {
            let m = params.matrix;
            // Convert JSON row-major [[a,b,c],[d,e,f],[g,h,i]] to ColMajor [i32; 9]
            let col_major = [
                m[0][0], m[1][0], m[2][0],
                m[0][1], m[1][1], m[2][1],
                m[0][2], m[1][2], m[2][2],
            ];
            let new_state = state.generate_supercell(&col_major)?;
            *state = new_state;
        }
        CrystalCommand::ExportFile(params) => {
            match params.format {
                ExportFormat::Poscar => {
                    export_poscar(state, &params.path).map_err(|e| e.to_string())?;
                }
                ExportFormat::Lammps => {
                    export_lammps_data(state, &params.path).map_err(|e| e.to_string())?;
                }
                ExportFormat::Qe => {
                    export_qe_input(state, &params.path).map_err(|e| e.to_string())?;
                }
            }
        }
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
