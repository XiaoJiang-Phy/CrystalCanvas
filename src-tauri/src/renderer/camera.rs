//! Orbit camera with perspective projection — provides view-projection matrix for GPU upload

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

/// Orbital camera that looks at a target point from a given position.
/// Provides a perspective projection matrix for the GPU.
pub struct Camera {
    /// Camera position in world space
    pub eye: Vec3,
    /// Point the camera looks at
    pub target: Vec3,
    /// Up direction (typically Y-up)
    pub up: Vec3,
    /// Vertical field of view in degrees
    pub fovy_deg: f32,
    /// Aspect ratio (width / height)
    pub aspect: f32,
    /// Near clipping plane distance
    pub znear: f32,
    /// Far clipping plane distance
    pub zfar: f32,
    /// Whether to use perspective (true) or orthographic (false) projection
    pub is_perspective: bool,
    /// Orthographic scale factor
    pub orthographic_scale: f32,
}

impl Camera {
    /// Create a default camera suitable for viewing a small crystal structure.
    /// Positioned at (0, 10, 30) looking at origin, 45° FOV.
    pub fn default_for_crystal() -> Self {
        Self {
            eye: Vec3::new(0.0, 10.0, 30.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fovy_deg: 45.0,
            aspect: 16.0 / 9.0,
            znear: 0.1,
            zfar: 200.0,
            is_perspective: true,
            orthographic_scale: 30.0,
        }
    }

    /// Update aspect ratio (called on window resize).
    pub fn set_aspect(&mut self, width: f32, height: f32) {
        self.aspect = width / height;
    }

    /// Switch to perspective rendering.
    pub fn set_perspective(&mut self) {
        self.is_perspective = true;
    }

    /// Switch to orthographic rendering.
    pub fn set_orthographic(&mut self, scale: f32) {
        self.is_perspective = false;
        self.orthographic_scale = scale;
    }

    /// Build the combined view-projection matrix.
    /// Uses right-handed coordinate system with Y-up.
    /// wgpu uses a [0,1] depth range (unlike OpenGL's [-1,1]),
    /// so we use `perspective_rh` which handles this correctly.
    #[allow(dead_code)]
    pub fn build_view_projection_matrix(&self) -> Mat4 {
        let view = Mat4::look_at_rh(self.eye, self.target, self.up);
        let proj = Mat4::perspective_rh(
            self.fovy_deg.to_radians(),
            self.aspect,
            self.znear,
            self.zfar,
        );
        proj * view
    }

    /// Build the view matrix only (for use in impostor sphere shader).
    pub fn build_view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye, self.target, self.up)
    }

    /// Build the projection matrix only.
    pub fn build_projection_matrix(&self) -> Mat4 {
        if self.is_perspective {
            Mat4::perspective_rh(
                self.fovy_deg.to_radians(),
                self.aspect,
                self.znear,
                self.zfar,
            )
        } else {
            let width = self.orthographic_scale * self.aspect;
            let height = self.orthographic_scale;
            Mat4::orthographic_rh(
                -width / 2.0,
                width / 2.0,
                -height / 2.0,
                height / 2.0,
                self.znear,
                self.zfar,
            )
        }
    }
}

/// GPU-uploadable camera uniform data.
/// Contains the view-projection matrix in column-major format.
///
/// Must be `#[repr(C)]` for correct GPU memory layout and
/// derive `Pod + Zeroable` for safe bytemuck casting.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct CameraUniform {
    /// Combined view-projection matrix (column-major, 4x4)
    pub view_proj: [[f32; 4]; 4],
    /// View matrix (column-major, 4x4) — needed for billboard computation
    pub view: [[f32; 4]; 4],
    /// Projection matrix (column-major, 4x4)
    pub proj: [[f32; 4]; 4],
}

impl CameraUniform {
    /// Create a zeroed uniform (identity-like placeholder).
    pub fn new() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            view: Mat4::IDENTITY.to_cols_array_2d(),
            proj: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }

    /// Update from a Camera's current state.
    pub fn update_from_camera(&mut self, camera: &Camera) {
        let view = camera.build_view_matrix();
        let proj = camera.build_projection_matrix();
        let view_proj = proj * view;
        self.view_proj = view_proj.to_cols_array_2d();
        self.view = view.to_cols_array_2d();
        self.proj = proj.to_cols_array_2d();
    }
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self::new()
    }
}
