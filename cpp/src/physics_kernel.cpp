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
#include <limits>
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

int niggli_reduce(double* lattice, double symprec) {
    try {
        double spg_lattice[3][3] = {
            {lattice[0], lattice[3], lattice[6]},
            {lattice[1], lattice[4], lattice[7]},
            {lattice[2], lattice[5], lattice[8]}
        };
        int result = spg_niggli_reduce(spg_lattice, symprec);
        if (result == 0) return 1; // spglib returns 0 on failure
        // Write back transposed
        lattice[0] = spg_lattice[0][0]; lattice[3] = spg_lattice[0][1]; lattice[6] = spg_lattice[0][2];
        lattice[1] = spg_lattice[1][0]; lattice[4] = spg_lattice[1][1]; lattice[7] = spg_lattice[1][2];
        lattice[2] = spg_lattice[2][0]; lattice[5] = spg_lattice[2][1]; lattice[8] = spg_lattice[2][2];
        return 0; // our API returns 0 on success
    } catch (...) {
        return 1;
    }
}

int delaunay_reduce(double* lattice, double symprec) {
    try {
        double spg_lattice[3][3] = {
            {lattice[0], lattice[3], lattice[6]},
            {lattice[1], lattice[4], lattice[7]},
            {lattice[2], lattice[5], lattice[8]}
        };
        int result = spg_delaunay_reduce(spg_lattice, symprec);
        if (result == 0) return 1; // failure
        lattice[0] = spg_lattice[0][0]; lattice[3] = spg_lattice[0][1]; lattice[6] = spg_lattice[0][2];
        lattice[1] = spg_lattice[1][0]; lattice[4] = spg_lattice[1][1]; lattice[7] = spg_lattice[1][2];
        lattice[2] = spg_lattice[2][0]; lattice[5] = spg_lattice[2][1]; lattice[8] = spg_lattice[2][2];
        return 0;
    } catch (...) {
        return 1;
    }
}

int standardize_cell(double* lattice, double* positions, int* types,
                     size_t n_atoms, size_t capacity, int to_primitive, double symprec) {
    try {
        double spg_lattice[3][3] = {
            {lattice[0], lattice[3], lattice[6]},
            {lattice[1], lattice[4], lattice[7]},
            {lattice[2], lattice[5], lattice[8]}
        };
        
        std::vector<double> pos_vec(capacity * 3, 0.0);
        std::copy(positions, positions + n_atoms * 3, pos_vec.begin());
        double(*spg_positions)[3] = reinterpret_cast<double(*)[3]>(pos_vec.data());

        std::vector<int> types_vec(capacity, 0);
        std::copy(types, types + n_atoms, types_vec.begin());

        int no_idealize = 0; // we want idealized lengths and angles
        int num_new_atoms = spg_standardize_cell(
            spg_lattice, spg_positions, types_vec.data(), 
            static_cast<int>(n_atoms), to_primitive, no_idealize, symprec);
        
        if (num_new_atoms == 0) return 0; // failure
        if (static_cast<size_t>(num_new_atoms) > capacity) return 0; // buffer overflow guard

        // Write back lattice
        lattice[0] = spg_lattice[0][0]; lattice[3] = spg_lattice[0][1]; lattice[6] = spg_lattice[0][2];
        lattice[1] = spg_lattice[1][0]; lattice[4] = spg_lattice[1][1]; lattice[7] = spg_lattice[1][2];
        lattice[2] = spg_lattice[2][0]; lattice[5] = spg_lattice[2][1]; lattice[8] = spg_lattice[2][2];

        // Write back positions and types
        std::copy(pos_vec.begin(), pos_vec.begin() + num_new_atoms * 3, positions);
        std::copy(types_vec.begin(), types_vec.begin() + num_new_atoms, types);
        
        return num_new_atoms;
    } catch (...) {
        return 0;
    }
}

#include <Eigen/Dense>

