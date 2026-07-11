import type {
    BondAnalysisResult,
    BzInfo,
    CrystalState,
    PhononModeSummary,
    WannierInfo,
} from '../types/crystal';

export interface AppSettingsDto {
    atom_scale: number;
    bond_tolerance: number;
    bond_radius: number;
    bond_color: [number, number, number, number];
    custom_atom_colors: Record<string, [number, number, number, number]>;
}

export interface ScreenLabel {
    label: string;
    x: number;
    y: number;
}

export interface VolumetricInfo {
    grid_dims: [number, number, number];
    data_min: number;
    data_max: number;
    format: string;
}

export interface TauriDragPayload {
    paths: string[];
    position: { x: number; y: number };
}

export type IpcErrorCode = 'invalid_argument' | 'io_error' | 'lock_poisoned'
    | 'parse_error' | 'render_error' | 'internal_error';

export interface IpcError {
    code: IpcErrorCode;
    message: string;
    recoverable: boolean;
}

export class IpcException extends Error implements IpcError {
    readonly code: IpcErrorCode;
    readonly recoverable: boolean;

    constructor({ code, message, recoverable }: IpcError) {
        super(message);
        this.name = 'IpcError';
        this.code = code;
        this.recoverable = recoverable;
    }
}

export interface IpcEventContract {
    'menu-action': string;
    state_changed: null;
    'tauri://drag-drop': TauriDragPayload;
    'tauri://drag-enter': TauriDragPayload;
    'tauri://drag-leave': null | undefined;
    'tauri://file-drop': { paths: string[] };
    'tauri://file-drop-cancelled': unknown;
    'tauri://file-drop-hover': unknown;
    undo_stack_changed: { can_undo: boolean; can_redo: boolean };
    view_projection_changed: { is_perspective: boolean };
    volumetric_loaded: VolumetricInfo;
}

export interface IpcCommandContract {
    check_api_key_status: {
        args: { providerType: string };
        result: boolean;
    };
    compute_brillouin_zone: { args: undefined; result: BzInfo };
    generate_kpath_text: { args: { npoints: number }; result: { qe: string; vasp: string } };
    get_bond_analysis: { args: { thresholdFactor: number }; result: BondAnalysisResult };
    get_bz_label_positions: { args: { width: number; height: number }; result: ScreenLabel[] };
    get_crystal_state: { args: undefined; result: CrystalState };
    get_measurement_labels_screen: { args: { width: number; height: number }; result: ScreenLabel[] };
    get_settings: { args: undefined; result: AppSettingsDto };
    llm_chat: { args: { userMessage: string; selectedIndices: number[] | null }; result: string };
    load_axsf_phonon: { args: { path: string }; result: PhononModeSummary[] };
    load_cif_file: { args: { path: string }; result: null };
    load_phonon_interactive: {
        args: { scfIn: string; scfOut: string; modes: string };
        result: PhononModeSummary[];
    };
    load_volumetric_file: { args: { path: string }; result: VolumetricInfo };
    export_file: { args: { format: string; path: string }; result: null };
    export_image: { args: { path: string; width: number; height: number; bgMode: string }; result: null };
    pick_atom: {
        args: { x: number; y: number; screenW: number; screenH: number };
        result: number | null;
    };
    load_wannier_hr: {
        args: { path: string };
        result: WannierInfo;
    };
    set_wannier_t_min: { args: { tMin: number }; result: null };
    set_wannier_r_shell: { args: { shellIdx: number; active: boolean }; result: null };
    set_wannier_orbital: { args: { orbIdx: number; active: boolean }; result: null };
    toggle_wannier_onsite: { args: { show: boolean }; result: null };
    toggle_hopping_display: { args: { show: boolean }; result: null };
    clear_wannier: { args: undefined; result: null };
    write_text_file: { args: { path: string; content: string }; result: null };
}

export type TypedIpcCommand = keyof IpcCommandContract;
export type TypedIpcEvent = keyof IpcEventContract;

