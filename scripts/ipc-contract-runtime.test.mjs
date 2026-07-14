import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const source = await readFile(new URL('../src/ipc/contracts.ts', import.meta.url), 'utf8');
const wannier_command_source = await readFile(new URL('../src-tauri/src/commands/wannier.rs', import.meta.url), 'utf8');
const wannier_core_source = await readFile(new URL('../src-tauri/src/wannier.rs', import.meta.url), 'utf8');
const reciprocal_command_source = await readFile(new URL('../src-tauri/src/commands/reciprocal.rs', import.meta.url), 'utf8');
const transpiled = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const contracts = await import(`data:text/javascript;base64,${Buffer.from(transpiled).toString('base64')}`);

test('Wannier IPC validator accepts the exact Rust wire shape', () => {
    assert.equal(contracts.is_wannier_info({
        num_wann: 2,
        r_shells: [[0, 0, 0], [1, -2, 3]],
        t_max: 1.25,
    }), true);
});

test('Wannier IPC validator rejects malformed physical and integer values', () => {
    for (const value of [
        { num_wann: -1, r_shells: [], t_max: 1 },
        { num_wann: 1, r_shells: [[2_147_483_648, 0, 0]], t_max: 1 },
        { num_wann: 1, r_shells: [[0, 0, 0]], t_max: -1 },
        { num_wann: 1, r_shells: [{ rx: 0, ry: 0, rz: 0 }], t_max: 1 },
    ]) {
        assert.equal(contracts.is_wannier_info(value), false);
    }
});

test('Wannier backend commands use the structured IPC result contract', () => {
    for (const command of [
        'load_wannier_hr',
        'set_wannier_t_min',
        'set_wannier_r_shell',
        'set_wannier_orbital',
        'toggle_wannier_onsite',
        'toggle_hopping_display',
        'clear_wannier',
    ]) {
        const start = wannier_command_source.indexOf(`pub fn ${command}`);
        const body = wannier_command_source.slice(start, wannier_command_source.indexOf('{', start));
        assert.match(body, /\)\s*-> IpcResult</, command);
    }
    assert.doesNotMatch(wannier_command_source, /\)\s*-> Result<[^\n]*String>/);
    assert.doesNotMatch(wannier_command_source, /wannier_overlay\.take\(\)/);
    assert.doesNotMatch(wannier_command_source, /if let Ok\(settings\) = settings_state\.lock\(\)/);
});

test('Wannier state-changing commands enforce CrystalState to Settings to Renderer lock order', () => {
    for (const command of [
        'load_wannier_hr',
        'clear_wannier',
    ]) {
        const start = wannier_command_source.indexOf(`pub fn ${command}`);
        const next = wannier_command_source.indexOf('#[tauri::command]', start + 1);
        const body = wannier_command_source.slice(start, next < 0 ? undefined : next);
        const crystal_lock = body.search(/crystal_state\s*\.lock\(\)/);
        const settings_lock = body.search(/settings_state\s*\.lock\(\)/);
        const renderer_lock = body.search(/renderer_state\s*\.lock\(\)/);
        assert.ok(crystal_lock >= 0 && crystal_lock < settings_lock && settings_lock < renderer_lock, command);
    }
    const helper_start = wannier_command_source.indexOf('fn apply_wannier_change');
    const helper_end = wannier_command_source.indexOf('#[tauri::command]', helper_start);
    const helper = wannier_command_source.slice(helper_start, helper_end);
    const crystal_lock = helper.search(/crystal_state\s*\.lock\(\)/);
    const settings_lock = helper.search(/settings_state\s*\.lock\(\)/);
    const renderer_lock = helper.search(/renderer_state\.lock\(\)/);
    assert.ok(crystal_lock >= 0 && crystal_lock < settings_lock && settings_lock < renderer_lock);
});

