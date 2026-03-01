// [Node 3.2] Supercell construction (Eigen) and atom count conservation tests
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Acceptance Criteria:
// - NaCl 8-atom conventional cell -> 3x3x3 supercell = 216 atoms
// - No coordinate overlap (minimum inter-atomic distance > threshold)
// - All atoms remain within the new cell boundaries
//

#include <gtest/gtest.h>
#include <cmath>
#include <vector>
#include <array>
#include <algorithm>

#include "physics_kernel.hpp"

// ===========================================================================
// Helper: NaCl conventional cell data
// ===========================================================================

namespace {

struct SupercellInput {
    double lattice[3][3];
    std::vector<std::array<double, 3>> positions;  // Fractional coordinates
    std::vector<int> types;
    int n_atoms;
};

struct SupercellResultTest {
    double lattice[3][3];
    std::vector<std::array<double, 3>> positions;  // Fractional coordinates in NEW cell
    std::vector<int> types;
    int n_atoms;
};

/// Create NaCl conventional cell (Fm-3m) with 8 atoms: 4 Na + 4 Cl
SupercellInput make_nacl_conventional() {
    SupercellInput cell;
    double a = 5.64;  // Å

    cell.lattice[0][0] = a;  cell.lattice[0][1] = 0;  cell.lattice[0][2] = 0;
    cell.lattice[1][0] = 0;  cell.lattice[1][1] = a;  cell.lattice[1][2] = 0;
    cell.lattice[2][0] = 0;  cell.lattice[2][1] = 0;  cell.lattice[2][2] = a;

    // 4 Na atoms (FCC positions) + 4 Cl atoms
    cell.positions = {
        {0.0, 0.0, 0.0},    // Na
        {0.5, 0.5, 0.0},    // Na
        {0.5, 0.0, 0.5},    // Na
        {0.0, 0.5, 0.5},    // Na
        {0.5, 0.5, 0.5},    // Cl
        {0.0, 0.0, 0.5},    // Cl
        {0.0, 0.5, 0.0},    // Cl
        {0.5, 0.0, 0.0},    // Cl
    };

    cell.types = {11, 11, 11, 11, 17, 17, 17, 17};  // Na=11, Cl=17
    cell.n_atoms = 8;
    return cell;
}

/// Compute Cartesian distance between two fractional positions in a cubic cell
double cart_distance_cubic(const std::array<double, 3>& a,
                           const std::array<double, 3>& b,
                           double lattice_a) {
    double dx = (a[0] - b[0]) * lattice_a;
    double dy = (a[1] - b[1]) * lattice_a;
    double dz = (a[2] - b[2]) * lattice_a;
    return std::sqrt(dx*dx + dy*dy + dz*dz);
}

// Wrapper to call build_supercell with simplified interface
SupercellResultTest run_supercell(const SupercellInput& input, int32_t nx, int32_t ny, int32_t nz) {
    int32_t expansion[9] = {nx, 0, 0, 0, ny, 0, 0, 0, nz};
    int n_new = get_supercell_size(input.n_atoms, expansion);
    
    SupercellResultTest res;
    res.n_atoms = n_new;
    res.positions.resize(n_new);
    res.types.resize(n_new);
    
    // Flatten inputs
    std::vector<double> flat_pos(input.n_atoms * 3);
    for(size_t i=0; i<input.positions.size(); ++i) {
        flat_pos[i*3] = input.positions[i][0];
        flat_pos[i*3+1] = input.positions[i][1];
        flat_pos[i*3+2] = input.positions[i][2];
    }
    
    std::vector<double> out_flat_pos(n_new * 3);
    
    build_supercell(
        &input.lattice[0][0],
        flat_pos.data(),
        input.types.data(),
        input.n_atoms,
        expansion,
        &res.lattice[0][0],
        out_flat_pos.data(),
        res.types.data()
    );
    
    for(int i=0; i<n_new; ++i) {
        res.positions[i][0] = out_flat_pos[i*3];
        res.positions[i][1] = out_flat_pos[i*3+1];
        res.positions[i][2] = out_flat_pos[i*3+2];
    }
    
    return res;
}

}  // anonymous namespace

// ===========================================================================
// Atom Count Conservation Tests
// ===========================================================================

