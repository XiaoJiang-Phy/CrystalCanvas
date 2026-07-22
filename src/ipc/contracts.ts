import type {
    BondAnalysisResult,
    BzInfo,
    CrystalState,
    KPathInfo,
    MeasurementOverlay,
    PhononModeSummary,
    WannierInfo,
} from '../types/crystal';
import type { IpcCommandName } from './commands.generated';

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

export interface KPathText {
    qe: string;
    vasp: string;
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

export interface StateChangedPayload {
    version: number;
}

export type CameraAxis = 'a' | 'b' | 'c' | 'a_star' | 'b_star' | 'c_star' | 'reset';
export type IsosurfaceSignMode = 'positive' | 'negative' | 'both';
export type VolumeRenderMode = 'isosurface' | 'volume' | 'both';
export type VolumeColormap = 'viridis' | 'grayscale' | 'inferno' | 'plasma' | 'coolwarm'
    | 'hot' | 'magma' | 'cividis' | 'turbo' | 'rdylbu';
export type LlmProvider = 'openai' | 'deepseek' | 'claude' | 'gemini' | 'ollama';
export type ExportFileFormat = 'POSCAR' | 'VASP' | 'LAMMPS' | 'QE';
export type ExportImageBackground = 'transparent' | 'white' | 'black' | 'default';

export type IpcErrorCode = 'invalid_argument' | 'io_error' | 'lock_poisoned'
    | 'not_in_tauri' | 'state_busy' | 'parse_error' | 'render_error' | 'internal_error';

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
    state_changed: StateChangedPayload;
    'tauri://drag-drop': TauriDragPayload;
    'tauri://drag-enter': TauriDragPayload;
    'tauri://drag-leave': null | undefined;
    'tauri://file-drop': { paths: string[] };
    'tauri://file-drop-cancelled': null | undefined;
    'tauri://file-drop-hover': { paths: string[] };
    undo_stack_changed: { can_undo: boolean; can_redo: boolean };
    view_projection_changed: { is_perspective: boolean };
    volumetric_loaded: VolumetricInfo;
}

