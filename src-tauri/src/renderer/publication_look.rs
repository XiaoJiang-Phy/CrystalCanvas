//! Fixed publication-only presentation profiles.

use bytemuck::{Pod, Zeroable};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationLookProfileId {
    ScientificGloss,
    Studio,
    Unlit,
}

impl PublicationLookProfileId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScientificGloss => "scientific_gloss",
            Self::Studio => "studio",
            Self::Unlit => "unlit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PublicationBondColorMode {
    Uniform,
    ByElements,
}

impl PublicationBondColorMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uniform => "uniform",
            Self::ByElements => "by_elements",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ToneMapping {
    Disabled,
    Aces,
}

impl ToneMapping {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Aces => "aces_fitted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DepthEnhancement {
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationCellLineBackground {
    Transparent,
    White,
    Black,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PublicationCellLineStyle {
    pub cell_line_color_rgba: [f32; 4],
}

impl PublicationCellLineStyle {
    pub const fn for_background(background: PublicationCellLineBackground) -> Self {
        match background {
            PublicationCellLineBackground::White => Self {
                cell_line_color_rgba: [0.18, 0.22, 0.28, 1.0],
            },
            PublicationCellLineBackground::Black => Self {
                cell_line_color_rgba: [0.82, 0.86, 0.92, 1.0],
            },
            PublicationCellLineBackground::Transparent => Self {
                cell_line_color_rgba: [0.20, 0.28, 0.40, 1.0],
            },
        }
    }
}

impl DepthEnhancement {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PublicationLookProfile {
    pub id: PublicationLookProfileId,
    pub version: &'static str,
    pub key_direction: [f32; 3],
    pub key_intensity: f32,
    pub fill_direction: [f32; 3],
    pub fill_intensity: f32,
    pub rim_direction: [f32; 3],
    pub rim_intensity: f32,
    pub ambient: f32,
    pub roughness: f32,
    pub specular: f32,
    pub opacity: f32,
    pub exposure: f32,
    pub tone_mapping: ToneMapping,
    pub bond_color_mode: PublicationBondColorMode,
    pub cell_line_width_pixels: f32,
    pub depth_enhancement: DepthEnhancement,
}

impl PublicationLookProfile {
    pub fn for_id(id: PublicationLookProfileId) -> Result<Self, String> {
        let profile = match id {
            PublicationLookProfileId::ScientificGloss => Self {
                id,
                version: "v1",
                key_direction: [0.30, 0.60, 0.741_619_9],
                key_intensity: 0.82,
                fill_direction: [-0.55, 0.24, 0.799_937_5],
                fill_intensity: 0.20,
                rim_direction: [0.10, -0.74, 0.665_131_6],
                rim_intensity: 0.18,
                ambient: 0.16,
                roughness: 0.30,
                specular: 0.34,
                opacity: 1.0,
                exposure: 0.0,
                tone_mapping: ToneMapping::Aces,
                bond_color_mode: PublicationBondColorMode::ByElements,
                cell_line_width_pixels: 1.0,
                depth_enhancement: DepthEnhancement::Disabled,
            },
            PublicationLookProfileId::Studio => Self {
                id,
                version: "v1",
                key_direction: [0.42, 0.68, 0.601_000_85],
                key_intensity: 0.88,
                fill_direction: [-0.64, 0.12, 0.758_946_66],
                fill_intensity: 0.30,
                rim_direction: [0.16, -0.82, 0.548_816_9],
                rim_intensity: 0.30,
                ambient: 0.13,
                roughness: 0.38,
                specular: 0.28,
                opacity: 1.0,
                exposure: 0.10,
                tone_mapping: ToneMapping::Aces,
                bond_color_mode: PublicationBondColorMode::Uniform,
                cell_line_width_pixels: 1.0,
                depth_enhancement: DepthEnhancement::Disabled,
            },
            PublicationLookProfileId::Unlit => Self {
                id,
                version: "v1",
                key_direction: [0.0, 0.0, 1.0],
                key_intensity: 0.0,
                fill_direction: [0.0, 0.0, 1.0],
                fill_intensity: 0.0,
                rim_direction: [0.0, 0.0, 1.0],
                rim_intensity: 0.0,
                ambient: 0.0,
                roughness: 1.0,
                specular: 0.0,
                opacity: 1.0,
                exposure: 0.0,
                tone_mapping: ToneMapping::Disabled,
                bond_color_mode: PublicationBondColorMode::ByElements,
                cell_line_width_pixels: 1.0,
                depth_enhancement: DepthEnhancement::Disabled,
            },
        };
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version.is_empty()
            || !self.key_intensity.is_finite()
            || !self.fill_intensity.is_finite()
            || !self.rim_intensity.is_finite()
            || !self.ambient.is_finite()
            || !self.roughness.is_finite()
            || !self.specular.is_finite()
            || !self.opacity.is_finite()
            || !self.exposure.is_finite()
            || !self.cell_line_width_pixels.is_finite()
        {
            return Err("publication look contains a non-finite field".to_owned());
        }
        for direction in [self.key_direction, self.fill_direction, self.rim_direction] {
            let squared_length = direction[0] * direction[0]
                + direction[1] * direction[1]
                + direction[2] * direction[2];
            if !squared_length.is_finite() || !(0.999..=1.001).contains(&squared_length) {
                return Err("publication light direction must be normalized".to_owned());
            }
        }
        if !(0.0..=4.0).contains(&self.key_intensity)
            || !(0.0..=4.0).contains(&self.fill_intensity)
            || !(0.0..=4.0).contains(&self.rim_intensity)
            || !(0.0..=1.0).contains(&self.ambient)
            || !(0.04..=1.0).contains(&self.roughness)
            || !(0.0..=1.0).contains(&self.specular)
            || !(0.0..=1.0).contains(&self.opacity)
            || !(-4.0..=4.0).contains(&self.exposure)
            || !(1.0..=1.0).contains(&self.cell_line_width_pixels)
        {
            return Err("publication look field is outside its fixed bounds".to_owned());
        }
        if self.id == PublicationLookProfileId::Unlit
            && (self.key_intensity != 0.0
                || self.fill_intensity != 0.0
                || self.rim_intensity != 0.0
                || self.ambient != 0.0
                || self.specular != 0.0
                || self.exposure != 0.0
                || self.tone_mapping != ToneMapping::Disabled
                || self.depth_enhancement != DepthEnhancement::Disabled)
        {
            return Err("Unlit must bypass all publication visual modulation".to_owned());
        }
        Ok(())
    }

    pub(crate) fn validate_fixed(&self) -> Result<(), String> {
        self.validate()?;
        let fixed = Self::for_id(self.id)?;
        if self != &fixed {
            return Err("publication look must match its fixed profile snapshot".to_owned());
        }
        Ok(())
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PublicationLookUniform {
    pub key_direction: [f32; 4],
    pub fill_direction: [f32; 4],
    pub rim_direction: [f32; 4],
    pub material: [f32; 4],
    pub exposure_tone_unlit_projection: [f32; 4],
}

impl PublicationLookUniform {
    pub fn from_profile(profile: PublicationLookProfile, is_perspective: bool) -> Self {
        Self {
            key_direction: [
                profile.key_direction[0],
                profile.key_direction[1],
                profile.key_direction[2],
                profile.key_intensity,
            ],
            fill_direction: [
                profile.fill_direction[0],
                profile.fill_direction[1],
                profile.fill_direction[2],
                profile.fill_intensity,
            ],
            rim_direction: [
                profile.rim_direction[0],
                profile.rim_direction[1],
                profile.rim_direction[2],
                profile.rim_intensity,
            ],
            material: [
                profile.ambient,
                profile.roughness,
                profile.specular,
                profile.opacity,
            ],
            exposure_tone_unlit_projection: [
                profile.exposure,
                if profile.tone_mapping == ToneMapping::Aces {
                    1.0
                } else {
                    0.0
                },
                if profile.id == PublicationLookProfileId::Unlit {
                    1.0
                } else {
                    0.0
                },
                if is_perspective { 1.0 } else { 0.0 },
            ],
        }
    }
}
