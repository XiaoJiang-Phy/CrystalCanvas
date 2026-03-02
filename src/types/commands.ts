// [Overview: Type definitions for the CrystalCommand JSON protocol. LLM generates these commands to drive the physics engine.]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * The root container for LLM-generated instructions.
 * Supports single commands or a sequence (workflow).
 */
export interface CrystalCommandBundle {
    commands: CrystalCommand[];
    metadata?: {
        explanation?: string; // LLM's reasoning for the user
        steps_count: number;
    };
}

export type CrystalCommand = 
    | AddAtomCommand
    | DeleteAtomsCommand
    | SubstituteAtomCommand
    | TransformCommand
    | SlabCommand
    | ExportFileCommand
    | BatchCommand; // Core commands

interface AddAtomCommand {
    action: 'add_atom';
    params: {
        element: string;
        frac_pos: [number, number, number];
    };
}

interface DeleteAtomsCommand {
    action: 'delete_atoms';
    params: {
        indices: number[];
    };
}

interface SubstituteAtomCommand {
    action: 'substitute';
    params: {
        indices: number[];
        new_element: string;
    };
}

interface TransformCommand {
    action: 'make_supercell';
    params: {
        matrix: [[number, number, number], [number, number, number], [number, number, number]];
    };
}

interface SlabCommand {
    action: 'cleave_slab';
    params: {
        miller: [number, number, number];
        layers: number;
        vacuum_a: number;
    };
}

interface ExportFileCommand {
    action: 'export_file';
    params: {
        format: 'POSCAR' | 'LAMMPS' | 'QE';
        path: string;
    };
}

interface BatchCommand {
    action: 'batch';
    params: {
        commands: CrystalCommand[];
    };
}
