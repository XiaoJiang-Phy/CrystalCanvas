// [Overview: C++ implementation of intense physics computations and matrix operations.]
// Spglib C Wrapper Implementation
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
#include "physics_kernel.hpp"
#include "physics_kernel_internal.hpp"
#include <iostream>
#include <exception>
#include <vector>
#include <cmath>
#include <algorithm>
#include <numeric>

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

namespace {

int gcd(int a, int b) {
    if (b == 0) return std::abs(a);
    return gcd(b, a % b);
}

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

} // namespace

/// Compute surface-oriented transformation matrix P via Diophantine equations.
/// Properties:  det(P) = 1  (unimodular — no atom duplication or collapse)
///   P.col(0)·G = P.col(1)·G = 0   (in-plane, G = [h,k,l])
///   P.col(2)·G = 1                 (single interplanar step)
/// Ref: Extended Euclidean construction for crystal surface basis.
[[nodiscard]] Eigen::Matrix3i get_surface_basis(
    const Eigen::Ref<const Eigen::Matrix3d>& lattice, int h, int k, int l)
{
    int g_all = gcd(gcd(std::abs(h), std::abs(k)), std::abs(l));
    if (g_all > 0) { h /= g_all; k /= g_all; l /= g_all; }

    Eigen::Vector3i v1, v2, v3;

    int y_kl = 0, z_kl = 0, g_kl = 0;
    ext_gcd(k, l, y_kl, z_kl, g_kl);  // k·y + l·z = g_kl

    if (g_kl == 0) {
        // k = l = 0  →  surface normal ∥ â₁,  h = ±1 after coprime reduction
        v1 = {0, 1, 0};
        v2 = {0, 0, 1};
        v3 = {(h > 0 ? 1 : -1), 0, 0};
    } else {
        int x_hg = 0, p_hg = 0, dummy = 0;
        ext_gcd(h, g_kl, x_hg, p_hg, dummy);  // h·x + g·p = 1

        v1 = {0, l / g_kl, -k / g_kl};
        v2 = {-g_kl, h * y_kl, h * z_kl};
        v3 = {x_hg, p_hg * y_kl, p_hg * z_kl};
    }

    // 2D Gauss reduction: shorten in-plane vectors v1, v2 using lattice metric
    for (int iter = 0; iter < 20; ++iter) {
        Eigen::Vector3d c1 = lattice * v1.cast<double>();
        Eigen::Vector3d c2 = lattice * v2.cast<double>();
        if (c2.squaredNorm() < c1.squaredNorm()) { std::swap(v1, v2); std::swap(c1, c2); }
        int n = static_cast<int>(std::round(c1.dot(c2) / c1.squaredNorm()));
        if (n == 0) break;
        v2 -= n * v1;
    }

    // Reduce v3 inclination: project out in-plane components
    {
        Eigen::Vector3d c1 = lattice * v1.cast<double>();
        Eigen::Vector3d c2 = lattice * v2.cast<double>();
        Eigen::Vector3d c3 = lattice * v3.cast<double>();
        int p1 = (c1.squaredNorm() > 1e-12)
            ? static_cast<int>(std::round(c3.dot(c1) / c1.squaredNorm())) : 0;
        int p2 = (c2.squaredNorm() > 1e-12)
            ? static_cast<int>(std::round(c3.dot(c2) / c2.squaredNorm())) : 0;
        v3 -= p1 * v1 + p2 * v2;
    }

    Eigen::Matrix3i P;
    P.col(0) = v1;
    P.col(1) = v2;
    P.col(2) = v3;

    if (P.determinant() < 0) {
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
    
    Eigen::Map<const Eigen::Matrix3d> L(lattice);
    Eigen::Matrix3i P = get_surface_basis(L, miller[0], miller[1], miller[2]);
    
    Eigen::Matrix3i exp_mat = P;
    exp_mat.col(2) *= layers;
    
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
    Eigen::Map<const Eigen::Matrix3d> L(lattice);
    Eigen::Matrix3i P = get_surface_basis(L, miller[0], miller[1], miller[2]);
    Eigen::Matrix3i exp_mat = P;
    exp_mat.col(2) *= layers;
    
    build_supercell(lattice, positions, types, n_atoms, exp_mat.data(), out_lattice, out_positions, out_types);
    
    Eigen::Map<Eigen::Matrix3d> L_out(out_lattice);
    Eigen::Vector3d c_vec = L_out.col(2);
    double c_len = c_vec.norm();
    
    double scale = (c_len + vacuum_A) / c_len;
    L_out.col(2) *= scale;
    
    int total_new_atoms = n_atoms * std::abs(exp_mat.determinant());
    for (int i = 0; i < total_new_atoms; ++i) {
        out_positions[3*i+2] /= scale;
        out_positions[3*i+2] += (vacuum_A / 2.0) / (c_len + vacuum_A);
    }
}

[[nodiscard]] int get_slab_size_v2(
    const double* lattice, const int32_t* miller,
    int n_layers, size_t n_atoms)
{
    if (n_layers <= 0) return 0;
    Eigen::Map<const Eigen::Matrix3d> L(lattice);
    Eigen::Matrix3i P = get_surface_basis(L, miller[0], miller[1], miller[2]);
    Eigen::Matrix3i exp_mat = P;
    exp_mat.col(2) *= n_layers;
    return get_supercell_size(n_atoms, exp_mat.data());
}

[[nodiscard]] int build_slab_v2(
    const double* lattice, const double* positions,
    const int* types, size_t n_atoms,
    const int32_t* miller, int n_layers, double vacuum_a,
    double* out_lattice, double* out_positions, int* out_types)
{
    vacuum_a = std::max(0.0, vacuum_a);
    Eigen::Map<const Eigen::Matrix3d> L(lattice);
    Eigen::Matrix3i P = get_surface_basis(L, miller[0], miller[1], miller[2]);
    Eigen::Matrix3i exp_mat = P;
    exp_mat.col(2) *= n_layers;
    
    build_supercell(lattice, positions, types, n_atoms, exp_mat.data(), out_lattice, out_positions, out_types);
    
    Eigen::Map<Eigen::Matrix3d> L_out(out_lattice);
    int total_new_atoms = n_atoms * std::abs(exp_mat.determinant());
    
    std::vector<int> indices(total_new_atoms);
    std::iota(indices.begin(), indices.end(), 0);
    
    std::vector<Eigen::Vector3d> cart_pos(total_new_atoms);
    for (int i = 0; i < total_new_atoms; ++i) {
        Eigen::Vector3d f(out_positions[3*i], out_positions[3*i+1], out_positions[3*i+2]);
        cart_pos[i] = L_out * f;
    }

    std::sort(indices.begin(), indices.end(), [&](int a, int b) {
        long long ax = std::round(cart_pos[a].x() * 1e4);
        long long bx = std::round(cart_pos[b].x() * 1e4);
        if (ax != bx) return ax < bx;
        long long ay = std::round(cart_pos[a].y() * 1e4);
        long long by = std::round(cart_pos[b].y() * 1e4);
        if (ay != by) return ay < by;
        long long az = std::round(cart_pos[a].z() * 1e4);
        long long bz = std::round(cart_pos[b].z() * 1e4);
        return az < bz;
    });

    std::vector<int> unique_indices;
    unique_indices.reserve(total_new_atoms);
    
    for (int idx : indices) {
        bool duplicate = false;
        for (auto it = unique_indices.rbegin(); it != unique_indices.rend(); ++it) {
            int u_idx = *it;
            if (cart_pos[idx].x() - cart_pos[u_idx].x() > 1e-3) break;
            if ((cart_pos[idx] - cart_pos[u_idx]).norm() < 1e-4) {
                duplicate = true;
                break;
            }
        }
        if (!duplicate) {
            unique_indices.push_back(idx);
        }
    }
    
    int n_unique = unique_indices.size();
    
    std::vector<double> final_pos(n_unique * 3);
    std::vector<int> final_types(n_unique);
    for (int i = 0; i < n_unique; ++i) {
        int original_idx = unique_indices[i];
        final_pos[3*i]   = out_positions[3*original_idx];
        final_pos[3*i+1] = out_positions[3*original_idx+1];
        final_pos[3*i+2] = out_positions[3*original_idx+2];
        final_types[i]   = out_types[original_idx];
    }
    std::copy(final_pos.begin(), final_pos.end(), out_positions);
    std::copy(final_types.begin(), final_types.end(), out_types);
    
    // --- Orthogonalize c-axis: force c ⊥ surface (α = β = 90°) ---
    // Decompose: c_tilted = α·a + β·b + h·n̂
    // Absorb (α, β) into fractional (x, y); replace c with h·n̂ + vacuum.
    Eigen::Vector3d a_vec = L_out.col(0);
    Eigen::Vector3d b_vec = L_out.col(1);
    Eigen::Vector3d c_vec = L_out.col(2);

    Eigen::Vector3d normal = a_vec.cross(b_vec);
    double normal_len = normal.norm();
    Eigen::Vector3d n_hat = normal / normal_len;

    double height = c_vec.dot(n_hat);       // slab thickness along normal
    if (height < 0.0) { n_hat = -n_hat; height = -height; }

    // Solve Gram system for in-plane decomposition:
    //   [|a|²   a·b ] [α]   [a·c]
    //   [a·b   |b|² ] [β] = [b·c]
    double aa = a_vec.squaredNorm();
    double ab = a_vec.dot(b_vec);
    double bb = b_vec.squaredNorm();
    double ac = a_vec.dot(c_vec);
    double bc = b_vec.dot(c_vec);
    double det_G = aa * bb - ab * ab;
    double alpha = (bb * ac - ab * bc) / det_G;
    double beta  = (aa * bc - ab * ac) / det_G;

    double c_new_len = height + vacuum_a;

    for (int i = 0; i < n_unique; ++i) {
        double fx = out_positions[3*i+0];
        double fy = out_positions[3*i+1];
        double fz = out_positions[3*i+2];

        // Absorb c-tilt into (x, y), wrap to [0, 1)
        double nx = fx + alpha * fz;
        double ny = fy + beta  * fz;
        nx -= std::floor(nx);
        ny -= std::floor(ny);

        // Scale z for vacuum and center slab in the middle
        double nz = fz * height / c_new_len + vacuum_a / (2.0 * c_new_len);

        out_positions[3*i+0] = nx;
        out_positions[3*i+1] = ny;
        out_positions[3*i+2] = nz;
    }

    // Set c perpendicular to surface
    L_out.col(2) = n_hat * c_new_len;

    // QR standardize to PDB convention (a‖X, b in XY, c along Z)
    Eigen::HouseholderQR<Eigen::Matrix3d> qr(L_out);
    Eigen::Matrix3d R = qr.matrixQR().triangularView<Eigen::Upper>();
    for (int i = 0; i < 3; ++i) {
        if (R(i, i) < 0.0) R.row(i) *= -1.0;
    }
    L_out = R;
    
    return n_unique;
}

[[nodiscard]] int cluster_slab_layers(
    const double* positions, size_t n_atoms,
    const double* lattice,
    double layer_tolerance_a,
    double* out_layer_centers, size_t max_layers)
{
    if (n_atoms == 0 || max_layers == 0) return 0;
    
    Eigen::Map<const Eigen::Matrix3d> L(lattice);
    double c_len = L.col(2).norm();

    std::vector<double> z_carts(n_atoms);
    for (size_t i = 0; i < n_atoms; ++i) {
        z_carts[i] = positions[3*i+2] * c_len;
    }

    std::sort(z_carts.begin(), z_carts.end());

    int layer_count = 0;
    double current_sum = z_carts[0];
    int current_cluster_size = 1;

    for (size_t i = 1; i < n_atoms; ++i) {
        if (z_carts[i] - z_carts[i-1] > layer_tolerance_a) {
            if (static_cast<size_t>(layer_count) < max_layers) {
                out_layer_centers[layer_count] = current_sum / current_cluster_size;
            }
            layer_count++;
            
            current_sum = z_carts[i];
            current_cluster_size = 1;
        } else {
            current_sum += z_carts[i];
            current_cluster_size++;
        }
    }

    if (static_cast<size_t>(layer_count) < max_layers) {
        out_layer_centers[layer_count] = current_sum / current_cluster_size;
    }
    layer_count++;

    return layer_count;
}

void shift_slab_termination(
    double* positions, size_t n_atoms,
    const double* lattice, int target_layer_idx,
    const double* layer_centers, int n_layers)
{
    if (n_atoms == 0 || target_layer_idx < 0 || target_layer_idx >= n_layers) return;

    Eigen::Map<const Eigen::Matrix3d> L(lattice);
    double c_len = L.col(2).norm();

    double z_target = layer_centers[target_layer_idx];
    double delta_f = z_target / c_len;

    for (size_t i = 0; i < n_atoms; ++i) {
        double fz = positions[3*i+2] - delta_f;
        fz = fz - std::floor(fz);
        
        // Safety against precise 1.0 wrap representation errors
        if (fz >= 1.0 - 1e-12) {
            fz = 0.0;
        }
        
        positions[3*i+2] = fz;
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
