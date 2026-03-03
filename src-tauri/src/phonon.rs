//! Phonon eigenvector data and animation logic (M10)
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Serialize;

/// A single phonon mode at a specific q-point.
#[derive(Clone, Debug, Serialize)]
pub struct PhononMode {
    /// Frequency in cm⁻¹
    pub frequency_cm1: f64,
    /// Whether this frequency is imaginary (< 0 in QE convention)
    pub is_imaginary: bool,
    /// Eigenvector per atom: displacement in mass-weighted coordinates
    /// Shape: [n_atoms][3] (normalized)
    pub eigenvectors: Vec<[f64; 3]>,
}

/// Summary of a phonon mode (for frontend display).
#[derive(Clone, Debug, Serialize)]
pub struct PhononModeSummary {
    pub index: usize,
    pub frequency_cm1: f64,
    pub is_imaginary: bool,
}

/// Phonon data container (Gamma-point only for now).
#[derive(Clone, Debug, Serialize, Default)]
pub struct PhononData {
    pub modes: Vec<PhononMode>,
    pub n_atoms: usize,
}

impl PhononData {
    /// Get mode summaries for frontend display.
    pub fn summaries(&self) -> Vec<PhononModeSummary> {
        self.modes
            .iter()
            .enumerate()
            .map(|(i, m)| PhononModeSummary {
                index: i,
                frequency_cm1: m.frequency_cm1,
                is_imaginary: m.is_imaginary,
            })
            .collect()
    }
}

/// Parse a QE `dynmat.mold` file (Molden format) to extract phonon eigenvectors.
///
/// Format:
/// ```text
/// [Molden Format]
/// [FREQ]
///   freq_1      (cm⁻¹)
///   ...
/// [FR-COORD]
///   element  x  y  z    (Bohr)
///   ...
/// [FR-NORM-COORD]
///  vibration     1
///   ex1  ey1  ez1   (per atom, normalized)
///   ...
///  vibration     2
///   ...
/// ```
pub fn parse_molden_phonon(content: &str) -> Result<PhononData, String> {
    let mut frequencies: Vec<f64> = Vec::new();
    let mut eigenvectors: Vec<Vec<[f64; 3]>> = Vec::new();

    #[derive(PartialEq)]
    enum Section {
        None,
        Freq,
        FrCoord,
        FrNormCoord,
    }

    let mut section = Section::None;
    let mut current_mode_vecs: Vec<[f64; 3]> = Vec::new();
    let mut coord_count: usize = 0;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Section headers
        if trimmed == "[FREQ]" {
            section = Section::Freq;
            continue;
        } else if trimmed == "[FR-COORD]" {
            section = Section::FrCoord;
            coord_count = 0;
            continue;
        } else if trimmed == "[FR-NORM-COORD]" {
            section = Section::FrNormCoord;
            continue;
        } else if trimmed.starts_with('[') {
            // Unknown section, skip
            section = Section::None;
            continue;
        }

        match section {
            Section::Freq => {
                if let Ok(freq) = trimmed.parse::<f64>() {
                    frequencies.push(freq);
                }
            }
            Section::FrCoord => {
                // Count atoms from coordinate section
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 4 {
                    coord_count += 1;
                }
            }
            Section::FrNormCoord => {
                if trimmed.starts_with("vibration") {
                    // Save previous mode if any
                    if !current_mode_vecs.is_empty() {
                        eigenvectors.push(current_mode_vecs.clone());
                    }
                    current_mode_vecs = Vec::new();
                } else {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let x = parts[0]
                            .parse::<f64>()
                            .map_err(|_| "Invalid eigenvector x".to_string())?;
                        let y = parts[1]
                            .parse::<f64>()
                            .map_err(|_| "Invalid eigenvector y".to_string())?;
                        let z = parts[2]
                            .parse::<f64>()
                            .map_err(|_| "Invalid eigenvector z".to_string())?;
                        current_mode_vecs.push([x, y, z]);
                    }
                }
            }
            Section::None => {}
        }
    }

    // Save last mode
    if !current_mode_vecs.is_empty() {
        eigenvectors.push(current_mode_vecs);
    }

    let n_atoms = coord_count;

    if frequencies.is_empty() {
        return Err("No frequencies found in Molden file".to_string());
    }
    if frequencies.len() != eigenvectors.len() {
        return Err(format!(
            "Mismatch: {} frequencies but {} eigenvector sets",
            frequencies.len(),
            eigenvectors.len()
        ));
    }

    let modes: Vec<PhononMode> = frequencies
        .into_iter()
        .zip(eigenvectors.into_iter())
        .map(|(freq, evecs)| PhononMode {
            frequency_cm1: freq,
            is_imaginary: freq < 0.0,
            eigenvectors: evecs,
        })
        .collect();

    Ok(PhononData { modes, n_atoms })
}