namespace {

using ColMajorMatrix3d = Eigen::Matrix<double, 3, 3, Eigen::ColMajor>;
using ColMajorMatrix3i = Eigen::Matrix<int, 3, 3, Eigen::ColMajor>;
using ColMajorMatrix3i64 = Eigen::Matrix<int64_t, 3, 3, Eigen::ColMajor>;
using Vector3i64 = Eigen::Matrix<int64_t, 3, 1>;

constexpr int MAX_MILLER_INDEX_ABS = 128;
constexpr size_t MAX_STRUCTURAL_ATOMS = 10'000;
constexpr size_t MAX_ENUMERATION_OVERHEAD = 256;

struct SupercellPlan {
    int min_shift[3];
    int max_shift[3];
    size_t output_atoms;
};

[[nodiscard]] bool checked_add_i64(int64_t left, int64_t right, int64_t& output)
{
    if ((right > 0 && left > std::numeric_limits<int64_t>::max() - right)
        || (right < 0 && left < std::numeric_limits<int64_t>::min() - right)) {
        return false;
    }
    output = left + right;
    return true;
}

[[nodiscard]] bool checked_sub_i64(int64_t left, int64_t right, int64_t& output)
{
    if ((right > 0 && left < std::numeric_limits<int64_t>::min() + right)
        || (right < 0 && left > std::numeric_limits<int64_t>::max() + right)) {
        return false;
    }
    output = left - right;
    return true;
}

[[nodiscard]] bool checked_mul_i64(int64_t left, int64_t right, int64_t& output)
{
    if (left == 0 || right == 0) {
        output = 0;
        return true;
    }
    if (left == -1) {
        if (right == std::numeric_limits<int64_t>::min()) return false;
        output = -right;
        return true;
    }
    if (right == -1) {
        if (left == std::numeric_limits<int64_t>::min()) return false;
        output = -left;
        return true;
    }
    if ((left > 0 && right > 0 && left > std::numeric_limits<int64_t>::max() / right)
        || (left > 0 && right < 0 && right < std::numeric_limits<int64_t>::min() / left)
        || (left < 0 && right > 0 && left < std::numeric_limits<int64_t>::min() / right)
        || (left < 0 && right < 0 && left < std::numeric_limits<int64_t>::max() / right)) {
        return false;
    }
    output = left * right;
    return true;
}

[[nodiscard]] bool checked_size_mul(size_t left, size_t right, size_t& output)
{
    if (left != 0 && right > std::numeric_limits<size_t>::max() / left) {
        return false;
    }
    output = left * right;
    return true;
}

[[nodiscard]] bool checked_round_i64(double value, int64_t& output)
{
    const double rounded = std::round(value);
    constexpr double INT64_LOWER_EXCLUSIVE = -9'223'372'036'854'775'808.0;
    constexpr double INT64_UPPER_EXCLUSIVE = 9'223'372'036'854'775'808.0;
    if (!std::isfinite(rounded)
        || rounded <= INT64_LOWER_EXCLUSIVE
        || rounded >= INT64_UPPER_EXCLUSIVE) {
        return false;
    }
    output = static_cast<int64_t>(rounded);
    return true;
}

[[nodiscard]] bool subtract_scaled(
    Vector3i64& target, int64_t scale, const Vector3i64& basis)
{
    Vector3i64 next;
    for (int row = 0; row < 3; ++row) {
        int64_t product = 0;
        if (!checked_mul_i64(scale, basis[row], product)
            || !checked_sub_i64(target[row], product, next[row])) {
            return false;
        }
    }
    target = next;
    return true;
}

[[nodiscard]] bool determinant_i64(const ColMajorMatrix3i& matrix, int64_t& output)
{
    const auto value = [&matrix](int row, int column) {
        return static_cast<int64_t>(matrix(row, column));
    };
    int64_t first_minor = 0;
    int64_t second_minor = 0;
    int64_t third_minor = 0;
    int64_t product = 0;
    if (!checked_mul_i64(value(1, 1), value(2, 2), first_minor)
        || !checked_mul_i64(value(1, 2), value(2, 1), product)
        || !checked_sub_i64(first_minor, product, first_minor)
        || !checked_mul_i64(value(1, 0), value(2, 2), second_minor)
        || !checked_mul_i64(value(1, 2), value(2, 0), product)
        || !checked_sub_i64(second_minor, product, second_minor)
        || !checked_mul_i64(value(1, 0), value(2, 1), third_minor)
        || !checked_mul_i64(value(1, 1), value(2, 0), product)
        || !checked_sub_i64(third_minor, product, third_minor)
        || !checked_mul_i64(value(0, 0), first_minor, first_minor)
        || !checked_mul_i64(value(0, 1), second_minor, second_minor)
        || !checked_mul_i64(value(0, 2), third_minor, third_minor)
        || !checked_sub_i64(first_minor, second_minor, output)
        || !checked_add_i64(output, third_minor, output)) {
        return false;
    }
    return true;
}

[[nodiscard]] bool determinant_i64(const ColMajorMatrix3i64& matrix, int64_t& output)
{
    int64_t first_minor = 0;
    int64_t second_minor = 0;
    int64_t third_minor = 0;
    int64_t product = 0;
    if (!checked_mul_i64(matrix(1, 1), matrix(2, 2), first_minor)
        || !checked_mul_i64(matrix(1, 2), matrix(2, 1), product)
        || !checked_sub_i64(first_minor, product, first_minor)
        || !checked_mul_i64(matrix(1, 0), matrix(2, 2), second_minor)
        || !checked_mul_i64(matrix(1, 2), matrix(2, 0), product)
        || !checked_sub_i64(second_minor, product, second_minor)
        || !checked_mul_i64(matrix(1, 0), matrix(2, 1), third_minor)
        || !checked_mul_i64(matrix(1, 1), matrix(2, 0), product)
        || !checked_sub_i64(third_minor, product, third_minor)
        || !checked_mul_i64(matrix(0, 0), first_minor, first_minor)
        || !checked_mul_i64(matrix(0, 1), second_minor, second_minor)
        || !checked_mul_i64(matrix(0, 2), third_minor, third_minor)
        || !checked_sub_i64(first_minor, second_minor, output)
        || !checked_add_i64(output, third_minor, output)) {
        return false;
    }
    return true;
}

void ext_gcd(int64_t h, int64_t k, int64_t& u, int64_t& v, int64_t& g)
{
    if (k == 0) {
        u = h < 0 ? -1 : 1;
        v = 0;
        g = std::abs(h);
        return;
    }
    int64_t u1 = 0;
    int64_t v1 = 0;
    ext_gcd(k, h % k, u1, v1, g);
    u = v1;
    v = u1 - (h / k) * v1;
}

[[nodiscard]] bool try_get_surface_basis(
    const ColMajorMatrix3d& lattice, int64_t h, int64_t k, int64_t l,
    ColMajorMatrix3i& output)
{
    if (!lattice.allFinite() || (h == 0 && k == 0 && l == 0)) return false;
    const double lattice_determinant = lattice.determinant();
    if (!std::isfinite(lattice_determinant) || lattice_determinant == 0.0) return false;

    const int64_t g_all = std::gcd(std::gcd(std::abs(h), std::abs(k)), std::abs(l));
    h /= g_all;
    k /= g_all;
    l /= g_all;

    Vector3i64 v1;
    Vector3i64 v2;
    Vector3i64 v3;
    int64_t y_kl = 0;
    int64_t z_kl = 0;
    int64_t g_kl = 0;
    ext_gcd(k, l, y_kl, z_kl, g_kl);

    if (g_kl == 0) {
        v1 << 0, 1, 0;
        v2 << 0, 0, 1;
        v3 << (h > 0 ? 1 : -1), 0, 0;
    } else {
        int64_t x_hg = 0;
        int64_t p_hg = 0;
        int64_t unused = 0;
        ext_gcd(h, g_kl, x_hg, p_hg, unused);
        int64_t hy = 0;
        int64_t hz = 0;
        int64_t py = 0;
        int64_t pz = 0;
        if (!checked_mul_i64(h, y_kl, hy)
            || !checked_mul_i64(h, z_kl, hz)
            || !checked_mul_i64(p_hg, y_kl, py)
            || !checked_mul_i64(p_hg, z_kl, pz)) {
            return false;
        }
        v1 << 0, l / g_kl, -k / g_kl;
        v2 << -g_kl, hy, hz;
        v3 << x_hg, py, pz;
    }

    for (int iteration = 0; iteration < 20; ++iteration) {
        Eigen::Vector3d c1 = lattice * v1.cast<double>();
        Eigen::Vector3d c2 = lattice * v2.cast<double>();
        if (!c1.allFinite() || !c2.allFinite()) return false;
        if (c2.squaredNorm() < c1.squaredNorm()) {
            std::swap(v1, v2);
            std::swap(c1, c2);
        }
        const double denominator = c1.squaredNorm();
        if (!std::isfinite(denominator) || denominator <= 0.0) return false;
        int64_t reduction = 0;
        if (!checked_round_i64(c1.dot(c2) / denominator, reduction)) return false;
        if (reduction == 0) break;
        if (!subtract_scaled(v2, reduction, v1)) return false;
    }

    Eigen::Vector3d c1 = lattice * v1.cast<double>();
    Eigen::Vector3d c2 = lattice * v2.cast<double>();
    Eigen::Vector3d c3 = lattice * v3.cast<double>();
    if (!c1.allFinite() || !c2.allFinite() || !c3.allFinite()) return false;
    const double c1_norm = c1.squaredNorm();
    const double c2_norm = c2.squaredNorm();
    int64_t p1 = 0;
    int64_t p2 = 0;
    if (c1_norm > 1e-12 && !checked_round_i64(c3.dot(c1) / c1_norm, p1)) return false;
    if (c2_norm > 1e-12 && !checked_round_i64(c3.dot(c2) / c2_norm, p2)) return false;
    if (!subtract_scaled(v3, p1, v1) || !subtract_scaled(v3, p2, v2)) return false;

    ColMajorMatrix3i64 basis;
    basis.col(0) = v1;
    basis.col(1) = v2;
    basis.col(2) = v3;
    int64_t determinant = 0;
    if (!determinant_i64(basis, determinant)) return false;
    if (determinant == -1) {
        for (int row = 0; row < 3; ++row) {
            if (basis(row, 0) == std::numeric_limits<int64_t>::min()) return false;
            basis(row, 0) = -basis(row, 0);
        }
        determinant = 1;
    }
    if (determinant != 1) return false;

    for (int column = 0; column < 3; ++column) {
        for (int row = 0; row < 3; ++row) {
            const int64_t value = basis(row, column);
            if (value < std::numeric_limits<int>::min()
                || value > std::numeric_limits<int>::max()) {
                return false;
            }
            output(row, column) = static_cast<int>(value);
        }
    }
    return true;
}

[[nodiscard]] bool valid_miller_indices(const int32_t* miller)
{
    const auto h = static_cast<int64_t>(miller[0]);
    const auto k = static_cast<int64_t>(miller[1]);
    const auto l = static_cast<int64_t>(miller[2]);
    return (h != 0 || k != 0 || l != 0)
        && std::abs(h) <= MAX_MILLER_INDEX_ABS
        && std::abs(k) <= MAX_MILLER_INDEX_ABS
        && std::abs(l) <= MAX_MILLER_INDEX_ABS;
}

[[nodiscard]] bool make_supercell_plan(
    const int32_t* expansion, size_t n_atoms, size_t output_atoms,
    SupercellPlan& output)
{
    if (n_atoms == 0 || output_atoms == 0 || output_atoms > MAX_STRUCTURAL_ATOMS) {
        return false;
    }

    size_t enumeration_width = 1;
    for (int row = 0; row < 3; ++row) {
        int64_t minimum = 0;
        int64_t maximum = 0;
        for (int column = 0; column < 3; ++column) {
            const int64_t value = expansion[column * 3 + row];
            if (!checked_add_i64(minimum, std::min(value, int64_t{0}), minimum)
                || !checked_add_i64(maximum, std::max(value, int64_t{0}), maximum)) {
                return false;
            }
        }
        if (minimum < static_cast<int64_t>(std::numeric_limits<int>::min()) + 1
            || maximum > static_cast<int64_t>(std::numeric_limits<int>::max()) - 2) {
            return false;
        }
        int64_t span = 0;
        if (!checked_sub_i64(maximum, minimum, span)
            || !checked_add_i64(span, 3, span)
            || span <= 0) {
            return false;
        }
        size_t width = static_cast<size_t>(span);
        if (!checked_size_mul(enumeration_width, width, enumeration_width)) return false;
        output.min_shift[row] = static_cast<int>(minimum) - 1;
        output.max_shift[row] = static_cast<int>(maximum) + 1;
    }

    size_t work_items = 0;
    size_t work_limit = 0;
    if (!checked_size_mul(n_atoms, enumeration_width, work_items)
        || !checked_size_mul(output_atoms, MAX_ENUMERATION_OVERHEAD, work_limit)
        || work_items > work_limit) {
        return false;
    }
    output.output_atoms = output_atoms;
    return true;
}

[[nodiscard]] bool supercell_output_atoms(
    const int32_t* expansion, size_t n_atoms, size_t& output)
{
    if (expansion == nullptr || n_atoms == 0) return false;
    const ColMajorMatrix3i matrix = Eigen::Map<const ColMajorMatrix3i>(expansion);
    int64_t determinant = 0;
    if (!determinant_i64(matrix, determinant)
        || determinant <= 0) {
        return false;
    }
    const auto copies = static_cast<size_t>(determinant);
    return checked_size_mul(n_atoms, copies, output) && output != 0;
}

[[nodiscard]] bool make_slab_plan(
    const ColMajorMatrix3d& lattice, const int32_t* miller,
    int n_layers, size_t n_atoms, SupercellPlan& plan,
    ColMajorMatrix3i& expansion)
{
    if (!valid_miller_indices(miller) || n_layers <= 0) return false;
    size_t output_atoms = 0;
    if (!checked_size_mul(n_atoms, static_cast<size_t>(n_layers), output_atoms)
        || output_atoms > MAX_STRUCTURAL_ATOMS) {
        return false;
    }

    ColMajorMatrix3i surface_basis;
    if (!try_get_surface_basis(
            lattice, miller[0], miller[1], miller[2], surface_basis)) {
        return false;
    }
    expansion = surface_basis;
    for (int row = 0; row < 3; ++row) {
        int64_t scaled = 0;
        if (!checked_mul_i64(surface_basis(row, 2), n_layers, scaled)
            || scaled < std::numeric_limits<int>::min()
            || scaled > std::numeric_limits<int>::max()) {
            return false;
        }
        expansion(row, 2) = static_cast<int>(scaled);
    }
    if (!make_supercell_plan(expansion.data(), n_atoms, output_atoms, plan)) return false;
    int64_t determinant = 0;
    return determinant_i64(expansion, determinant) && determinant == n_layers;
}

}

