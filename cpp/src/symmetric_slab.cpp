// [Overview: Composition-preserving symmetric slab construction.]
// Implementation: bulk symmetry cut phases followed by finite-slab site matching.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
#include "physics_kernel.hpp"
#include "physics_kernel_internal.hpp"
#include <spglib.h>
#include <algorithm>
#include <cmath>
#include <memory>
#include <numeric>
#include <vector>

namespace {
using Matrix3d = Eigen::Matrix<double, 3, 3, Eigen::ColMajor>;
using Matrix3i = Eigen::Matrix<int, 3, 3, Eigen::ColMajor>;
using Dataset = std::unique_ptr<SpglibDataset, decltype(&spg_free_dataset)>;
constexpr double SYMMETRY_TOLERANCE_A = 1e-5;
constexpr size_t MAX_CUT_PHASES = 128;
constexpr size_t MAX_PAIR_CHECKS = 4'000'000;

double wrap(double value) { return value - std::floor(value); }

Dataset symmetry_dataset(const Matrix3d& lattice, std::vector<double>& positions,
                         const std::vector<int>& classes) {
    // Spglib's C matrix has Cartesian rows and lattice-vector columns.
    double cell[3][3];
    for (int row = 0; row < 3; ++row)
        for (int col = 0; col < 3; ++col) cell[row][col] = lattice(row, col);
    return Dataset(spg_get_dataset(cell, reinterpret_cast<double(*)[3]>(positions.data()),
                                  classes.data(), static_cast<int>(classes.size()),
                                  SYMMETRY_TOLERANCE_A), spg_free_dataset);
}

// No z wrapping: a symmetry of the periodic bulk is not sufficient for a slab.
bool finite_slab_symmetry(const Matrix3d& lattice, std::vector<double>& positions,
                          const std::vector<int>& classes, size_t& checks) {
    auto dataset = symmetry_dataset(lattice, positions, classes);
    if (!dataset) return false;
    const size_t count = classes.size();
    std::vector<size_t> order(count);
    std::iota(order.begin(), order.end(), 0);
    std::sort(order.begin(), order.end(), [&](size_t a, size_t b) {
        return positions[3 * a + 2] < positions[3 * b + 2];
    });
    const double z_tolerance = SYMMETRY_TOLERANCE_A / lattice.col(2).norm();
    for (int operation = 0; operation < dataset->n_operations; ++operation) {
        const auto& rotation = dataset->rotations[operation];
        if (rotation[2][0] != 0 || rotation[2][1] != 0 || rotation[2][2] != -1) continue;
        std::vector<bool> used(count, false);
        bool matches = true;
        for (size_t i = 0; i < count && matches; ++i) {
            Eigen::Vector3d mapped;
            for (int row = 0; row < 2; ++row) {
                mapped[row] = dataset->translations[operation][row];
                for (int col = 0; col < 3; ++col)
                    mapped[row] += rotation[row][col] * positions[3 * i + col];
            }
            // The centered finite envelope fixes the normal translation to one.
            mapped.z() = 1.0 - positions[3 * i + 2];
            auto begin = std::lower_bound(order.begin(), order.end(), mapped.z() - z_tolerance,
                [&](size_t index, double height) { return positions[3 * index + 2] < height; });
            bool found = false;
            for (auto it = begin; it != order.end(); ++it) {
                const size_t j = *it;
                if (positions[3 * j + 2] > mapped.z() + z_tolerance) break;
                if (++checks > MAX_PAIR_CHECKS) return false;
                if (used[j] || classes[i] != classes[j]) continue;
                Eigen::Vector3d delta = mapped - Eigen::Map<const Eigen::Vector3d>(&positions[3 * j]);
                delta.x() -= std::round(delta.x());
                delta.y() -= std::round(delta.y());
                if ((lattice * delta).norm() <= SYMMETRY_TOLERANCE_A) {
                    used[j] = true;
                    found = true;
                    break;
                }
            }
            matches = found;
        }
        if (matches) return true;
    }
    return false;
}
}