export interface IpcCommandContract {
    add_atom: { args: { elementSymbol: string; atomicNumber: number; fractPos: [number, number, number] }; result: null };
    add_measurement: { args: { indices: number[] }; result: MeasurementOverlay };
    apply_cell_standardize: { args: { toPrimitive: boolean }; result: null };
    apply_niggli_reduce: { args: undefined; result: null };
    apply_slab: { args: { miller: [number, number, number]; layers: number; vacuumA: number }; result: null };
    apply_supercell: { args: { matrix: [[number, number, number], [number, number, number], [number, number, number]] }; result: null };
    begin_atom_drag: { args: { indices: number[] }; result: string };
    cancel_atom_drag: { args: { sessionId: string }; result: null };
    check_api_key_status: { args: { providerType: LlmProvider }; result: boolean };
    clear_measurements: { args: undefined; result: null };
    clear_wannier: { args: undefined; result: null };
    commit_atom_drag: { args: { sessionId: string }; result: null };
    compute_brillouin_zone: { args: undefined; result: BzInfo };
    delete_atoms: { args: { indices: number[] }; result: null };
    export_file: { args: { format: ExportFileFormat; path: string }; result: null };
    export_image: { args: { path: string; width: number; height: number; bgMode: ExportImageBackground }; result: null };
    generate_kpath_text: { args: { npoints: number }; result: KPathText };
    get_bond_analysis: { args: { thresholdFactor?: number | null }; result: BondAnalysisResult };
    get_bz_label_positions: { args: { width: number; height: number }; result: ScreenLabel[] };
    get_crystal_state: { args: undefined; result: CrystalState };
    get_kpath_info: { args: undefined; result: KPathInfo };
    get_measurement_labels_screen: { args: { width: number; height: number }; result: ScreenLabel[] };
    get_measurements: { args: undefined; result: MeasurementOverlay[] };
    get_settings: { args: undefined; result: AppSettingsDto };
    get_volumetric_info: { args: undefined; result: VolumetricInfo | null };
    llm_chat: { args: { userMessage: string; selectedIndices?: number[] | null }; result: string };
    llm_configure: { args: { providerType: LlmProvider; apiKey: string; model: string }; result: null };
    llm_execute_command: { args: { commandJson: string }; result: null };
    load_axsf_phonon: { args: { path: string }; result: PhononModeSummary[] };
    load_cif_file: { args: { path: string }; result: null };
    load_phonon: { args: { path: string }; result: PhononModeSummary[] };
    load_phonon_interactive: { args: { scfIn: string; scfOut: string; modes: string }; result: PhononModeSummary[] };
    load_volumetric_file: { args: { path: string }; result: VolumetricInfo };
    load_wannier_hr: { args: { path: string }; result: WannierInfo };
    pan_camera: { args: { dx: number; dy: number }; result: null };
    pick_atom: { args: { x: number; y: number; screenW: number; screenH: number }; result: number | null };
    preview_slab: { args: { miller: [number, number, number]; layers: number; vacuumA: number }; result: CrystalState };
    preview_supercell: { args: { expansion: [number, number, number, number, number, number, number, number, number] }; result: CrystalState };
    redo: { args: undefined; result: null };
    reset_camera: { args: undefined; result: null };
    restore_unitcell: { args: undefined; result: null };
    rotate_camera: { args: { dx: number; dy: number }; result: null };
    set_bz_scale: { args: { scale: number }; result: null };
    set_camera_projection: { args: { isPerspective: boolean }; result: null };
    set_camera_view_axis: { args: { axis: CameraAxis }; result: null };
    set_isosurface_color: { args: { color: [number, number, number, number] }; result: null };
    set_isosurface_opacity: { args: { opacity: number }; result: null };
    set_isosurface_sign_mode: { args: { mode: IsosurfaceSignMode }; result: null };
    set_isovalue: { args: { value: number }; result: null };
    set_phonon_mode: { args: { modeIndex?: number | null }; result: null };
    set_phonon_phase: { args: { phase: number; amplitude?: number | null }; result: null };
    set_render_flags: { args: { showCell: boolean; showBonds: boolean }; result: null };
    set_volume_colormap: { args: { mode: VolumeColormap }; result: null };
    set_volume_density_cutoff: { args: { cutoff: number }; result: null };
    set_volume_opacity_range: { args: { min: number; max: number; opacityScale: number }; result: null };
    set_volume_render_mode: { args: { mode: VolumeRenderMode }; result: null };
    set_wannier_orbital: { args: { orbIdx: number; active: boolean }; result: null };
    set_wannier_r_shell: { args: { shellIdx: number; active: boolean }; result: null };
    set_wannier_t_min: { args: { tMin: number }; result: null };
    shift_termination: { args: { targetLayerIdx: number; layerToleranceA?: number | null }; result: number };
    substitute_atoms: { args: { indices: number[]; newElementSymbol: string; newAtomicNumber: number }; result: null };
    toggle_bz_display: { args: { show: boolean }; result: null };
    toggle_hopping_display: { args: { show: boolean }; result: null };
    toggle_wannier_onsite: { args: { show: boolean }; result: null };
    translate_atoms_screen: { args: { indices: number[]; dx: number; dy: number }; result: null };
    undo: { args: undefined; result: null };
    update_atom_drag: { args: { sessionId: string; dx: number; dy: number }; result: null };
    update_lattice_params: { args: { a: number; b: number; c: number; alpha: number; beta: number; gamma: number }; result: null };
    update_selection: { args: { indices: number[] }; result: null };
    update_settings: { args: { newSettings: AppSettingsDto }; result: null };
    update_viewport_size: { args: { width: number; height: number }; result: null };
    write_text_file: { args: { path: string; content: string }; result: null };
    zoom_camera: { args: { delta: number }; result: null };
}

export type TypedIpcCommand = keyof IpcCommandContract;
export type TypedIpcEvent = keyof IpcEventContract;

