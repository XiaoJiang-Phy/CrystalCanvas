//! Volumetric data structure for 3D scalar fields (CHGCAR, LOCPOT, .cube, .xsf)
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Serialize;

#[derive(Clone, Serialize)]
pub enum VolumetricFormat {
    VaspChgcar,
    VaspLocpot,
    GaussianCube,
    Xsf,
}

/// 3D scalar field on a regular grid, aligned to the crystallographic unit cell.
/// Grid indices follow Fortran column-major order: x fastest, z slowest.
/// CAUTION: deep-copies data Vec (up to 13.5 MB at 150³). Avoid cloning on hot paths.
#[derive(Clone, Serialize)]
pub struct VolumetricData {
    /// Grid dimensions $(N_x, N_y, N_z)$
    pub grid_dims: [usize; 3],

    /// 3x3 lattice matrix (ColMajor, Å) defining the voxel-to-Cartesian mapping:
    /// $\mathbf{r}(i,j,k) = \frac{i}{N_x}\mathbf{a} + \frac{j}{N_y}\mathbf{b} + \frac{k}{N_z}\mathbf{c}$
    /// ColMajor: [a_x, a_y, a_z, b_x, b_y, b_z, c_x, c_y, c_z]
    pub lattice: [f64; 9],

    /// Flattened scalar field values in physical units ($e/\text{Å}^3$ for CHGCAR, $e/a_0^3$ for .cube).
    /// Index: `data[ix + iy * Nx + iz * Nx * Ny]` (x-fastest, Fortran order).
    pub data: Vec<f32>,

    /// Global min/max for UI slider range
    pub data_min: f32,
    pub data_max: f32,

    /// Source file type (for provenance)
    pub source_format: VolumetricFormat,