[[nodiscard]] int get_supercell_size(size_t n_atoms, const int32_t* expansion) {
    size_t output_atoms = 0;
    if (!supercell_output_atoms(expansion, n_atoms, output_atoms)) return 0;
    const auto max_output = static_cast<size_t>(std::numeric_limits<int>::max());
    return output_atoms <= max_output ? static_cast<int>(output_atoms) : 0;
}

[[nodiscard]] int build_supercell_checked(
    const double* lattice,
    const double* positions,
    const int* types,
    size_t n_atoms,
    const int32_t* expansion,
    size_t output_capacity,
    double* out_lattice,
    double* out_positions,
    int* out_types
) {
    try {
        if (lattice == nullptr || positions == nullptr || types == nullptr
            || expansion == nullptr || out_lattice == nullptr
            || out_positions == nullptr || out_types == nullptr) {
            return 0;
        }
        size_t output_atoms = 0;
        if (!supercell_output_atoms(expansion, n_atoms, output_atoms)
            || output_atoms > output_capacity) {
            return 0;
        }
        SupercellPlan plan;
        if (!make_supercell_plan(expansion, n_atoms, output_atoms, plan)) return 0;

        Eigen::Map<const ColMajorMatrix3d> L(lattice);
        Eigen::Map<const ColMajorMatrix3i> P(expansion);
        if (!L.allFinite()) return 0;
        const ColMajorMatrix3d p_double = P.cast<double>();
        const Eigen::FullPivLU<ColMajorMatrix3d> p_lu(p_double);
        if (!p_lu.isInvertible()) return 0;

        size_t out_idx = 0;
        for (size_t i = 0; i < n_atoms; ++i) {
            Eigen::Vector3d f(positions[3*i], positions[3*i+1], positions[3*i+2]);
            if (!f.allFinite()) return 0;
            const int type = types[i];
            for (int nx = plan.min_shift[0]; nx <= plan.max_shift[0]; ++nx) {
                for (int ny = plan.min_shift[1]; ny <= plan.max_shift[1]; ++ny) {
                    for (int nz = plan.min_shift[2]; nz <= plan.max_shift[2]; ++nz) {
                        Eigen::Vector3d shift(nx, ny, nz);
                        Eigen::Vector3d f_new = p_lu.solve(f + shift);
                        if (!f_new.allFinite()) return 0;
                        const double eps = 1e-5;
                        if (f_new.x() >= -eps && f_new.x() < 1.0 - eps &&
                            f_new.y() >= -eps && f_new.y() < 1.0 - eps &&
                            f_new.z() >= -eps && f_new.z() < 1.0 - eps) {
                            if (out_idx >= plan.output_atoms) return 0;
                            f_new.x() -= std::floor(f_new.x());
                            f_new.y() -= std::floor(f_new.y());
                            f_new.z() -= std::floor(f_new.z());
                            out_positions[out_idx * 3] = f_new.x();
                            out_positions[out_idx * 3 + 1] = f_new.y();
                            out_positions[out_idx * 3 + 2] = f_new.z();
                            out_types[out_idx] = type;
                            ++out_idx;
                        }
                    }
                }
            }
        }
        if (out_idx != plan.output_atoms) return 0;
        const ColMajorMatrix3d output_lattice = L * p_double;
        if (!output_lattice.allFinite()) return 0;
        Eigen::Map<ColMajorMatrix3d> L_out(out_lattice);
        L_out = output_lattice;
        return static_cast<int>(out_idx);
    } catch (...) {
        return 0;
    }
}