export type IpcArgs<Command extends TypedIpcCommand> = IpcCommandContract[Command]['args'];

export type IpcResult<Command extends TypedIpcCommand> = IpcCommandContract[Command]['result'];

function is_integer_triplet(value: unknown): value is [number, number, number] {
    return Array.isArray(value)
        && value.length === 3
        && value.every((component) => Number.isSafeInteger(component)
            && component >= -2_147_483_648
            && component <= 2_147_483_647);
}

function is_record(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function is_finite_number(value: unknown): value is number {
    return typeof value === 'number' && Number.isFinite(value);
}

function is_number_triplet(value: unknown): value is [number, number, number] {
    return Array.isArray(value) && value.length === 3 && value.every(is_finite_number);
}

function is_nonnegative_integer(value: unknown): value is number {
    return Number.isSafeInteger(value) && (value as number) >= 0;
}

function is_rgba(value: unknown): value is [number, number, number, number] {
    return Array.isArray(value) && value.length === 4
        && value.every((component) => is_finite_number(component) && component >= 0 && component <= 1);
}

function is_measurement(value: unknown): boolean {
    if (!is_record(value) || !Array.isArray(value.indices)
        || !value.indices.every(is_nonnegative_integer)
        || !is_finite_number(value.value) || !is_number_triplet(value.label_position)) return false;
    const expected_indices = value.kind === 'Distance' ? 2
        : value.kind === 'Angle' ? 3
            : value.kind === 'Dihedral' ? 4 : 0;
    return expected_indices > 0 && value.indices.length === expected_indices;
}

function is_screen_labels(value: unknown): value is ScreenLabel[] {
    return Array.isArray(value) && value.every((label) => is_record(label)
        && typeof label.label === 'string' && is_finite_number(label.x) && is_finite_number(label.y));
}

function is_phonon_modes(value: unknown): value is PhononModeSummary[] {
    return Array.isArray(value) && value.every((mode) => is_record(mode)
        && typeof mode.index === 'number' && Number.isSafeInteger(mode.index) && mode.index >= 0
        && is_finite_number(mode.frequency_cm1) && typeof mode.is_imaginary === 'boolean'
        && is_number_triplet(mode.q_point));
}

export function is_wannier_info(value: unknown): value is WannierInfo {
    if (typeof value !== 'object' || value === null) return false;
    const candidate = value as Record<string, unknown>;
    return typeof candidate.num_wann === 'number'
        && Number.isSafeInteger(candidate.num_wann)
        && candidate.num_wann >= 0
        && Array.isArray(candidate.r_shells)
        && candidate.r_shells.every(is_integer_triplet)
        && typeof candidate.t_max === 'number'
        && Number.isFinite(candidate.t_max)
        && candidate.t_max >= 0;
}

export function is_volumetric_info(value: unknown): value is VolumetricInfo {
    if (typeof value !== 'object' || value === null) return false;
    const candidate = value as Record<string, unknown>;
    return Array.isArray(candidate.grid_dims)
        && candidate.grid_dims.length === 3
        && candidate.grid_dims.every((dimension) => Number.isSafeInteger(dimension) && dimension > 0)
        && typeof candidate.data_min === 'number'
        && Number.isFinite(candidate.data_min)
        && typeof candidate.data_max === 'number'
        && Number.isFinite(candidate.data_max)
        && candidate.data_min <= candidate.data_max
        && typeof candidate.format === 'string';
}

function is_crystal_state(value: unknown): value is CrystalState {
    if (!is_record(value)) return false;
    const numeric_fields = ['cell_a', 'cell_b', 'cell_c', 'cell_alpha', 'cell_beta', 'cell_gamma'];
    const atom_count = Array.isArray(value.labels) ? value.labels.length : -1;
    return typeof value.name === 'string' && typeof value.spacegroup_hm === 'string'
        && numeric_fields.every((field) => is_finite_number(value[field]))
        && Number.isSafeInteger(value.spacegroup_number)
        && is_nonnegative_integer(value.version) && is_nonnegative_integer(value.intrinsic_sites)
        && value.intrinsic_sites <= atom_count
        && typeof value.is_2d === 'boolean'
        && (value.vacuum_axis === null
            || (is_nonnegative_integer(value.vacuum_axis) && value.vacuum_axis <= 2))
        && Array.isArray(value.labels) && value.labels.every((label) => typeof label === 'string')
        && Array.isArray(value.elements) && value.elements.length === atom_count
        && value.elements.every((element) => typeof element === 'string')
        && Array.isArray(value.atomic_numbers) && value.atomic_numbers.length === atom_count
        && value.atomic_numbers.every((number) => is_nonnegative_integer(number) && number <= 255)
        && ['fract_x', 'fract_y', 'fract_z', 'occupancies'].every((field) => Array.isArray(value[field])
            && value[field].length === atom_count && value[field].every(is_finite_number))
        && Array.isArray(value.cart_positions) && value.cart_positions.length === atom_count
        && value.cart_positions.every(is_number_triplet)
        && Array.isArray(value.measurements) && value.measurements.every(is_measurement);
}

function is_bz_info(value: unknown): value is BzInfo {
    return is_record(value) && typeof value.bravais_type === 'string'
        && ['spacegroup', 'vertices_count', 'edges_count', 'faces_count'].every((field) => Number.isSafeInteger(value[field]))
        && typeof value.is_2d === 'boolean';
}

function is_app_settings(value: unknown): value is AppSettingsDto {
    return is_record(value) && is_finite_number(value.atom_scale)
        && value.atom_scale >= 0
        && is_finite_number(value.bond_tolerance) && value.bond_tolerance >= 0
        && is_finite_number(value.bond_radius) && value.bond_radius >= 0
        && is_rgba(value.bond_color)
        && is_record(value.custom_atom_colors)
        && Object.values(value.custom_atom_colors).every(is_rgba);
}

function is_bond_analysis(value: unknown): value is BondAnalysisResult {
    if (!is_record(value) || !Array.isArray(value.bonds) || !Array.isArray(value.coordination)
        || !Array.isArray(value.bond_length_stats) || !Array.isArray(value.distortion_indices)
        || value.distortion_indices.length !== value.coordination.length
        || !value.distortion_indices.every(is_finite_number)
        || !is_finite_number(value.threshold_factor) || value.threshold_factor <= 0) return false;
    const bonds_valid = value.bonds.every((bond) => is_record(bond)
        && is_nonnegative_integer(bond.atom_i) && is_nonnegative_integer(bond.atom_j)
        && is_finite_number(bond.distance) && bond.distance >= 0);
    const coordination_valid = value.coordination.every((coordination) => is_record(coordination)
        && is_nonnegative_integer(coordination.center_idx) && typeof coordination.element === 'string'
        && is_nonnegative_integer(coordination.coordination_number)
        && Array.isArray(coordination.neighbor_indices)
        && coordination.neighbor_indices.length === coordination.coordination_number
        && coordination.neighbor_indices.every(is_nonnegative_integer)
        && Array.isArray(coordination.neighbor_distances)
        && coordination.neighbor_distances.length === coordination.coordination_number
        && coordination.neighbor_distances.every((distance) => is_finite_number(distance) && distance >= 0)
        && typeof coordination.polyhedron_type === 'string');
    const stats_valid = value.bond_length_stats.every((stat) => is_record(stat)
        && typeof stat.element_a === 'string' && typeof stat.element_b === 'string'
        && is_nonnegative_integer(stat.count) && is_finite_number(stat.min)
        && is_finite_number(stat.max) && is_finite_number(stat.mean)
        && stat.min <= stat.mean && stat.mean <= stat.max);
    return bonds_valid && coordination_valid && stats_valid;
}

export function validate_ipc_result<Command extends TypedIpcCommand>(
    command: Command,
    value: unknown
): IpcResult<Command> {
    if (command === 'load_wannier_hr' && is_wannier_info(value)) {
        return value as IpcResult<Command>;
    }
    if (command === 'load_volumetric_file' && is_volumetric_info(value)) {
        return value as IpcResult<Command>;
    }
    if (command === 'check_api_key_status' && typeof value === 'boolean') return value as IpcResult<Command>;
    if (command === 'compute_brillouin_zone' && is_bz_info(value)) return value as IpcResult<Command>;
    if (command === 'generate_kpath_text' && is_record(value) && typeof value.qe === 'string' && typeof value.vasp === 'string') return value as IpcResult<Command>;
    if (command === 'get_bond_analysis' && is_bond_analysis(value)) return value as IpcResult<Command>;
    if ((command === 'get_bz_label_positions' || command === 'get_measurement_labels_screen') && is_screen_labels(value)) return value as IpcResult<Command>;
    if (command === 'get_crystal_state' && is_crystal_state(value)) return value as IpcResult<Command>;
    if (command === 'get_settings' && is_app_settings(value)) return value as IpcResult<Command>;
    if (command === 'llm_chat' && typeof value === 'string') return value as IpcResult<Command>;
    if ((command === 'load_axsf_phonon' || command === 'load_phonon_interactive') && is_phonon_modes(value)) return value as IpcResult<Command>;
    if (command === 'pick_atom' && (value === null || Number.isSafeInteger(value))) return value as IpcResult<Command>;
    if ((command === 'load_cif_file' || command === 'export_file' || command === 'export_image'
        || command === 'write_text_file' || command === 'set_wannier_t_min'
        || command === 'set_wannier_r_shell' || command === 'set_wannier_orbital'
        || command === 'toggle_wannier_onsite' || command === 'toggle_hopping_display'
        || command === 'clear_wannier') && value === null) return value as IpcResult<Command>;
    throw new Error(`Invalid IPC response for ${command}`);
}

export function normalize_ipc_error(value: unknown): IpcException {
    if (value instanceof IpcException) return value;
    if (is_record(value) && typeof value.code === 'string'
        && ['invalid_argument', 'io_error', 'lock_poisoned', 'parse_error', 'render_error', 'internal_error'].includes(value.code)
        && typeof value.message === 'string' && typeof value.recoverable === 'boolean') {
        return new IpcException(value as unknown as IpcError);
    }
    return new IpcException({
        code: 'internal_error',
        message: value instanceof Error ? value.message : String(value),
        recoverable: false,
    });
}

export function validate_ipc_event<Event extends TypedIpcEvent>(event: Event, value: unknown): IpcEventContract[Event] {
    if (event === 'state_changed' && value === null) return value as IpcEventContract[Event];
    if (event === 'tauri://drag-leave' && (value === null || value === undefined)) return value as IpcEventContract[Event];
    if (event === 'tauri://file-drop-cancelled' || event === 'tauri://file-drop-hover') return value as IpcEventContract[Event];
    if (event === 'menu-action' && typeof value === 'string') return value as IpcEventContract[Event];
    if ((event === 'tauri://drag-enter' || event === 'tauri://drag-drop') && is_record(value)
        && Array.isArray(value.paths) && value.paths.every((path) => typeof path === 'string')
        && is_record(value.position) && is_finite_number(value.position.x) && is_finite_number(value.position.y)) return value as IpcEventContract[Event];
    if (event === 'tauri://file-drop' && is_record(value) && Array.isArray(value.paths) && value.paths.every((path) => typeof path === 'string')) return value as IpcEventContract[Event];
    if (event === 'undo_stack_changed' && is_record(value) && typeof value.can_undo === 'boolean' && typeof value.can_redo === 'boolean') return value as IpcEventContract[Event];
    if (event === 'view_projection_changed' && is_record(value) && typeof value.is_perspective === 'boolean') return value as IpcEventContract[Event];
    if (event === 'volumetric_loaded' && is_volumetric_info(value)) return value as IpcEventContract[Event];
    throw new Error(`Invalid IPC event payload for ${event}`);
}