int build_symmetric_slab(
    const double* lattice, const double* positions, const int* site_classes,
    size_t n_atoms, const int32_t* miller, int n_layers, double vacuum_a,
    size_t output_capacity, double* out_lattice, double* out_positions,
    int* out_source_indices) noexcept {
    try {
        if (!lattice || !positions || !site_classes || !miller || !out_lattice
            || !out_positions || !out_source_indices || !std::isfinite(vacuum_a)
            || vacuum_a < 0.0) return 0;
        const int count = get_slab_size_v2(lattice, miller, n_layers, n_atoms);
        if (count <= 0 || static_cast<size_t>(count) > output_capacity) return 0;
        const Eigen::Map<const Matrix3d> input_cell(lattice);
        Matrix3i basis;
        if (!get_surface_basis(input_cell, miller[0], miller[1], miller[2], basis)) return 0;
        const int divisor = std::gcd(std::gcd(std::abs(miller[0]), std::abs(miller[1])), std::abs(miller[2]));
        const Eigen::Vector3d normal_index(double(miller[0]) / divisor,
            double(miller[1]) / divisor, double(miller[2]) / divisor);
        const Eigen::Vector3d reciprocal = input_cell.transpose().fullPivLu().solve(normal_index);
        const double period = 1.0 / reciprocal.norm();
        if (!std::isfinite(period) || period <= 0.0) return 0;
        std::vector<double> bulk(positions, positions + 3 * n_atoms);
        for (double& value : bulk) {
            if (!std::isfinite(value)) return 0;
            value = wrap(value);
        }
        std::vector<int> classes(site_classes, site_classes + n_atoms);
        if (std::any_of(classes.begin(), classes.end(), [](int value) { return value <= 0; })) return 0;
        auto dataset = symmetry_dataset(input_cell, bulk, classes);
        if (!dataset) return -1;
        std::vector<double> phases;
        for (int operation = 0; operation < dataset->n_operations; ++operation) {
            Eigen::RowVector3d transformed = Eigen::RowVector3d::Zero();
            for (int col = 0; col < 3; ++col)
                for (int row = 0; row < 3; ++row)
                    transformed[col] += normal_index[row] * dataset->rotations[operation][row][col];
            if (!(transformed.transpose() + normal_index).isZero(1e-10)) continue;
            const Eigen::Map<const Eigen::Vector3d> translation(dataset->translations[operation]);
            for (int half = 0; half < 2; ++half) {
                const double phase = wrap((normal_index.dot(translation) - n_layers + half) / 2.0);
                if (std::none_of(phases.begin(), phases.end(), [&](double existing) {
                    const double delta = phase - existing;
                    return std::abs(delta - std::round(delta)) * period <= SYMMETRY_TOLERANCE_A;
                })) {
                    if (phases.size() == MAX_CUT_PHASES) return -3;
                    phases.push_back(phase);
                }
            }
        }
        if (phases.empty()) return -1;
        std::sort(phases.begin(), phases.end());
        std::vector<int> source_indices(n_atoms);
        std::iota(source_indices.begin(), source_indices.end(), 0);
        std::vector<double> shifted(bulk.size()), candidate_positions(3 * count);
        std::vector<int> candidate_sources(count), candidate_classes(count);
        Matrix3d candidate_cell;
        size_t checks = 0;
        for (double phase : phases) {
            bool boundary_clear = true;
            for (size_t i = 0; i < n_atoms; ++i) {
                const Eigen::Map<const Eigen::Vector3d> point(&bulk[3 * i]);
                const double height = normal_index.dot(point) - phase;
                if (std::abs(height - std::round(height)) * period <= SYMMETRY_TOLERANCE_A) {
                    boundary_clear = false;
                    break;
                }
                const Eigen::Vector3d moved = point - phase * basis.col(2).cast<double>();
                for (int axis = 0; axis < 3; ++axis) shifted[3 * i + axis] = wrap(moved[axis]);
            }
            if (!boundary_clear) continue;
            if (build_slab_v2(lattice, shifted.data(), source_indices.data(), n_atoms,
                    miller, n_layers, vacuum_a, count, candidate_cell.data(),
                    candidate_positions.data(), candidate_sources.data()) != count) return 0;
            double minimum = 1.0, maximum = 0.0;
            for (int i = 0; i < count; ++i) {
                minimum = std::min(minimum, candidate_positions[3 * i + 2]);
                maximum = std::max(maximum, candidate_positions[3 * i + 2]);
                candidate_classes[i] = classes[candidate_sources[i]];
            }
            const double offset = 0.5 - (minimum + maximum) / 2.0;
            for (int i = 0; i < count; ++i) candidate_positions[3 * i + 2] += offset;
            if (finite_slab_symmetry(candidate_cell, candidate_positions, candidate_classes, checks)) {
                std::copy(candidate_cell.data(), candidate_cell.data() + 9, out_lattice);
                std::copy(candidate_positions.begin(), candidate_positions.end(), out_positions);
                std::copy(candidate_sources.begin(), candidate_sources.end(), out_source_indices);
                return count;
            }
            if (checks > MAX_PAIR_CHECKS) return -3;
        }
        return -2;
    } catch (...) {
        return 0;
    }
}