void build_supercell(
    const double* lattice, const double* positions,
    const int* types, size_t n_atoms, const int32_t* expansion,
    double* out_lattice, double* out_positions, int* out_types)
{
    const int expected = get_supercell_size(n_atoms, expansion);
    if (expected <= 0) return;
    static_cast<void>(build_supercell_checked(
        lattice, positions, types, n_atoms, expansion,
        static_cast<size_t>(expected), out_lattice, out_positions, out_types));
}

[[nodiscard]] bool get_surface_basis(
    const Eigen::Ref<const ColMajorMatrix3d>& lattice, int h, int k, int l,
    ColMajorMatrix3i& output)
{
    return try_get_surface_basis(lattice, h, k, l, output);
}

int get_slab_size(
    const double* lattice,
    const int32_t* miller,
    int layers,
    double vacuum_A,
    size_t n_atoms
) {
    (void)vacuum_A;
    if (lattice == nullptr || miller == nullptr) return 0;
    Eigen::Map<const ColMajorMatrix3d> lattice_matrix(lattice);
    SupercellPlan plan;
    ColMajorMatrix3i expansion;
    if (!make_slab_plan(lattice_matrix, miller, layers, n_atoms, plan, expansion)) return 0;
    return static_cast<int>(plan.output_atoms);
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
    if (lattice == nullptr || positions == nullptr || types == nullptr
        || miller == nullptr || out_lattice == nullptr
        || out_positions == nullptr || out_types == nullptr) {
        return;
    }
    Eigen::Map<const ColMajorMatrix3d> lattice_matrix(lattice);
    SupercellPlan plan;
    ColMajorMatrix3i expansion;
    if (!make_slab_plan(lattice_matrix, miller, layers, n_atoms, plan, expansion)) return;
    const int actual_atoms = build_supercell_checked(
        lattice, positions, types, n_atoms, expansion.data(), plan.output_atoms,
        out_lattice, out_positions, out_types);
    if (actual_atoms <= 0 || static_cast<size_t>(actual_atoms) != plan.output_atoms) return;

    Eigen::Map<ColMajorMatrix3d> L_out(out_lattice);
    Eigen::Vector3d c_vec = L_out.col(2);
    double c_len = c_vec.norm();
    if (!std::isfinite(c_len) || c_len <= 0.0) return;
    double scale = (c_len + vacuum_A) / c_len;
    if (!std::isfinite(scale) || scale <= 0.0) return;
    L_out.col(2) *= scale;

    for (size_t i = 0; i < plan.output_atoms; ++i) {
        out_positions[3*i+2] /= scale;
        out_positions[3*i+2] += (vacuum_A / 2.0) / (c_len + vacuum_A);
    }
}