type MissingIpcContract = Exclude<IpcCommandName, TypedIpcCommand>;
type ExtraIpcContract = Exclude<TypedIpcCommand, IpcCommandName>;
type CompleteIpcCommandContract = [MissingIpcContract, ExtraIpcContract] extends [never, never]
    ? true
    : never;
const COMPLETE_IPC_COMMAND_CONTRACT: CompleteIpcCommandContract = true;
void COMPLETE_IPC_COMMAND_CONTRACT;

export type IpcArgs<Command extends TypedIpcCommand> = IpcCommandContract[Command]['args'];

export type IpcResult<Command extends TypedIpcCommand> = IpcCommandContract[Command]['result'];

export function is_camera_axis(value: string): value is CameraAxis {
    return value === 'a' || value === 'b' || value === 'c' || value === 'a_star'
        || value === 'b_star' || value === 'c_star' || value === 'reset';
}

export function is_isosurface_sign_mode(value: string): value is IsosurfaceSignMode {
    return value === 'positive' || value === 'negative' || value === 'both';
}

export function is_volume_render_mode(value: string): value is VolumeRenderMode {
    return value === 'isosurface' || value === 'volume' || value === 'both';
}

export function is_volume_colormap(value: string): value is VolumeColormap {
    return value === 'viridis' || value === 'grayscale' || value === 'inferno'
        || value === 'plasma' || value === 'coolwarm' || value === 'hot'
        || value === 'magma' || value === 'cividis' || value === 'turbo'
        || value === 'rdylbu';
}

export function is_llm_provider(value: string): value is LlmProvider {
    return value === 'openai' || value === 'deepseek' || value === 'claude'
        || value === 'gemini' || value === 'ollama';
}

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