/// NaCl 8 atoms x 3^3 = 216 atoms
TEST(SupercellTest, NaCl3x3x3AtomCount216) {
    auto input = make_nacl_conventional();
    ASSERT_EQ(input.n_atoms, 8) << "Input NaCl should have 8 atoms";

    auto result = run_supercell(input, 3, 3, 3);
    EXPECT_EQ(result.n_atoms, 216)
        << "3x3x3 supercell of 8-atom NaCl must have exactly 216 atoms";
}

/// 2x2x2 supercell: 8 x 8 = 64 atoms
TEST(SupercellTest, NaCl2x2x2AtomCount64) {
    auto input = make_nacl_conventional();
    auto result = run_supercell(input, 2, 2, 2);
    EXPECT_EQ(result.n_atoms, 64);
}

/// 1x1x1 "supercell" should be identical to input
TEST(SupercellTest, NaCl1x1x1Identity) {
    auto input = make_nacl_conventional();
    auto result = run_supercell(input, 1, 1, 1);
    EXPECT_EQ(result.n_atoms, 8);
    for (int i = 0; i < 8; ++i) {
        EXPECT_NEAR(result.positions[i][0], input.positions[i][0], 1e-10);
        EXPECT_NEAR(result.positions[i][1], input.positions[i][1], 1e-10);
        EXPECT_NEAR(result.positions[i][2], input.positions[i][2], 1e-10);
    }
}

// ===========================================================================
// Coordinate Integrity Tests
// ===========================================================================

/// No coordinate overlap - minimum distance between any two atoms > 0.1A
TEST(SupercellTest, NoCoordinateOverlap) {
    auto input = make_nacl_conventional();
    auto result = run_supercell(input, 3, 3, 3);
    
    double new_a = input.lattice[0][0] * 3;  // Supercell lattice constant
    double min_dist = 1e10;
    
    for (int i = 0; i < result.n_atoms; ++i) {
        for (int j = i + 1; j < result.n_atoms; ++j) {
            double d = cart_distance_cubic(
                result.positions[i], result.positions[j], new_a
            );
            min_dist = std::min(min_dist, d);
        }
    }
    
    EXPECT_GT(min_dist, 0.1)
        << "Minimum inter-atomic distance should be > 0.1 A, got: " << min_dist;
}

/// All atoms must have fractional coordinates in [0, 1) within the new supercell
TEST(SupercellTest, AllAtomsInsideBoundary) {
    auto input = make_nacl_conventional();
    auto result = run_supercell(input, 3, 3, 3);
    
    for (int i = 0; i < result.n_atoms; ++i) {
        EXPECT_GE(result.positions[i][0], 0.0)
            << "Atom " << i << " x < 0";
        EXPECT_LT(result.positions[i][0], 1.0)
            << "Atom " << i << " x >= 1";
        EXPECT_GE(result.positions[i][1], 0.0)
            << "Atom " << i << " y < 0";
        EXPECT_LT(result.positions[i][1], 1.0)
            << "Atom " << i << " y >= 1";
        EXPECT_GE(result.positions[i][2], 0.0)
            << "Atom " << i << " z < 0";
        EXPECT_LT(result.positions[i][2], 1.0)
            << "Atom " << i << " z >= 1";
    }
}

/// Supercell lattice vectors must be Nx the original
TEST(SupercellTest, LatticeVectorsScaledCorrectly) {
    auto input = make_nacl_conventional();
    auto result = run_supercell(input, 3, 3, 3);
    
    EXPECT_NEAR(result.lattice[0][0], input.lattice[0][0] * 3, 1e-10);
    EXPECT_NEAR(result.lattice[1][1], input.lattice[1][1] * 3, 1e-10);
    EXPECT_NEAR(result.lattice[2][2], input.lattice[2][2] * 3, 1e-10);
    
    // Off-diagonal should remain zero for cubic cell
    EXPECT_NEAR(result.lattice[0][1], 0.0, 1e-10);
    EXPECT_NEAR(result.lattice[0][2], 0.0, 1e-10);
}

/// Element types must be preserved (4 Na + 4 Cl per unit cell -> 108 Na + 108 Cl)
TEST(SupercellTest, ElementTypesPreserved) {
    auto input = make_nacl_conventional();
    auto result = run_supercell(input, 3, 3, 3);
    
    int na_count = 0;
    int cl_count = 0;
    for(int type : result.types) {
        if(type == 11) na_count++;
        else if(type == 17) cl_count++;
    }
    
    EXPECT_EQ(na_count, 108) << "Should have 4*27 = 108 Na atoms";
    EXPECT_EQ(cl_count, 108) << "Should have 4*27 = 108 Cl atoms";
}
