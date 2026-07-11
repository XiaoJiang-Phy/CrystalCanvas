use tauri::State;

use crate::ipc::{IpcError, IpcResult};

#[derive(serde::Serialize)]
pub struct WannierInfo {
    pub num_wann: usize,
    pub r_shells: Vec<[i32; 3]>,
    pub t_max: f64,
}

fn validate_wannier_t_min(t_min: f64, t_max: f64) -> IpcResult<()> {
    if !t_min.is_finite() || !t_max.is_finite() || t_min < 0.0 || t_min > t_max {
        return Err(IpcError::invalid_argument(format!(
            "Wannier threshold {} is outside [0, {}]",
            t_min, t_max
        )));
    }
    Ok(())
}

fn validate_wannier_index(kind: &str, index: usize, len: usize) -> IpcResult<()> {
    if index >= len {
        return Err(IpcError::invalid_argument(format!(
            "Wannier {} index {} is outside [0, {})",
            kind, index, len
        )));
    }
    Ok(())
}

struct WannierScene {
    hoppings: Vec<crate::renderer::instance::BondInstance>,
    atoms: Vec<crate::renderer::instance::AtomInstance>,
}

fn build_wannier_scene(
    cs: &crate::crystal_state::CrystalState,
    settings: &crate::settings::AppSettings,
) -> WannierScene {
    let hoppings = if let Some(overlay) = &cs.wannier_overlay {
        crate::renderer::instance::build_hopping_instances(
            &overlay.visible_hoppings,
            overlay.hr_data.t_max,
        )
    } else {
        Vec::new()
    };
    WannierScene {
        hoppings,
        atoms: crate::wannier::build_atoms_with_ghosts(cs, settings),
    }
}

#[derive(Clone, Copy)]
enum WannierChange {
    Threshold(f64),
    RShell { index: usize, active: bool },
    Orbital { index: usize, active: bool },
    Onsite(bool),
}

impl WannierChange {
    fn validate(self, overlay: &crate::wannier::WannierOverlay) -> IpcResult<()> {
        match self {
            Self::Threshold(value) => validate_wannier_t_min(value, overlay.hr_data.t_max),
            Self::RShell { index, .. } => {
                validate_wannier_index("R-shell", index, overlay.active_r_shells.len())
            }
            Self::Orbital { index, .. } => {
                validate_wannier_index("orbital", index, overlay.active_orbitals.len())
            }
            Self::Onsite(_) => Ok(()),
        }
    }

    fn apply(self, overlay: &mut crate::wannier::WannierOverlay) -> Self {
        match self {
            Self::Threshold(value) => {
                Self::Threshold(std::mem::replace(&mut overlay.t_min_threshold, value))
            }
            Self::RShell { index, active } => Self::RShell {
                index,
                active: std::mem::replace(&mut overlay.active_r_shells[index], active),
            },
            Self::Orbital { index, active } => Self::Orbital {
                index,
                active: std::mem::replace(&mut overlay.active_orbitals[index], active),
            },
            Self::Onsite(show) => Self::Onsite(std::mem::replace(&mut overlay.show_onsite, show)),
        }
    }
}