function is_measurement(value: unknown): value is MeasurementOverlay {
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

function is_kpath_info(value: unknown): value is KPathInfo {
    if (!is_record(value) || !Array.isArray(value.points) || !Array.isArray(value.segments)) {
        return false;
    }
    const labels = new Set<string>();
    for (const point of value.points) {
        if (!is_record(point) || typeof point.label !== 'string' || point.label.length === 0
            || !is_number_triplet(point.coord_frac) || labels.has(point.label)) return false;
        labels.add(point.label);
    }
    return value.segments.every((segment) => Array.isArray(segment)
        && segment.length >= 2
        && segment.every((label) => typeof label === 'string'
            && label.length > 0 && labels.has(label)));
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

const is_null = (value: unknown): value is null => value === null;
const is_integer = (value: unknown): value is number => Number.isSafeInteger(value);
const is_measurements = (value: unknown): value is MeasurementOverlay[] => Array.isArray(value)
    && value.every(is_measurement);
const is_kpath_text = (value: unknown): value is { qe: string; vasp: string } => is_record(value)
    && typeof value.qe === 'string' && typeof value.vasp === 'string';

const IPC_RESULT_VALIDATORS: {
    [Command in TypedIpcCommand]: (value: unknown) => boolean;
} = {
    add_atom: is_null,
    add_measurement: is_measurement,
    apply_cell_standardize: is_null,
    apply_niggli_reduce: is_null,
    apply_slab: is_null,
    apply_supercell: is_null,
    begin_atom_drag: (value) => typeof value === 'string',
    cancel_atom_drag: is_null,
    check_api_key_status: (value) => typeof value === 'boolean',
    clear_measurements: is_null,
    clear_wannier: is_null,
    commit_atom_drag: is_null,
    compute_brillouin_zone: is_bz_info,
    delete_atoms: is_null,
    export_file: is_null,
    export_image: is_null,
    generate_kpath_text: is_kpath_text,
    get_bond_analysis: is_bond_analysis,
    get_bz_label_positions: is_screen_labels,
    get_crystal_state: is_crystal_state,
    get_kpath_info: is_kpath_info,
    get_measurement_labels_screen: is_screen_labels,
    get_measurements: is_measurements,
    get_settings: is_app_settings,
    get_volumetric_info: (value) => value === null || is_volumetric_info(value),
    llm_chat: (value) => typeof value === 'string',
    llm_configure: is_null,
    llm_execute_command: is_null,
    load_axsf_phonon: is_phonon_modes,
    load_cif_file: is_null,
    load_phonon: is_phonon_modes,
    load_phonon_interactive: is_phonon_modes,
    load_volumetric_file: is_volumetric_info,
    load_wannier_hr: is_wannier_info,
    pan_camera: is_null,
    pick_atom: (value) => value === null || is_nonnegative_integer(value),
    preview_slab: is_crystal_state,
    preview_supercell: is_crystal_state,
    redo: is_null,
    reset_camera: is_null,
    restore_unitcell: is_null,
    rotate_camera: is_null,
    set_bz_scale: is_null,
    set_camera_projection: is_null,
    set_camera_view_axis: is_null,
    set_isosurface_color: is_null,
    set_isosurface_opacity: is_null,
    set_isosurface_sign_mode: is_null,
    set_isovalue: is_null,
    set_phonon_mode: is_null,
    set_phonon_phase: is_null,
    set_render_flags: is_null,
    set_volume_colormap: is_null,
    set_volume_density_cutoff: is_null,
    set_volume_opacity_range: is_null,
    set_volume_render_mode: is_null,
    set_wannier_orbital: is_null,
    set_wannier_r_shell: is_null,
    set_wannier_t_min: is_null,
    shift_termination: is_integer,
    substitute_atoms: is_null,
    toggle_bz_display: is_null,
    toggle_hopping_display: is_null,
    toggle_wannier_onsite: is_null,
    translate_atoms_screen: is_null,
    undo: is_null,
    update_atom_drag: is_null,
    update_lattice_params: is_null,
    update_selection: is_null,
    update_settings: is_null,
    update_viewport_size: is_null,
    write_text_file: is_null,
    zoom_camera: is_null,
};

export function validate_ipc_result<Command extends TypedIpcCommand>(
    command: Command,
    value: unknown
): IpcResult<Command> {
    if (IPC_RESULT_VALIDATORS[command](value)) return value as IpcResult<Command>;
    throw new Error(`Invalid IPC response for ${command}`);
}

export function normalize_ipc_error(value: unknown): IpcException {
    if (value instanceof IpcException) return value;
    if (is_record(value) && typeof value.code === 'string'
        && ['invalid_argument', 'io_error', 'lock_poisoned', 'not_in_tauri', 'state_busy', 'parse_error', 'render_error', 'internal_error'].includes(value.code)
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
    if (event === 'state_changed' && is_record(value) && is_nonnegative_integer(value.version)) return value as IpcEventContract[Event];
    if (event === 'tauri://drag-leave' && (value === null || value === undefined)) return value as IpcEventContract[Event];
    if (event === 'tauri://file-drop-cancelled' && (value === null || value === undefined)) return value as IpcEventContract[Event];
    if (event === 'menu-action' && typeof value === 'string') return value as IpcEventContract[Event];
    if ((event === 'tauri://drag-enter' || event === 'tauri://drag-drop') && is_record(value)
        && Array.isArray(value.paths) && value.paths.every((path) => typeof path === 'string')
        && is_record(value.position) && is_finite_number(value.position.x) && is_finite_number(value.position.y)) return value as IpcEventContract[Event];
    if (event === 'tauri://file-drop' && is_record(value) && Array.isArray(value.paths) && value.paths.every((path) => typeof path === 'string')) return value as IpcEventContract[Event];
    if (event === 'tauri://file-drop-hover' && is_record(value) && Array.isArray(value.paths)
        && value.paths.every((path) => typeof path === 'string')) return value as IpcEventContract[Event];
    if (event === 'undo_stack_changed' && is_record(value) && typeof value.can_undo === 'boolean' && typeof value.can_redo === 'boolean') return value as IpcEventContract[Event];
    if (event === 'view_projection_changed' && is_record(value) && typeof value.is_perspective === 'boolean') return value as IpcEventContract[Event];
    if (event === 'volumetric_loaded' && is_volumetric_info(value)) return value as IpcEventContract[Event];
    throw new Error(`Invalid IPC event payload for ${event}`);
}