test('Wannier interactive filtering stays single-pass and outside the renderer lock', () => {
    const filter_start = wannier_core_source.indexOf('pub fn filter_and_rebuild');
    const filter_end = wannier_core_source.indexOf('\n    }\n}', filter_start);
    const filter_body = wannier_core_source.slice(filter_start, filter_end);
    assert.doesNotMatch(filter_body, /\.find\(|contains_key/);

    const helper_start = wannier_command_source.indexOf('fn apply_wannier_change');
    const helper_end = wannier_command_source.indexOf('#[tauri::command]', helper_start);
    const helper = wannier_command_source.slice(helper_start, helper_end);
    const filter = helper.indexOf('.filter_and_rebuild');
    const scene_build = helper.indexOf('build_wannier_scene');
    const renderer_lock = helper.indexOf('renderer_state.lock()');
    assert.ok(filter >= 0 && filter < scene_build && scene_build < renderer_lock);
});

test('Wannier mutation commands accept only unit success responses', () => {
    for (const command of [
        'set_wannier_t_min',
        'set_wannier_r_shell',
        'set_wannier_orbital',
        'toggle_wannier_onsite',
        'toggle_hopping_display',
        'clear_wannier',
    ]) {
        assert.equal(contracts.validate_ipc_result(command, null), null);
        assert.throws(() => contracts.validate_ipc_result(command, undefined));
    }
});

test('Reciprocal backend commands use structured IPC results', () => {
    for (const command of [
        'compute_brillouin_zone', 'toggle_bz_display', 'get_kpath_info',
        'set_bz_scale', 'generate_kpath_text', 'get_bz_label_positions',
    ]) {
        const start = reciprocal_command_source.indexOf(`pub fn ${command}`);
        const body = reciprocal_command_source.slice(start, reciprocal_command_source.indexOf('{', start));
        assert.match(body, /\)\s*-> IpcResult</, command);
    }
    assert.doesNotMatch(reciprocal_command_source, /\)\s*-> Result<[^\n]*String>/);
});

test('Reciprocal renderer mutations accept only unit success responses', () => {
    for (const command of ['toggle_bz_display', 'set_bz_scale']) {
        assert.equal(contracts.validate_ipc_result(command, null), null);
        assert.throws(() => contracts.validate_ipc_result(command, undefined));
    }
});

test('Hiding the Brillouin zone does not acquire the crystal-state lock', () => {
    const start = reciprocal_command_source.indexOf('pub fn toggle_bz_display');
    const end = reciprocal_command_source.indexOf('#[tauri::command]', start);
    const command = reciprocal_command_source.slice(start, end);
    const hide_branch = command.indexOf('if !show');
    const renderer_lock = command.indexOf('renderer_state', hide_branch);
    const crystal_lock = command.indexOf('crystal_state', hide_branch);
    assert.ok(hide_branch >= 0 && hide_branch < renderer_lock && renderer_lock < crystal_lock);
});

test('K-path info validation checks fractional coordinates and segment labels', () => {
    const info = {
        points: [
            { label: 'Γ', coord_frac: [0, 0, 0] },
            { label: 'X', coord_frac: [0.5, 0, 0] },
        ],
        segments: [['Γ', 'X']],
    };
    assert.deepEqual(contracts.validate_ipc_result('get_kpath_info', info), info);
    assert.throws(() => contracts.validate_ipc_result('get_kpath_info', {
        ...info, points: [{ label: 'Γ', coord_frac: [0, Number.NaN, 0] }],
    }));
    assert.throws(() => contracts.validate_ipc_result('get_kpath_info', {
        ...info, segments: [['Γ', 1]],
    }));
    assert.throws(() => contracts.validate_ipc_result('get_kpath_info', {
        ...info, segments: [['Γ', 'M']],
    }));
    assert.throws(() => contracts.validate_ipc_result('get_kpath_info', {
        ...info, points: [...info.points, { label: 'X', coord_frac: [0, 0.5, 0] }],
    }));
    assert.throws(() => contracts.validate_ipc_result('get_kpath_info', {
        ...info, segments: [['Γ']],
    }));
});

test('Volumetric IPC validator accepts finite ordered grid metadata', () => {
    assert.equal(contracts.is_volumetric_info({
        grid_dims: [32, 24, 16], data_min: -0.5, data_max: 1.25, format: 'cube',
    }), true);
});

test('Volumetric IPC validator rejects invalid dimensions and non-finite ranges', () => {
    for (const value of [
        { grid_dims: [32, 0, 16], data_min: 0, data_max: 1, format: 'cube' },
        { grid_dims: [32, 24, 16], data_min: 2, data_max: 1, format: 'cube' },
        { grid_dims: [32, 24, 16], data_min: 0, data_max: Number.POSITIVE_INFINITY, format: 'cube' },
    ]) assert.equal(contracts.is_volumetric_info(value), false);
});

test('Tauri v2 drag events accept the installed API wire payload', () => {
    const payload = { paths: ['/tmp/Si.cif'], position: { x: 120, y: 80 } };
    assert.deepEqual(contracts.validate_ipc_event('tauri://drag-enter', payload), payload);
    assert.deepEqual(contracts.validate_ipc_event('tauri://drag-drop', payload), payload);
    assert.equal(contracts.validate_ipc_event('tauri://drag-leave', null), null);
});

