// [Overview: C++ implementation of intense physics computations and matrix operations.]
// Spglib C Wrapper Implementation
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
#include "physics_kernel.hpp"
#include <iostream>
#include <exception>
#include <vector>

// Include spglib
extern "C" {
#include "spglib.h"
}

int get_spacegroup(const double *lattice, const double *positions,
                   const int *types, size_t n_atoms, double symprec) {
  try {
    // IMPORTANT: The input `lattice` is in COLUMN-MAJOR order from Rust/Eigen:
    //   [a_x, a_y, a_z, b_x, b_y, b_z, c_x, c_y, c_z]
    //   i.e. columns = lattice vectors (a, b, c)
    //
    // Spglib expects lattice[3][3] in ROW-MAJOR where ROWS = lattice vectors:
    //   lattice[0] = a-vector, lattice[1] = b-vector, lattice[2] = c-vector
    //
    // Therefore we must TRANSPOSE the input.
    double spg_lattice[3][3] = {
        {lattice[0], lattice[3], lattice[6]},  // a-vector (col 0 -> row 0)
        {lattice[1], lattice[4], lattice[7]},  // b-vector (col 1 -> row 1)
        {lattice[2], lattice[5], lattice[8]}   // c-vector (col 2 -> row 2)
    };

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
    std::string hm_symbol(dataset->international_symbol);
    spg_free_dataset(dataset);

    fprintf(stderr, "[Spglib] Detected spacegroup #%d (%s)\n",
            spacegroup_number, hm_symbol.c_str());

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

// Helper to compute gcd
int gcd(int a, int b) {
    if (b == 0) return std::abs(a);
    return gcd(b, a % b);
}

// Find vectors u and v such that h*u + k*v = gcd(h,k) (Extended Euclidean Algorithm)
void ext_gcd(int h, int k, int& u, int& v, int& g) {
    if (k == 0) {
        u = 1; v = 0; g = std::abs(h);
        if (h < 0) u = -1;
        return;
    }
    int u1, v1;
    ext_gcd(k, h % k, u1, v1, g);
    u = v1;
    v = u1 - (h / k) * v1;
}

// Compute the transformation matrix P from traditional Miller indices
// that returns a primitive surface cell.
Eigen::Matrix3i get_surface_transformation(int h, int k, int l) {
    Eigen::Matrix3i P;
    P.setZero();
    
    // Normalize Miller indices
    int g1 = gcd(gcd(h, k), l);
    if (g1 > 0) {
        h /= g1; k /= g1; l /= g1;
    }
    
    if (h == 0 && k == 0) {
        // Special case: (0 0 l)
        P << 1, 0, 0,
             0, 1, 0,
             0, 0, 1;
        return P;
    }
    
    int u, v, g_hk;
    ext_gcd(h, k, u, v, g_hk);
    
    // v1: [-k/g, h/g, 0]
    P(0, 0) = -k / g_hk;
    P(1, 0) = h / g_hk;
    P(2, 0) = 0;
    
    // v2: [-l*u, -l*v, g_hk]
    P(0, 1) = -l * u;
    P(1, 1) = -l * v;
    P(2, 1) = g_hk;
    
    // v3: we pick a vector out of plane, simple choice [u, v, 0] doesn't always work if l!=0
    // Try to find a simple v3 such that det(P) = 1
    // The determinant of the above first two columns and [x, y, z] is:
    // det = (-k/g)*( (-l*v)*z - g_hk*y ) - (h/g)*( (-l*u)*z - g_hk*x )
    // We want det = 1 or -1
    // Easier: complete basis using Smith Normal Form or general integer matrix completion
    // Given (h,k,l) is primitive, we know there exist p,q,r st h*p+k*q+l*r = 1
    // Then v3 = [p, q, r] gives a basis where the surface is the ab plane and c points out.
    
    // We can just use the extended gcd three variables:
    // a*h + b*k = g_hk -> a = u, b = v
    // We also know g_hk and l are coprime since gcd(h,k,l) = 1
    // So there exist p', q' st p'*g_hk + q'*l = 1
    int p_prime, q_prime, g2;
    ext_gcd(g_hk, l, p_prime, q_prime, g2); // g2=1
    
    int p = p_prime * u;
    int q = p_prime * v;
    int r = q_prime;
    
    P(0, 2) = p;
    P(1, 2) = q;
    P(2, 2) = r;
    
    if (P.determinant() < 0) {
        // Ensure right-handedness
        P.col(0) = -P.col(0);
    }
    
    return P;
}

int get_slab_size(
    const double* lattice,
    const int32_t* miller,
    int layers,
    double vacuum_A,
    size_t n_atoms
) {
    if (layers <= 0) return 0;
    
    // 1. Calculate the surface transformation matrix P
    Eigen::Matrix3i P = get_surface_transformation(miller[0], miller[1], miller[2]);
    
    // 2. The expansion matrix has c-axis multiplied by 'layers'
    Eigen::Matrix3i exp_mat = P;
    exp_mat(0, 2) *= layers;
    exp_mat(1, 2) *= layers;
    exp_mat(2, 2) *= layers;
    
    return get_supercell_size(n_atoms, exp_mat.data());
}

void build_slab(
    const double* lattice,
    const double* positions,
    const int* types,
    size_t n_atoms,
    const int32_t* miller,
    int layers,
    double vacuum_A,
    double* out_lattice,
    double* out_positions,
    int* out_types
) {
    // 1. Expansion matrix for the slab
    Eigen::Matrix3i P = get_surface_transformation(miller[0], miller[1], miller[2]);
    Eigen::Matrix3i exp_mat = P;
    exp_mat(0, 2) *= layers;
    exp_mat(1, 2) *= layers;
    exp_mat(2, 2) *= layers;
    
    // Call build_supercell to generate the block of atoms
    build_supercell(lattice, positions, types, n_atoms, exp_mat.data(), out_lattice, out_positions, out_types);
    
    // Now, adjust the out_lattice c-axis to include vacuum padding
    Eigen::Map<Eigen::Matrix3d> L_out(out_lattice);
    Eigen::Vector3d c_vec = L_out.col(2);
    double c_len = c_vec.norm();
    
    // New total c_len = c_len + vacuum_A
    double scale = (c_len + vacuum_A) / c_len;
    L_out.col(2) *= scale;
    
    // Adjust fractional coordinates (since the box got larger, z-coords get scaled down)
    // Only the z-coordinate component (or we can just do f_new = L_new^-1 * L_old * f_old)
    // Because c-axis is scaled by 'scale', the fractional z should be divided by 'scale'.
    int total_new_atoms = n_atoms * std::abs(exp_mat.determinant());
    for (int i = 0; i < total_new_atoms; ++i) {
        out_positions[3*i+2] /= scale;
        // Optional: shift to center the slab in the vacuum
        out_positions[3*i+2] += (vacuum_A / 2.0) / (c_len + vacuum_A);
    }
}

bool check_overlap_mic(const double* lattice, const double* positions,
                       size_t num_atoms, const double* new_frac_pos,
                       double threshold_A) {
    // Map the lattice as Column-Major to follow CrystalCanvas convention
    Eigen::Map<const Eigen::Matrix<double, 3, 3, Eigen::ColMajor>> lattice_matrix(lattice);
    Eigen::Vector3d fractional_new(new_frac_pos[0], new_frac_pos[1], new_frac_pos[2]);
    double threshold_squared = threshold_A * threshold_A;

    for (size_t i = 0; i < num_atoms; ++i) {
        Eigen::Vector3d fractional_old(positions[3 * i], positions[3 * i + 1], positions[3 * i + 2]);
        Eigen::Vector3d fractional_difference = fractional_new - fractional_old;

        // Apply Minimum Image Convention (MIC) for overlap check
        fractional_difference.x() -= std::round(fractional_difference.x());
        fractional_difference.y() -= std::round(fractional_difference.y());
        fractional_difference.z() -= std::round(fractional_difference.z());

        if ((lattice_matrix * fractional_difference).squaredNorm() < threshold_squared) {
            return true;
        }
    }
    return false;
}

int compute_bonds(const double* lattice, const double* cart_positions,
                  const double* frac_positions, const double* covalent_radii,
                  size_t num_atoms, double threshold_factor,
                  double min_bond_length, int32_t* out_atom_i,
                  int32_t* out_atom_j, double* out_distances,
                  size_t max_bonds) {
    try {
        double minimum_bond_length_squared = min_bond_length * min_bond_length;
        int bond_count = 0;

        // Map the lattice as Column-Major to follow CrystalCanvas convention
        Eigen::Map<const Eigen::Matrix<double, 3, 3, Eigen::ColMajor>> lattice_matrix(lattice);

        for (size_t i = 0; i < num_atoms; ++i) {
            Eigen::Vector3d fractional_i(frac_positions[3 * i],
                                         frac_positions[3 * i + 1],
                                         frac_positions[3 * i + 2]);
            double radius_i = covalent_radii[i];

            for (size_t j = i; j < num_atoms; ++j) {
                double radius_j = covalent_radii[j];
                double max_bond_distance = (radius_i + radius_j) * threshold_factor;
                double max_bond_distance_squared = max_bond_distance * max_bond_distance;

                Eigen::Vector3d fractional_j(frac_positions[3 * j],
                                             frac_positions[3 * j + 1],
                                             frac_positions[3 * j + 2]);

                // Perform 27-image search to correctly find bonds in small unit cells (e.g., Rutile TiO2)
                for (int nx = -1; nx <= 1; ++nx) {
                    for (int ny = -1; ny <= 1; ++ny) {
                        for (int nz = -1; nz <= 1; ++nz) {
                            if (i == j) {
                                // For self-pairing, avoid double counting (i, i, shift) and (i, i, -shift)
                                if (nz < 0) continue;
                                if (nz == 0 && ny < 0) continue;
                                if (nz == 0 && ny == 0 && nx <= 0) continue;
                            }

                            Eigen::Vector3d shift(static_cast<double>(nx),
                                                  static_cast<double>(ny),
                                                  static_cast<double>(nz));
                            Eigen::Vector3d diff = (fractional_j + shift) - fractional_i;
                            double distance_squared = (lattice_matrix * diff).squaredNorm();

                            if (distance_squared > minimum_bond_length_squared &&
                                distance_squared < max_bond_distance_squared) {
                                if (static_cast<size_t>(bond_count) >= max_bonds) {
                                    return bond_count;
                                }
                                out_atom_i[bond_count] = static_cast<int32_t>(i);
                                out_atom_j[bond_count] = static_cast<int32_t>(j);
                                out_distances[bond_count] = std::sqrt(distance_squared);
                                bond_count++;
                            }
                        }
                    }
                }
            }
        }
        return bond_count;
    } catch (const std::exception& e) {
        std::cerr << "Exception in compute_bonds: " << e.what() << std::endl;
        return 0;
    } catch (...) {
        std::cerr << "Unknown exception in compute_bonds." << std::endl;
        return 0;
    }
}

int find_coordination_shell(const double* lattice, const double* cart_positions,
                            const double* frac_positions,
                            const double* covalent_radii, size_t num_atoms,
                            size_t center_idx, double threshold_factor,
                            double min_bond_length,
                            int32_t* out_neighbor_indices,
                            double* out_distances, size_t max_neighbors) {
    try {
        if (center_idx >= num_atoms) {
            return 0;
        }

        // Map the lattice as Column-Major to follow CrystalCanvas convention
        Eigen::Map<const Eigen::Matrix<double, 3, 3, Eigen::ColMajor>> lattice_matrix(lattice);

        Eigen::Vector3d fractional_center(frac_positions[3 * center_idx],
                                          frac_positions[3 * center_idx + 1],
                                          frac_positions[3 * center_idx + 2]);
        double radius_center = covalent_radii[center_idx];
        double minimum_bond_length_squared = min_bond_length * min_bond_length;
        int neighbor_count = 0;

        for (size_t j = 0; j < num_atoms; ++j) {
            double radius_j = covalent_radii[j];
            double max_bond_distance = (radius_center + radius_j) * threshold_factor;
            double max_bond_distance_squared = max_bond_distance * max_bond_distance;

            Eigen::Vector3d fractional_j(frac_positions[3 * j],
                                         frac_positions[3 * j + 1],
                                         frac_positions[3 * j + 2]);

            // Perform 27-image search to accurately identify neighbors in periodic systems
            for (int nx = -1; nx <= 1; ++nx) {
                for (int ny = -1; ny <= 1; ++ny) {
                    for (int nz = -1; nz <= 1; ++nz) {
                        // Skip the center atom itself in the primary cell
                        if (j == center_idx && nx == 0 && ny == 0 && nz == 0) continue;

                        Eigen::Vector3d shift(static_cast<double>(nx),
                                              static_cast<double>(ny),
                                              static_cast<double>(nz));
                        Eigen::Vector3d diff = (fractional_j + shift) - fractional_center;
                        double distance_squared = (lattice_matrix * diff).squaredNorm();

                        if (distance_squared > minimum_bond_length_squared &&
                            distance_squared < max_bond_distance_squared) {
                            if (static_cast<size_t>(neighbor_count) >= max_neighbors) {
                                return neighbor_count;
                            }
                            out_neighbor_indices[neighbor_count] = static_cast<int32_t>(j);
                            out_distances[neighbor_count] = std::sqrt(distance_squared);
                            neighbor_count++;
                        }
                    }
                }
            }
        }
        return neighbor_count;
    } catch (const std::exception& e) {
        std::cerr << "Exception in find_coordination_shell: " << e.what() << std::endl;
        return 0;
    } catch (...) {
        std::cerr << "Unknown exception in find_coordination_shell." << std::endl;
        return 0;
    }
}
