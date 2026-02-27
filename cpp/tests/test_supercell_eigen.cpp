// [Node 3.2] 超胞构建 (Eigen) 与原子数守恒测试
//
// 验收标准:
// - NaCl 8 原子常规晶胞 → 3×3×3 超胞 = 216 原子
// - 坐标无重叠（最小原子间距 > 阈值）
// - 所有原子在新晶胞边界内
//
// 当前状态: DISABLED_ 前缀 — 等待超胞构建模块实现
//
// 编译: cd cpp/tests && cmake -B build && cmake --build build
// 运行: cd cpp/tests/build && ctest -R SupercellTest

#include <gtest/gtest.h>
#include <cmath>
#include <vector>
#include <array>
#include <algorithm>

// TODO: #include "crystal_kernel.hpp"  // 待实现的超胞构建封装

// ===========================================================================
// Helper: NaCl 常规晶胞数据
// ===========================================================================

namespace {

struct SupercellInput {
    double lattice[3][3];
    std::vector<std::array<double, 3>> positions;  // Fractional coordinates
    std::vector<int> types;
    int n_atoms;
};

struct SupercellResult {
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

    // 4 Na atoms (FCC positions)
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

// TODO: Wrapper function to build supercell
// SupercellResult build_supercell(const SupercellInput& input, int nx, int ny, int nz) {
//     // Call Eigen-based supercell builder
//     ...
// }

}  // anonymous namespace

// ===========================================================================
// 原子数守恒测试 (DISABLED_ until supercell module is ready)
// ===========================================================================

/// NaCl 8 atoms × 3³ = 216 atoms
TEST(SupercellTest, DISABLED_NaCl3x3x3AtomCount216) {
    auto input = make_nacl_conventional();
    ASSERT_EQ(input.n_atoms, 8) << "Input NaCl should have 8 atoms";

    // auto result = build_supercell(input, 3, 3, 3);
    // EXPECT_EQ(result.n_atoms, 216)
    //     << "3x3x3 supercell of 8-atom NaCl must have exactly 216 atoms";
    GTEST_SKIP() << "Awaiting supercell module implementation";
}

/// 2×2×2 supercell: 8 × 8 = 64 atoms
TEST(SupercellTest, DISABLED_NaCl2x2x2AtomCount64) {
    auto input = make_nacl_conventional();

    // auto result = build_supercell(input, 2, 2, 2);
    // EXPECT_EQ(result.n_atoms, 64);
    GTEST_SKIP() << "Awaiting supercell module implementation";
}

/// 1×1×1 "supercell" should be identical to input
TEST(SupercellTest, DISABLED_NaCl1x1x1Identity) {
    auto input = make_nacl_conventional();

    // auto result = build_supercell(input, 1, 1, 1);
    // EXPECT_EQ(result.n_atoms, 8);
    // for (int i = 0; i < 8; ++i) {
    //     EXPECT_NEAR(result.positions[i][0], input.positions[i][0], 1e-10);
    //     EXPECT_NEAR(result.positions[i][1], input.positions[i][1], 1e-10);
    //     EXPECT_NEAR(result.positions[i][2], input.positions[i][2], 1e-10);
    // }
    GTEST_SKIP() << "Awaiting supercell module implementation";
}

// ===========================================================================
// 坐标完整性测试
// ===========================================================================

/// No coordinate overlap — minimum distance between any two atoms > 0.1Å
TEST(SupercellTest, DISABLED_NoCoordinateOverlap) {
    auto input = make_nacl_conventional();
    // auto result = build_supercell(input, 3, 3, 3);
    //
    // double new_a = input.lattice[0][0] * 3;  // Supercell lattice constant
    // double min_dist = 1e10;
    //
    // for (int i = 0; i < result.n_atoms; ++i) {
    //     for (int j = i + 1; j < result.n_atoms; ++j) {
    //         double d = cart_distance_cubic(
    //             result.positions[i], result.positions[j], new_a
    //         );
    //         min_dist = std::min(min_dist, d);
    //     }
    // }
    //
    // EXPECT_GT(min_dist, 0.1)
    //     << "Minimum inter-atomic distance should be > 0.1 Å, got: " << min_dist;
    GTEST_SKIP() << "Awaiting supercell module implementation";
}

/// All atoms must have fractional coordinates in [0, 1) within the new supercell
TEST(SupercellTest, DISABLED_AllAtomsInsideBoundary) {
    auto input = make_nacl_conventional();
    // auto result = build_supercell(input, 3, 3, 3);
    //
    // for (int i = 0; i < result.n_atoms; ++i) {
    //     EXPECT_GE(result.positions[i][0], 0.0)
    //         << "Atom " << i << " x < 0";
    //     EXPECT_LT(result.positions[i][0], 1.0)
    //         << "Atom " << i << " x >= 1";
    //     EXPECT_GE(result.positions[i][1], 0.0)
    //         << "Atom " << i << " y < 0";
    //     EXPECT_LT(result.positions[i][1], 1.0)
    //         << "Atom " << i << " y >= 1";
    //     EXPECT_GE(result.positions[i][2], 0.0)
    //         << "Atom " << i << " z < 0";
    //     EXPECT_LT(result.positions[i][2], 1.0)
    //         << "Atom " << i << " z >= 1";
    // }
    GTEST_SKIP() << "Awaiting supercell module implementation";
}

/// Supercell lattice vectors must be N× the original
TEST(SupercellTest, DISABLED_LatticeVectorsScaledCorrectly) {
    auto input = make_nacl_conventional();
    // auto result = build_supercell(input, 3, 3, 3);
    //
    // EXPECT_NEAR(result.lattice[0][0], input.lattice[0][0] * 3, 1e-10);
    // EXPECT_NEAR(result.lattice[1][1], input.lattice[1][1] * 3, 1e-10);
    // EXPECT_NEAR(result.lattice[2][2], input.lattice[2][2] * 3, 1e-10);
    //
    // // Off-diagonal should remain zero for cubic cell
    // EXPECT_NEAR(result.lattice[0][1], 0.0, 1e-10);
    // EXPECT_NEAR(result.lattice[0][2], 0.0, 1e-10);
    GTEST_SKIP() << "Awaiting supercell module implementation";
}

/// Element types must be preserved (4 Na + 4 Cl per unit cell → 108 Na + 108 Cl)
TEST(SupercellTest, DISABLED_ElementTypesPreserved) {
    auto input = make_nacl_conventional();
    // auto result = build_supercell(input, 3, 3, 3);
    //
    // int na_count = std::count(result.types.begin(), result.types.end(), 11);
    // int cl_count = std::count(result.types.begin(), result.types.end(), 17);
    //
    // EXPECT_EQ(na_count, 108) << "Should have 4*27 = 108 Na atoms";
    // EXPECT_EQ(cl_count, 108) << "Should have 4*27 = 108 Cl atoms";
    GTEST_SKIP() << "Awaiting supercell module implementation";
}