[[nodiscard]] int get_slab_size_v2(
    const double* lattice, const int32_t* miller,
    int n_layers, size_t n_atoms)
{
    if (lattice == nullptr || miller == nullptr) return 0;
    Eigen::Map<const ColMajorMatrix3d> L(lattice);
    SupercellPlan plan;
    ColMajorMatrix3i expansion;
    if (!make_slab_plan(L, miller, n_layers, n_atoms, plan, expansion)) return 0;
    return static_cast<int>(plan.output_atoms);
}

[[nodiscard]] int build_slab_v2(
    const double* lattice, const double* positions,
    const int* types, size_t n_atoms,
    const int32_t* miller, int n_layers, double vacuum_a,
    size_t output_capacity, double* out_lattice,
    double* out_positions, int* out_types) noexcept
{
    try {
        if (lattice == nullptr || positions == nullptr || types == nullptr
            || miller == nullptr || out_lattice == nullptr
            || out_positions == nullptr || out_types == nullptr
            || !std::isfinite(vacuum_a) || vacuum_a < 0.0) {
            return 0;
        }
        Eigen::Map<const ColMajorMatrix3d> L(lattice);
        SupercellPlan plan;
        ColMajorMatrix3i expansion;
        if (!make_slab_plan(L, miller, n_layers, n_atoms, plan, expansion)) return 0;
        if (plan.output_atoms > output_capacity) return 0;
        const int total_new_atoms = build_supercell_checked(
            lattice, positions, types, n_atoms, expansion.data(), output_capacity,
            out_lattice, out_positions, out_types);
        if (total_new_atoms <= 0
            || static_cast<size_t>(total_new_atoms) != plan.output_atoms) {
            return 0;
        }

        Eigen::Map<ColMajorMatrix3d> L_out(out_lattice);
        if (!L_out.allFinite()) return 0;
        std::vector<int> indices(total_new_atoms);
        std::iota(indices.begin(), indices.end(), 0);
        std::vector<Eigen::Vector3d> cart_pos(total_new_atoms);
        for (int i = 0; i < total_new_atoms; ++i) {
            const Eigen::Vector3d fractional(
                out_positions[3 * i], out_positions[3 * i + 1], out_positions[3 * i + 2]);
            if (!fractional.allFinite()) return 0;
            cart_pos[i] = L_out * fractional;
            if (!cart_pos[i].allFinite()) return 0;
        }

        std::sort(indices.begin(), indices.end(), [&](int left, int right) {
            const Eigen::Vector3d& lhs = cart_pos[left];
            const Eigen::Vector3d& rhs = cart_pos[right];
            if (lhs.x() != rhs.x()) return lhs.x() < rhs.x();
            if (lhs.y() != rhs.y()) return lhs.y() < rhs.y();
            if (lhs.z() != rhs.z()) return lhs.z() < rhs.z();
            return left < right;
        });

        std::vector<int> unique_indices;
        unique_indices.reserve(total_new_atoms);
        for (const int index : indices) {
            bool duplicate = false;
            for (auto it = unique_indices.rbegin(); it != unique_indices.rend(); ++it) {
                const int unique_index = *it;
                if (cart_pos[index].x() - cart_pos[unique_index].x() > 1e-3) break;
                const double distance = (cart_pos[index] - cart_pos[unique_index]).stableNorm();
                if (!std::isfinite(distance)) return 0;
                if (distance < 1e-4) {
                    duplicate = true;
                    break;
                }
            }
            if (!duplicate) unique_indices.push_back(index);
        }
        if (unique_indices.empty()
            || unique_indices.size() > static_cast<size_t>(std::numeric_limits<int>::max())) {
            return 0;
        }

        const int n_unique = static_cast<int>(unique_indices.size());
        std::vector<double> final_pos(static_cast<size_t>(n_unique) * 3);
        std::vector<int> final_types(n_unique);
        for (int i = 0; i < n_unique; ++i) {
            const int original_index = unique_indices[i];
            final_pos[3 * i] = out_positions[3 * original_index];
            final_pos[3 * i + 1] = out_positions[3 * original_index + 1];
            final_pos[3 * i + 2] = out_positions[3 * original_index + 2];
            final_types[i] = out_types[original_index];
        }
        std::copy(final_pos.begin(), final_pos.end(), out_positions);
        std::copy(final_types.begin(), final_types.end(), out_types);

        const Eigen::Vector3d a_vec = L_out.col(0);
        const Eigen::Vector3d b_vec = L_out.col(1);
        const Eigen::Vector3d c_vec = L_out.col(2);
        const Eigen::Vector3d normal = a_vec.cross(b_vec);
        if (!normal.allFinite()) return 0;
        const double normal_len = normal.stableNorm();
        if (!std::isfinite(normal_len) || normal_len <= 0.0) return 0;
        Eigen::Vector3d n_hat = normal / normal_len;
        if (!n_hat.allFinite()) return 0;

        double height = c_vec.dot(n_hat);
        if (!std::isfinite(height)) return 0;
        if (height < 0.0) {
            n_hat = -n_hat;
            height = -height;
        }
        if (!std::isfinite(height) || height <= 0.0) return 0;

        const double aa = a_vec.squaredNorm();
        const double ab = a_vec.dot(b_vec);
        const double bb = b_vec.squaredNorm();
        const double ac = a_vec.dot(c_vec);
        const double bc = b_vec.dot(c_vec);
        if (!std::isfinite(aa) || !std::isfinite(ab) || !std::isfinite(bb)
            || !std::isfinite(ac) || !std::isfinite(bc)) {
            return 0;
        }
        const double det_g = aa * bb - ab * ab;
        if (!std::isfinite(det_g) || det_g <= 0.0) return 0;
        const double alpha = (bb * ac - ab * bc) / det_g;
        const double beta = (aa * bc - ab * ac) / det_g;
        const double c_new_len = height + vacuum_a;
        if (!std::isfinite(alpha) || !std::isfinite(beta)
            || !std::isfinite(c_new_len) || c_new_len <= 0.0) {
            return 0;
        }

        for (int i = 0; i < n_unique; ++i) {
            const double fx = out_positions[3 * i];
            const double fy = out_positions[3 * i + 1];
            const double fz = out_positions[3 * i + 2];
            if (!std::isfinite(fx) || !std::isfinite(fy) || !std::isfinite(fz)) return 0;
            double nx = fx + alpha * fz;
            double ny = fy + beta * fz;
            const double nz = fz * height / c_new_len + vacuum_a / (2.0 * c_new_len);
            if (!std::isfinite(nx) || !std::isfinite(ny) || !std::isfinite(nz)) return 0;
            nx -= std::floor(nx);
            ny -= std::floor(ny);
            if (!std::isfinite(nx) || !std::isfinite(ny)) return 0;
            out_positions[3 * i] = nx;
            out_positions[3 * i + 1] = ny;
            out_positions[3 * i + 2] = nz;
        }

        const Eigen::Vector3d new_c = n_hat * c_new_len;
        if (!new_c.allFinite()) return 0;
        L_out.col(2) = new_c;
        Eigen::HouseholderQR<ColMajorMatrix3d> qr(L_out);
        ColMajorMatrix3d upper = qr.matrixQR().triangularView<Eigen::Upper>();
        if (!upper.allFinite()) return 0;
        for (int i = 0; i < 3; ++i) {
            if (upper(i, i) < 0.0) upper.row(i) *= -1.0;
        }
        if (!upper.allFinite()) return 0;
        L_out = upper;
        return n_unique;
    } catch (...) {
        return 0;
    }
}

[[nodiscard]] int cluster_slab_layers(
    const double* positions, size_t n_atoms,
    const double* lattice,
    double layer_tolerance_a,
    double* out_layer_centers, size_t max_layers)
{
    if (n_atoms == 0 || max_layers == 0) return 0;
    
    Eigen::Map<const ColMajorMatrix3d> L(lattice);
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

    Eigen::Map<const ColMajorMatrix3d> L(lattice);
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