/// Parse a QE `dynmat.dat` file (from dynmat.x output).
///
/// Format:
/// ```text
///  diagonalizing the dynamical matrix ...
///
///  q =      0.0000     0.0000     0.0000
///  ****...
///      freq (  1) =   freq_THz [THz] =   freq_cm [cm-1]
///  (  re_x im_x   re_y im_y   re_z im_z  )   <- per atom
///  ...
/// ```
pub fn parse_dynmat_dat(content: &str) -> Result<PhononData, String> {
    let mut frequencies: Vec<f64> = Vec::new();
    let mut eigenvectors: Vec<Vec<[f64; 3]>> = Vec::new();
    let mut current_vecs: Vec<[f64; 3]> = Vec::new();
    let mut in_mode = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("freq") {
            // Save previous mode
            if !current_vecs.is_empty() {
                eigenvectors.push(current_vecs.clone());
            }
            current_vecs = Vec::new();
            in_mode = true;

            // Parse cm-1 value: "freq (  1) =  ... [THz] =  ... [cm-1]"
            if let Some(cm_pos) = trimmed.find("[cm-1]") {
                let before_cm = &trimmed[..cm_pos].trim();
                // Find the last '=' before [cm-1]
                if let Some(eq_pos) = before_cm.rfind('=') {
                    let freq_str = before_cm[eq_pos + 1..].trim();
                    if let Ok(freq) = freq_str.parse::<f64>() {
                        frequencies.push(freq);
                    }
                }
            }
        } else if in_mode && trimmed.starts_with('(') && trimmed.ends_with(')') {
            // Parse eigenvector line: (  re_x  im_x   re_y  im_y   re_z  im_z  )
            let inner = &trimmed[1..trimmed.len() - 1];
            let parts: Vec<f64> = inner
                .split_whitespace()
                .filter_map(|s| s.parse::<f64>().ok())
                .collect();
            // Take only real parts (indices 0, 2, 4)
            if parts.len() >= 6 {
                current_vecs.push([parts[0], parts[2], parts[4]]);
            }
        } else if trimmed.starts_with("***") {
            // Boundary marker, end of data block
            if !current_vecs.is_empty() {
                eigenvectors.push(current_vecs.clone());
                current_vecs = Vec::new();
            }
            in_mode = false;
        }
    }

    // Save last mode if not terminated by ***
    if !current_vecs.is_empty() {
        eigenvectors.push(current_vecs);
    }

    if frequencies.is_empty() {
        return Err("No frequencies found in dynmat.dat".to_string());
    }
    if frequencies.len() != eigenvectors.len() {
        return Err(format!(
            "Mismatch: {} frequencies but {} eigenvector sets",
            frequencies.len(),
            eigenvectors.len()
        ));
    }

    let n_atoms = eigenvectors.first().map_or(0, |v| v.len());

    let modes: Vec<PhononMode> = frequencies
        .into_iter()
        .zip(eigenvectors.into_iter())
        .map(|(freq, evecs)| PhononMode {
            frequency_cm1: freq,
            is_imaginary: freq < 0.0,
            eigenvectors: evecs,
        })
        .collect();

    Ok(PhononData { modes, n_atoms })
}

/// Auto-detect file format and parse phonon data.
pub fn parse_phonon_file(path: &str) -> Result<PhononData, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read phonon file '{}': {}", path, e))?;

    // Auto-detect format
    if content.contains("[Molden Format]") || content.contains("[FREQ]") {
        parse_molden_phonon(&content)
    } else if content.contains("diagonalizing") || content.contains("freq (") {
        parse_dynmat_dat(&content)
    } else {
        Err("Unrecognized phonon file format. Supported: Molden (.mold), QE dynmat (.dat)".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_molden_ceo2() {
        let content = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/data/CeO/dynmat.mold"),
        )
        .expect("Failed to read dynmat.mold");

        let data = parse_molden_phonon(&content).expect("Failed to parse Molden");

        assert_eq!(data.n_atoms, 3, "CeO2 has 3 atoms in primitive cell");
        assert_eq!(data.modes.len(), 9, "3 atoms × 3 = 9 modes");

        // Acoustic modes (1-3) should be ~0 cm⁻¹
        for mode in &data.modes[0..3] {
            assert!(
                mode.frequency_cm1.abs() < 1.0,
                "Acoustic mode should be ~0, got {}",
                mode.frequency_cm1
            );
        }

        // T1u optical modes (4-6) should be ~256.57 cm⁻¹
        for mode in &data.modes[3..6] {
            assert!(
                (mode.frequency_cm1 - 256.57).abs() < 1.0,
                "T1u mode should be ~256.57, got {}",
                mode.frequency_cm1
            );
        }

        // T2g Raman modes (7-9) should be ~416.42 cm⁻¹
        for mode in &data.modes[6..9] {
            assert!(
                (mode.frequency_cm1 - 416.42).abs() < 1.0,
                "T2g mode should be ~416.42, got {}",
                mode.frequency_cm1
            );
        }

        // Each mode should have eigenvectors for 3 atoms
        for mode in &data.modes {
            assert_eq!(
                mode.eigenvectors.len(),
                3,
                "Each mode should have 3 eigenvectors"
            );
        }
    }

    #[test]
    fn test_parse_dynmat_dat_ceo2() {
        let content = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/data/CeO/CeO.dynmat.dat"),
        )
        .expect("Failed to read CeO.dynmat.dat");

        let data = parse_dynmat_dat(&content).expect("Failed to parse dynmat.dat");

        assert_eq!(data.n_atoms, 3, "CeO2 has 3 atoms");
        assert_eq!(data.modes.len(), 9, "Should have 9 modes");

        // Check T1u ~ 256.57 cm⁻¹
        assert!(
            (data.modes[3].frequency_cm1 - 256.57).abs() < 1.0,
            "Mode 4 should be ~256.57, got {}",
            data.modes[3].frequency_cm1
        );
    }

    #[test]
    fn test_auto_detect_format() {
        let mold_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/data/CeO/dynmat.mold");
        let dat_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/data/CeO/CeO.dynmat.dat");

        let mold_data = parse_phonon_file(mold_path).expect("Failed to auto-parse .mold");
        let dat_data = parse_phonon_file(dat_path).expect("Failed to auto-parse .dat");

        assert_eq!(mold_data.modes.len(), 9);
        assert_eq!(dat_data.modes.len(), 9);
    }
}