test('Tauri v2 drag events reject missing paths or invalid positions', () => {
    assert.throws(() => contracts.validate_ipc_event('tauri://drag-enter', { position: { x: 1, y: 2 } }));
    assert.throws(() => contracts.validate_ipc_event('tauri://drag-drop', { paths: [], position: { x: Number.NaN, y: 2 } }));
});

test('IPC errors retain structured fields and standard Error string semantics', () => {
    const structured = { code: 'io_error', message: 'disk full', recoverable: true };
    const normalized = contracts.normalize_ipc_error(structured);
    assert.equal(normalized instanceof Error, true);
    assert.equal(normalized.code, 'io_error');
    assert.equal(normalized.message, 'disk full');
    assert.equal(normalized.recoverable, true);
    assert.match(String(normalized), /disk full/);

    const legacy = contracts.normalize_ipc_error('legacy failure');
    assert.equal(legacy instanceof Error, true);
    assert.equal(legacy.code, 'internal_error');
    assert.equal(legacy.message, 'legacy failure');
    assert.equal(legacy.recoverable, false);
    assert.match(String(legacy), /legacy failure/);
});

test('CrystalState validation enforces finite aligned SoA arrays and measurement DTOs', () => {
    const state = {
        name: 'Si',
        cell_a: 5.43, cell_b: 5.43, cell_c: 5.43,
        cell_alpha: 90, cell_beta: 90, cell_gamma: 90,
        spacegroup_hm: 'Fd-3m', spacegroup_number: 227,
        labels: ['Si1'], elements: ['Si'], atomic_numbers: [14],
        fract_x: [0], fract_y: [0], fract_z: [0], occupancies: [1],
        cart_positions: [[0, 0, 0]], version: 1, intrinsic_sites: 1,
        is_2d: false, vacuum_axis: null,
        measurements: [{ indices: [0, 0], kind: 'Distance', value: 0, label_position: [0, 0, 0] }],
    };
    assert.deepEqual(contracts.validate_ipc_result('get_crystal_state', state), state);
    assert.throws(() => contracts.validate_ipc_result('get_crystal_state', {
        ...state, elements: [],
    }));
    assert.throws(() => contracts.validate_ipc_result('get_crystal_state', {
        ...state, fract_x: [Number.NaN],
    }));
    assert.throws(() => contracts.validate_ipc_result('get_crystal_state', {
        ...state, measurements: [{ indices: ['0'], kind: 'Distance', value: 0, label_position: [0, 0, 0] }],
    }));
});

test('Bond analysis validation rejects malformed nested records', () => {
    const analysis = {
        bonds: [{ atom_i: 0, atom_j: 1, distance: 2.35 }],
        coordination: [{
            center_idx: 0, element: 'Si', coordination_number: 1,
            neighbor_indices: [1], neighbor_distances: [2.35], polyhedron_type: '',
        }],
        bond_length_stats: [{ element_a: 'Si', element_b: 'Si', count: 1, min: 2.35, max: 2.35, mean: 2.35 }],
        distortion_indices: [0], threshold_factor: 1.2,
    };
    assert.deepEqual(contracts.validate_ipc_result('get_bond_analysis', analysis), analysis);
    assert.throws(() => contracts.validate_ipc_result('get_bond_analysis', {
        ...analysis, bonds: [{ atom_i: 0, atom_j: 1, distance: '2.35' }],
    }));
    assert.throws(() => contracts.validate_ipc_result('get_bond_analysis', {
        ...analysis,
        coordination: [{ ...analysis.coordination[0], neighbor_distances: [] }],
    }));
});

test('Settings validation checks every RGBA tuple and custom color entry', () => {
    const settings = {
        atom_scale: 1, bond_tolerance: 0.45, bond_radius: 0.08,
        bond_color: [0.65, 0.65, 0.65, 1],
        custom_atom_colors: { Si: [0.1, 0.2, 0.3, 1] },
    };
    assert.deepEqual(contracts.validate_ipc_result('get_settings', settings), settings);
    assert.throws(() => contracts.validate_ipc_result('get_settings', {
        ...settings, custom_atom_colors: { Si: [0.1, 0.2, 0.3] },
    }));
    assert.throws(() => contracts.validate_ipc_result('get_settings', {
        ...settings, bond_color: [0.65, 0.65, 0.65, 1.5],
    }));
});
