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
    | DeleteAtomCommand
    | SubstituteAtomCommand
    | TransformCommand
    | SlabCommand
    | InterfaceMatchCommand
    | DisplacementSeriesCommand; // 用于铁电分析/位移映射

/** 铁电/动力学：原子位移序列生成 */
interface DisplacementSeriesCommand {
    type: 'generate_displacement_series';
    params: {
        atom_index: number;
        direction: [number, number, number]; // Cartesian vector
        start_offset_angstrom: number;
        end_offset_angstrom: number;
        steps: number;
        export_config: {
            format: 'VASP' | 'QE' | 'LAMMPS';
            prefix: string; // 文件名前缀
        };
    };
}

/** 基础操作：添加原子 */
interface AddAtomCommand {
    type: 'add_atom';
    element: string;
    position: [number, number, number]; // Fractional [u, v, w]
    occupancy?: number;
}

/** 基础操作：删除原子 */
interface DeleteAtomCommand {
    type: 'delete_atoms';
    indices: number[]; // Array of atom indices from the SoA
}

/** 基础操作：元素替换/掺杂 */
interface SubstituteAtomCommand {
    type: 'substitute_atoms';
    indices: number[];
    new_element: string;
}

/** 拓扑变换：超胞与应变 */
interface TransformCommand {
    type: 'transform';
    matrix: [number, number, number, number, number, number, number, number, number]; // 3x3 expansion matrix
}

/** 表面建模 */
interface SlabCommand {
    type: 'generate_slab';
    miller_indices: [number, number, number];
    layers: number;
    vacuum_angstrom: number;
    orthogonalize?: boolean;
}

/** 杀手锏功能：异质结匹配算法 */
interface InterfaceMatchCommand {
    type: 'optimize_interface';
    params: {
        target_indices: [number, number, number];
        substrate_indices: [number, number, number];
        max_mismatch: number;      // e.g., 0.02 (2%)
        max_area_angstrom2?: number; // Prevent supercell explosion
    };
}
