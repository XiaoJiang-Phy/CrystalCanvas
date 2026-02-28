// Spglib C Wrapper Implementation
#include "physics_kernel.hpp"
#include <iostream>
#include <stdexcept>
#include <vector>

// Include spglib
extern "C" {
#include "spglib.h"
}

int get_spacegroup(const double *lattice, const double *positions,
                   const int *types, size_t n_atoms, double symprec) {
  try {
    // Spglib modifies the dataset and expects positions to be mutable if it
    // needs to standardize. For spg_get_dataset, we just need to pass the raw
    // arrays. However, spgat_get_dataset requires double lattice[3][3], double
    // position[][3], int types[].

    // Convert flat 1D lattice (assuming row-major 3x3) to 2D array for spglib
    double spg_lattice[3][3] = {{lattice[0], lattice[1], lattice[2]},
                                {lattice[3], lattice[4], lattice[5]},
                                {lattice[6], lattice[7], lattice[8]}};

    // Convert positions flat array to 2D array
    std::vector<double> pos_vec(positions, positions + n_atoms * 3);
    double(*spg_positions)[3] = reinterpret_cast<double(*)[3]>(pos_vec.data());

    // Spglib expects non-const types array, make a copy
    std::vector<int> types_vec(types, types + n_atoms);

    // Get spacegroup dataset
    SpglibDataset *dataset = spg_get_dataset(
        spg_lattice, spg_positions, types_vec.data(), n_atoms, symprec);

    if (dataset == nullptr) {
      std::cerr << "Spglib failed to find spacegroup." << std::endl;
      return 0; // Return 0 to indicate failure
    }

    int spacegroup_number = dataset->spacegroup_number;
    spg_free_dataset(dataset);

    return spacegroup_number;

  } catch (const std::exception &e) {
    // Catch any C++ standard exceptions (though spglib is C, std::vector might
    // throw bad_alloc)
    std::cerr << "Exception in get_spacegroup: " << e.what() << std::endl;
    return 0; // Failure
  } catch (...) {
    std::cerr << "Unknown exception in get_spacegroup." << std::endl;
    return 0;
  }
}

#include <Eigen/Dense>

int get_supercell_size(size_t n_atoms, const int32_t* expansion) {
    Eigen::Map<const Eigen::Matrix3i> exp_mat(expansion);
    int det = std::abs(exp_mat.determinant());
    return n_atoms * det;
}

void build_supercell(
    const double* lattice,
    const double* positions,
    const int* types,
    size_t n_atoms,
    const int32_t* expansion,
    double* out_lattice,
    double* out_positions,
    int* out_types
) {
    // Both lattice and P are mapped as ColMajor by default in Eigen
    Eigen::Map<const Eigen::Matrix3d> L(lattice);
    Eigen::Map<const Eigen::Matrix3i> P(expansion);
    
    Eigen::Map<Eigen::Matrix3d> L_out(out_lattice);
    // New lattice L_out = L * P (assuming basis vectors are columns of L)
    L_out = L * P.cast<double>();
    
    Eigen::Matrix3d P_inv = P.cast<double>().inverse();
    
    // Determine bounds for integer shifts
    Eigen::Matrix<double, 3, 8> corners;
    corners << 0, 1, 0, 1, 0, 1, 0, 1,
               0, 0, 1, 1, 0, 0, 1, 1,
               0, 0, 0, 0, 1, 1, 1, 1;
    Eigen::Matrix<double, 3, 8> mapped_corners = P.cast<double>() * corners;
    Eigen::Vector3d min_b = mapped_corners.rowwise().minCoeff();
    Eigen::Vector3d max_b = mapped_corners.rowwise().maxCoeff();
    
    int min_nx = std::floor(min_b.x()) - 1;
    int max_nx = std::ceil(max_b.x()) + 1;
    int min_ny = std::floor(min_b.y()) - 1;
    int max_ny = std::ceil(max_b.y()) + 1;
    int min_nz = std::floor(min_b.z()) - 1;
    int max_nz = std::ceil(max_b.z()) + 1;

    size_t out_idx = 0;
    
    for (size_t i = 0; i < n_atoms; ++i) {
        Eigen::Vector3d f(positions[3*i], positions[3*i+1], positions[3*i+2]);
        int type = types[i];
        
        for (int nx = min_nx; nx <= max_nx; ++nx) {
            for (int ny = min_ny; ny <= max_ny; ++ny) {
                for (int nz = min_nz; nz <= max_nz; ++nz) {
                    Eigen::Vector3d shift(nx, ny, nz);
                    Eigen::Vector3d v = f + shift;
                    Eigen::Vector3d f_new = P_inv * v;
                    
                    const double eps = 1e-5;
                    // Check if f_new is within [0, 1) bounds for the new cell
                    if (f_new.x() >= -eps && f_new.x() < 1.0 - eps &&
                        f_new.y() >= -eps && f_new.y() < 1.0 - eps &&
                        f_new.z() >= -eps && f_new.z() < 1.0 - eps) {
                        
                        // Wrap precisely into [0, 1) to avoid -0.00000 
                        f_new.x() = f_new.x() - std::floor(f_new.x());
                        f_new.y() = f_new.y() - std::floor(f_new.y());
                        f_new.z() = f_new.z() - std::floor(f_new.z());
                        
                        out_positions[out_idx * 3 + 0] = f_new.x();
                        out_positions[out_idx * 3 + 1] = f_new.y();
                        out_positions[out_idx * 3 + 2] = f_new.z();
                        out_types[out_idx] = type;
                        out_idx++;
                    }
                }
            }
        }
    }
}
