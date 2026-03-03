//! [Overview: Rust backend state model for user-configurable rendering settings and persistence.]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub atom_scale: f32,
    pub bond_tolerance: f32,
    pub bond_radius: f32,
    pub bond_color: [f32; 4],
    pub custom_atom_colors: HashMap<String, [f32; 4]>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            atom_scale: 1.0,
            bond_tolerance: 0.45,
            bond_radius: 0.08,
            bond_color: [0.65, 0.65, 0.65, 1.0],
            custom_atom_colors: HashMap::new(),
        }
    }
}

impl AppSettings {
    pub fn get_config_path(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
        use tauri::Manager;
        let mut path = app_handle.path().app_config_dir().map_err(|e| e.to_string())?;
        path.push("settings.json");
        Ok(path)
    }

    pub fn load(app_handle: &tauri::AppHandle) -> Self {
        if let Ok(path) = Self::get_config_path(app_handle)
            && let Ok(data) = std::fs::read_to_string(&path)
            && let Ok(settings) = serde_json::from_str(&data)
        {
            return settings;
        }
        Self::default()
    }

    pub fn save(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        let path = Self::get_config_path(app_handle)?;
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, data).map_err(|e| e.to_string())?;
        Ok(())
    }
}