fn apply_wannier_change(
    change: WannierChange,
    crystal_state: &State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    settings_state: &State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
    renderer_state: &State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    let mut cs = crystal_state
        .lock()
        .map_err(|e| IpcError::lock(e.to_string()))?;
    change.validate(
        cs.wannier_overlay.as_ref()
            .ok_or_else(|| IpcError::invalid_argument("No Wannier data loaded"))?,
    )?;
    let settings = settings_state
        .lock()
        .map_err(|e| IpcError::lock(e.to_string()))?;
    let lattice_col_major = cs.get_lattice_col_major();
    let cs_ref = &mut *cs;
    let cart_positions = &cs_ref.cart_positions;
    let overlay = cs_ref.wannier_overlay.as_mut()
        .ok_or_else(|| IpcError::invalid_argument("No Wannier data loaded"))?;
    let rollback = change.apply(overlay);
    if let Err(error) = overlay.filter_and_rebuild(&lattice_col_major, cart_positions) {
        rollback.apply(overlay);
        return Err(IpcError::invalid_argument(error));
    }
    let scene = build_wannier_scene(&cs, &settings);
    let mut renderer = match renderer_state.lock() {
        Ok(renderer) => renderer,
        Err(error) => {
            let cs_ref = &mut *cs;
            let cart_positions = &cs_ref.cart_positions;
            let overlay = cs_ref.wannier_overlay.as_mut()
                .ok_or_else(|| IpcError::invalid_argument("No Wannier data loaded"))?;
            rollback.apply(overlay);
            overlay.filter_and_rebuild(&lattice_col_major, cart_positions)
                .map_err(|rollback_error| IpcError::from(format!(
                    "Failed to restore Wannier state after renderer lock failure: {}",
                    rollback_error
                )))?;
            return Err(IpcError::lock(error.to_string()));
        }
    };
    renderer.update_hoppings(&scene.hoppings);
    renderer.update_atoms(&scene.atoms);
    Ok(())
}

#[tauri::command]
pub fn load_wannier_hr(
    path: String,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> IpcResult<WannierInfo> {
    log::info!("load_wannier_hr: {}", path);
    let hr_data = crate::io::wannier_hr_parser::parse_wannier_hr(&path)
        .map_err(IpcError::parse)?;
    let mut cs = crystal_state
        .lock()
        .map_err(|e| IpcError::lock(e.to_string()))?;

    if cs.num_atoms() < hr_data.num_wann {
        return Err(IpcError::invalid_argument(format!(
            "Crystal structure has {} atoms, but Wannier data has {} orbitals",
            cs.num_atoms(),
            hr_data.num_wann
        )));
    }

    let lattice_col_major = cs.get_lattice_col_major();
    // WannierOverlay::new naturally populates visible_hoppings with defaults
    let overlay = crate::wannier::WannierOverlay::new(
        hr_data,
        &lattice_col_major,
        &cs.cart_positions,
    )
    .map_err(IpcError::invalid_argument)?;
    let settings = settings_state
        .lock()
        .map_err(|e| IpcError::lock(e.to_string()))?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|e| IpcError::lock(e.to_string()))?;
    let instances = crate::renderer::instance::build_hopping_instances(&overlay.visible_hoppings, overlay.hr_data.t_max);
    renderer.update_hoppings(&instances);
    renderer.show_hoppings = true;
    renderer.show_bonds = false;

    // Extract WannierInfo before moving overlay into cs
    let num_wann = overlay.hr_data.num_wann;
    let r_shells = overlay.hr_data.r_shells.clone();
    let t_max = overlay.hr_data.t_max;

    cs.wannier_overlay = Some(overlay);
    let atoms = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
    renderer.update_atoms(&atoms);

    Ok(WannierInfo {
        num_wann,
        r_shells,
        t_max,
    })
}