    /// Origin offset (relevant for .cube files and some .xsf; zero for CHGCAR)
    pub origin: [f64; 3],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volumetric_data_creation() {
        let data = vec![1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let vol = VolumetricData {
            grid_dims: [2, 2, 2],
            lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            data_min: 1.0,
            data_max: 8.0,
            data,
            source_format: VolumetricFormat::VaspChgcar,
            origin: [0.0, 0.0, 0.0],
        };
        assert_eq!(vol.data_min, 1.0);
        assert_eq!(vol.data_max, 8.0);
        assert_eq!(vol.grid_dims, [2, 2, 2]);
        assert_eq!(vol.data.len(), 8);
    }

    #[test]
    fn test_empty_data_field_is_valid() {
        let vol = VolumetricData {
            grid_dims: [0, 0, 0],
            lattice: [0.0; 9],
            data: Vec::new(),
            data_min: 0.0,
            data_max: 0.0,
            source_format: VolumetricFormat::GaussianCube,
            origin: [0.0, 0.0, 0.0],
        };
        assert!(vol.data.is_empty());
        let n_voxels = vol.grid_dims[0] * vol.grid_dims[1] * vol.grid_dims[2];
        assert_eq!(n_voxels, vol.data.len());
    }

    #[test]
    fn test_single_voxel_grid() {
        let vol = VolumetricData {
            grid_dims: [1, 1, 1],
            lattice: [3.867, 0.0, 0.0, 0.0, 3.867, 0.0, 0.0, 0.0, 6.359],
            data: vec![-0.5_f32],
            data_min: -0.5,
            data_max: -0.5,
            source_format: VolumetricFormat::VaspLocpot,
            origin: [0.0, 0.0, 0.0],
        };
        assert_eq!(vol.data.len(), 1);
        assert_eq!(vol.data_min, vol.data_max);
        assert!(vol.data_min < 0.0, "LOCPOT can have negative potential");
    }

    #[test]
    fn test_constant_scalar_field_min_eq_max() {
        let n = 27_usize;
        let vol = VolumetricData {
            grid_dims: [3, 3, 3],
            lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            data: vec![0.1_f32; n],
            data_min: 0.1,
            data_max: 0.1,
            source_format: VolumetricFormat::VaspChgcar,
            origin: [0.0, 0.0, 0.0],
        };
        assert_eq!(vol.data_min, vol.data_max);
        // A normalised slider value (v - min) / (max - min) with min==max must be guarded upstream.
        // Here we only assert the struct holds the invariant; slider guard is UI's problem.
        assert!((vol.data_max - vol.data_min).abs() < f32::EPSILON);
    }

    #[test]
    fn test_f32_extremes_do_not_corrupt_struct() {
        let data = vec![f32::MIN_POSITIVE, 1.0e10_f32, f32::MAX];
        let vol = VolumetricData {
            grid_dims: [3, 1, 1],
            lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            data_min: f32::MIN_POSITIVE,
            data_max: f32::MAX,
            data,
            source_format: VolumetricFormat::Xsf,
            origin: [0.0, 0.0, 0.0],
        };
        assert!(vol.data_min > 0.0);
        assert!(vol.data_max.is_finite());
        assert!(!vol.data_max.is_nan());
        let range = vol.data_max - vol.data_min;
        assert!(range.is_finite() || range.is_infinite(), "range overflow is expected behavior; caller must handle");
    }

    #[test]
    fn test_data_grid_mismatch_is_detectable() {
        let vol = VolumetricData {
            grid_dims: [4, 4, 4],
            lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            data: vec![0.0_f32; 8],
            data_min: 0.0,
            data_max: 0.0,
            source_format: VolumetricFormat::VaspChgcar,
            origin: [0.0, 0.0, 0.0],
        };
        let claimed = vol.grid_dims[0] * vol.grid_dims[1] * vol.grid_dims[2];
        // Mismatch must be detectable so parsers can return Err before handing to GPU
        assert_ne!(claimed, vol.data.len(), "mismatch must be detectable by caller");
    }

    #[test]
    fn test_degenerate_zero_lattice_is_detectable() {
        let vol = VolumetricData {
            grid_dims: [2, 2, 2],
            lattice: [0.0; 9],
            data: vec![1.0_f32; 8],
            data_min: 1.0,
            data_max: 1.0,
            source_format: VolumetricFormat::Xsf,
            origin: [0.0, 0.0, 0.0],
        };
        // ColMajor 3×3 determinant: det = a_x*(b_y*c_z - b_z*c_y) - a_y*(...) + a_z*(...)
        let l = &vol.lattice;
        let det = l[0] * (l[4] * l[8] - l[5] * l[7])
                - l[1] * (l[3] * l[8] - l[5] * l[6])
                + l[2] * (l[3] * l[7] - l[4] * l[6]);
        // Any lattice-based coordinate transform with det==0 must be rejected upstream
        assert_eq!(det, 0.0, "degenerate lattice must have zero determinant");
    }

    #[test]
    fn test_all_negative_values_min_max_ordering() {
        let data: Vec<f32> = vec![-9.0, -4.0, -7.0, -1.0, -3.0, -6.0, -8.0, -2.0];
        let vol = VolumetricData {
            grid_dims: [2, 2, 2],
            lattice: [5.0, 0.0, 0.0, 0.0, 5.0, 0.0, 0.0, 0.0, 5.0],
            data_min: -9.0,
            data_max: -1.0,
            data,
            source_format: VolumetricFormat::VaspLocpot,
            origin: [0.0, 0.0, 0.0],
        };
        assert!(vol.data_min < 0.0);
        assert!(vol.data_max < 0.0);
        assert!(vol.data_min < vol.data_max, "min must be strictly less than max");
    }

    #[test]
    fn test_inverted_min_max_is_invariant_violation() {
        let vol = VolumetricData {
            grid_dims: [1, 1, 1],
            lattice: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            data: vec![5.0_f32],
            data_min: 99.0,
            data_max: -1.0,
            source_format: VolumetricFormat::GaussianCube,
            origin: [0.0, 0.0, 0.0],
        };
        // The struct accepts it (no runtime check) — parser must enforce ordering.
        let invariant_holds = vol.data_min <= vol.data_max;
        assert!(!invariant_holds, "parser must enforce data_min <= data_max; this struct carries an invalid state");
    }
}
