//! [Overview: Physical sandbox validation layer for LLM Commands (Layer 2 of Safety Pipeline)]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::crystal_state::{
    validate_atom_request, validate_slab_request, validate_supercell_request, CrystalState,
    MAX_STRUCTURAL_ATOMS,
};
use crate::llm::command::CrystalCommand;

#[derive(Debug, PartialEq)]
pub enum SandboxError {
    IndexOutOfBounds { index: u32, max: usize },
    VacuumOutOfRange { vacuum: f64, min: f64, max: f64 },
    NegativeDeterminant,
    TooManyAtomsEstimated { estimated: usize, max: usize },
    InvalidStructuralRequest { message: &'static str },
}

impl std::fmt::Display for SandboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxError::IndexOutOfBounds { index, max } => write!(
                f,
                "Atom index {} is out of bounds (max valid index is {})",
                index,
                max.saturating_sub(1)
            ),
            SandboxError::VacuumOutOfRange { vacuum, min, max } => write!(
                f,
                "Vacuum thickness {} Å is outside allowed range [{} Å, {} Å]",
                vacuum, min, max
            ),
            SandboxError::NegativeDeterminant => write!(
                f,
                "Supercell transformation matrix determinant must be positive"
            ),
            SandboxError::TooManyAtomsEstimated { estimated, max } => write!(
                f,
                "Operation would create {} atoms, exceeding the safety limit of {}",
                estimated, max
            ),
            SandboxError::InvalidStructuralRequest { message } => write!(f, "{}", message),
        }
    }
}

/// Validate a command against the current crystal state.
pub fn validate_command(
    command: &CrystalCommand,
    state: &CrystalState,
) -> Result<(), SandboxError> {
    match command {
        CrystalCommand::DeleteAtoms(params) => {
            for &idx in &params.indices {
                if idx as usize >= state.num_atoms() {
                    return Err(SandboxError::IndexOutOfBounds {
                        index: idx,
                        max: state.num_atoms(),
                    });
                }
            }
        }
        CrystalCommand::AddAtom(params) => {
            let atomic_number = crate::llm::router::element_to_atomic_number(&params.element);
            validate_atom_request(
                &params.element,
                atomic_number,
                params.frac_pos,
                state.num_atoms(),
            )
            .map_err(|message| SandboxError::InvalidStructuralRequest { message })?;
        }
        CrystalCommand::Substitute(params) => {
            if crate::llm::router::element_to_atomic_number(&params.new_element) == 0 {
                return Err(SandboxError::InvalidStructuralRequest {
                    message: "substitute element identity is invalid",
                });
            }
            for &idx in &params.indices {
                if idx as usize >= state.num_atoms() {
                    return Err(SandboxError::IndexOutOfBounds {
                        index: idx,
                        max: state.num_atoms(),
                    });
                }
            }
        }
        CrystalCommand::CleaveSlab(params) => {
            if !params.vacuum_a.is_finite() || params.vacuum_a < 5.0 || params.vacuum_a > 100.0 {
                return Err(SandboxError::VacuumOutOfRange {
                    vacuum: params.vacuum_a,
                    min: 5.0,
                    max: 100.0,
                });
            }
            let layers = i32::try_from(params.layers).map_err(|_| {
                SandboxError::InvalidStructuralRequest {
                    message: "slab layers exceed the supported range",
                }
            })?;
            validate_slab_request(params.miller, layers, params.vacuum_a)
                .map_err(|message| SandboxError::InvalidStructuralRequest { message })?;
            let estimated = state
                .num_atoms()
                .checked_mul(params.layers as usize)
                .ok_or(SandboxError::InvalidStructuralRequest {
                    message: "slab atom count overflow",
                })?;
            if estimated > MAX_STRUCTURAL_ATOMS {
                return Err(SandboxError::TooManyAtomsEstimated {
                    estimated,
                    max: MAX_STRUCTURAL_ATOMS,
                });
            }
        }
        CrystalCommand::MakeSupercell(params) => {
            let m = params.matrix;
            let expansion = [
                m[0][0], m[1][0], m[2][0],
                m[0][1], m[1][1], m[2][1],
                m[0][2], m[1][2], m[2][2],
            ];
            validate_supercell_request(&expansion, state.num_atoms())
                .map_err(|message| SandboxError::InvalidStructuralRequest { message })?;
        }
        CrystalCommand::ExportFile(_) => {
            // File exports are safe from a physics constraint perspective
            // I/O safety is handled at the Tauri command level
        }
        CrystalCommand::Batch(params) => {
            // To be perfectly accurate we would need to simulate the state updates,
            // but for now validating each sequentially against the initial state provides basic bounds checking.
            for cmd in &params.commands {
                validate_command(cmd, state)?;
            }
        }
    }
    Ok(())
}