#[tauri::command]
pub fn set_wannier_t_min(
    t_min: f64,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> IpcResult<()> {
    log::info!("set_wannier_t_min: {}", t_min);
    apply_wannier_change(
        WannierChange::Threshold(t_min),
        &crystal_state,
        &settings_state,
        &renderer_state,
    )
}

#[tauri::command]
pub fn set_wannier_r_shell(
    shell_idx: usize,
    active: bool,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> IpcResult<()> {
    log::info!("set_wannier_r_shell: {} -> {}", shell_idx, active);
    apply_wannier_change(
        WannierChange::RShell { index: shell_idx, active },
        &crystal_state,
        &settings_state,
        &renderer_state,
    )
}

#[tauri::command]
pub fn set_wannier_orbital(
    orb_idx: usize,
    active: bool,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> IpcResult<()> {
    log::info!("set_wannier_orbital: {} -> {}", orb_idx, active);
    apply_wannier_change(
        WannierChange::Orbital { index: orb_idx, active },
        &crystal_state,
        &settings_state,
        &renderer_state,
    )
}

#[tauri::command]
pub fn toggle_wannier_onsite(
    show: bool,
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> IpcResult<()> {
    log::info!("toggle_wannier_onsite: {}", show);
    apply_wannier_change(
        WannierChange::Onsite(show),
        &crystal_state,
        &settings_state,
        &renderer_state,
    )
}

#[tauri::command]
pub fn toggle_hopping_display(
    show: bool,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
) -> IpcResult<()> {
    log::info!("toggle_hopping_display: {}", show);
    let mut renderer = renderer_state
        .lock()
        .map_err(|e| IpcError::lock(e.to_string()))?;
    renderer.show_hoppings = show;
    Ok(())
}

#[tauri::command]
pub fn clear_wannier(
    crystal_state: State<'_, std::sync::Mutex<crate::crystal_state::CrystalState>>,
    renderer_state: State<'_, std::sync::Mutex<crate::renderer::renderer::Renderer>>,
    settings_state: State<'_, std::sync::Mutex<crate::settings::AppSettings>>,
) -> IpcResult<()> {
    log::info!("clear_wannier");
    let mut cs = crystal_state
        .lock()
        .map_err(|e| IpcError::lock(e.to_string()))?;
    let settings = settings_state
        .lock()
        .map_err(|e| IpcError::lock(e.to_string()))?;
    let mut renderer = renderer_state
        .lock()
        .map_err(|e| IpcError::lock(e.to_string()))?;
    cs.wannier_overlay = None;
    renderer.update_hoppings(&[]);
    let atoms = crate::wannier::build_atoms_with_ghosts(&cs, &settings);
    renderer.update_atoms(&atoms);
    renderer.show_hoppings = false;
    renderer.show_bonds = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_wannier_index, validate_wannier_t_min, WannierChange, WannierInfo};
    use crate::io::wannier_hr_parser::parse_wannier_hr;

    #[test]
    fn wannier_info_serializes_r_shells_as_integer_triplets() {
        let info = WannierInfo {
            num_wann: 2,
            r_shells: vec![[0, 0, 0], [1, -2, 3]],
            t_max: 1.25,
        };

        let value = serde_json::to_value(info).expect("WannierInfo must serialize");
        assert_eq!(
            value["r_shells"],
            serde_json::json!([[0, 0, 0], [1, -2, 3]])
        );
    }

    #[test]
    fn wannier_threshold_rejects_non_finite_negative_and_out_of_range_values() {
        assert!(validate_wannier_t_min(f64::NAN, 1.0).is_err());
        assert!(validate_wannier_t_min(f64::INFINITY, 1.0).is_err());
        assert!(validate_wannier_t_min(-0.1, 1.0).is_err());
        assert!(validate_wannier_t_min(1.1, 1.0).is_err());
        assert!(validate_wannier_t_min(0.5, 1.0).is_ok());
    }

    #[test]
    fn wannier_indices_reject_out_of_range_values() {
        assert!(validate_wannier_index("R-shell", 2, 2).is_err());
        assert!(validate_wannier_index("orbital", 1, 2).is_ok());
    }

    #[test]
    fn wannier_changes_produce_zero_copy_inverse_operations() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .join("tests/fixtures/graphene_hr.dat");
        let hr_data = parse_wannier_hr(path.to_str().unwrap()).unwrap();
        let lattice = [2.46, 0.0, 0.0, -1.23, 2.13, 0.0, 0.0, 0.0, 10.0];
        let atoms = [[0.0, 0.0, 0.0], [1.23, 0.71, 0.0]];
        let mut overlay = crate::wannier::WannierOverlay::new(hr_data, &lattice, &atoms).unwrap();

        for change in [
            WannierChange::Threshold(0.5),
            WannierChange::RShell { index: 0, active: false },
            WannierChange::Orbital { index: 0, active: false },
            WannierChange::Onsite(true),
        ] {
            let rollback = change.apply(&mut overlay);
            rollback.apply(&mut overlay);
        }

        assert_eq!(overlay.t_min_threshold, 0.01);
        assert!(overlay.active_r_shells[0]);
        assert!(overlay.active_orbitals[0]);
        assert!(!overlay.show_onsite);
    }
}
