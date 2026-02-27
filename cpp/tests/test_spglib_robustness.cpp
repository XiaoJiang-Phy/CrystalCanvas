// [Node 3.1] Spglib 空间群推断的鲁棒性测试
//
// 验收标准:
// - 标准金刚石晶胞 (Fd-3m, No. 227) → 正确返回 227
// - 注入 0.001Å 随机噪音后，调整 symprec 仍返回 227
// - 畸形输入不引发 Segfault
//
// 当前状态: DISABLED_ 前缀 — 等待 Spglib 集成封装完成
//
// 编译: cd cpp/tests && cmake -B build && cmake --build build
// 运行: cd cpp/tests/build && ctest -R SpglibTest

#include <gtest/gtest.h>
#include <cmath>
#include <random>
#include <vector>
#include <array>

// TODO: #include "crystal_kernel.hpp"  // 待实现的 Spglib 封装头文件
// TODO: #include <spglib.h>  // 直接引用 (或通过封装)

// ===========================================================================
// Helper: 金刚石晶胞数据 (Fd-3m, No. 227)
// ===========================================================================

namespace {

// Diamond cubic structure (Fd-3m, No. 227)
// Lattice constant: a = 3.567 Å
// Basis atoms at: (0,0,0) and (1/4, 1/4, 1/4) in FCC conventional cell
// Full conventional cell has 8 atoms

struct TestCell {
    double lattice[3][3];  // Row-major lattice vectors
    std::vector<std::array<double, 3>> positions;  // Fractional coordinates
    std::vector<int> types;  // Atomic types
};

TestCell make_diamond_cell() {
    TestCell cell;
    double a = 3.567;  // Å

    // Cubic lattice
    cell.lattice[0][0] = a;  cell.lattice[0][1] = 0;  cell.lattice[0][2] = 0;
    cell.lattice[1][0] = 0;  cell.lattice[1][1] = a;  cell.lattice[1][2] = 0;
    cell.lattice[2][0] = 0;  cell.lattice[2][1] = 0;  cell.lattice[2][2] = a;

    // 8 carbon atoms in conventional diamond cell
    cell.positions = {
        {0.000, 0.000, 0.000},
        {0.500, 0.500, 0.000},
        {0.500, 0.000, 0.500},
        {0.000, 0.500, 0.500},
        {0.250, 0.250, 0.250},
        {0.750, 0.750, 0.250},
        {0.750, 0.250, 0.750},
        {0.250, 0.750, 0.750},
    };

    cell.types = {6, 6, 6, 6, 6, 6, 6, 6};  // All carbon
    return cell;
}

// Add random noise to all atomic positions
void add_noise(TestCell& cell, double amplitude) {
    std::mt19937 rng(42);  // Fixed seed for reproducibility
    std::uniform_real_distribution<double> dist(-amplitude, amplitude);

    for (auto& pos : cell.positions) {
        // Convert amplitude from Å to fractional coordinates
        double a = cell.lattice[0][0];
        double frac_amplitude = amplitude / a;
        std::uniform_real_distribution<double> frac_dist(-frac_amplitude, frac_amplitude);
        pos[0] += frac_dist(rng);
        pos[1] += frac_dist(rng);
        pos[2] += frac_dist(rng);
    }
}

// TODO: Wrapper function to call spglib and return space group number
// int get_spacegroup(const TestCell& cell, double symprec) {
//     SpglibDataset* dataset = spg_get_dataset(
//         cell.lattice, cell.positions.data(), cell.types.data(),
//         cell.positions.size(), symprec
//     );
//     if (!dataset) return -1;
//     int spg = dataset->spacegroup_number;
//     spg_free_dataset(dataset);
//     return spg;
// }

}  // anonymous namespace

// ===========================================================================
// 正确性测试 (DISABLED_ until Spglib wrapper is ready)
// ===========================================================================

/// Exact diamond cell → must return space group 227 (Fd-3m)
TEST(SpglibTest, DISABLED_DiamondExactReturns227) {
    auto cell = make_diamond_cell();
    // int spg = get_spacegroup(cell, 1e-5);
    // EXPECT_EQ(spg, 227) << "Exact diamond should be Fd-3m (227)";
    GTEST_SKIP() << "Awaiting Spglib integration";
}

/// Diamond with 0.001Å noise + adjusted symprec → must still return 227
TEST(SpglibTest, DISABLED_DiamondWithNoiseReturns227) {
    auto cell = make_diamond_cell();
    add_noise(cell, 0.001);  // 0.001 Å noise

    // With symprec=0.01 (10x the noise), should still identify 227
    // int spg = get_spacegroup(cell, 0.01);
    // EXPECT_EQ(spg, 227) << "Diamond + 0.001Å noise with symprec=0.01 should still be 227";
    GTEST_SKIP() << "Awaiting Spglib integration";
}

/// Diamond with larger noise (0.01Å) — test with tighter and looser symprec
TEST(SpglibTest, DISABLED_DiamondWithLargerNoiseSymprecSweep) {
    auto cell = make_diamond_cell();
    add_noise(cell, 0.01);  // 0.01 Å noise

    // With symprec=0.1 (10x the noise), should still identify correctly
    // int spg_loose = get_spacegroup(cell, 0.1);
    // EXPECT_EQ(spg_loose, 227);

    // With very tight symprec=0.001 (< noise), may NOT identify correctly
    // int spg_tight = get_spacegroup(cell, 0.001);
    // This is expected to potentially fail — just ensure no crash
    // EXPECT_GE(spg_tight, 1) << "Even with tight symprec, must return a valid space group";
    GTEST_SKIP() << "Awaiting Spglib integration";
}

/// Malformed input (zero atoms) → must not segfault, even if it returns error
TEST(SpglibTest, DISABLED_NoSegfaultOnEmptyInput) {
    TestCell cell;
    cell.lattice[0][0] = 5.0; cell.lattice[0][1] = 0; cell.lattice[0][2] = 0;
    cell.lattice[1][0] = 0;   cell.lattice[1][1] = 5.0; cell.lattice[1][2] = 0;
    cell.lattice[2][0] = 0;   cell.lattice[2][1] = 0;  cell.lattice[2][2] = 5.0;
    // No atoms
    cell.positions.clear();
    cell.types.clear();

    // Should not crash — may return -1 or error code
    // int spg = get_spacegroup(cell, 1e-5);
    // EXPECT_TRUE(spg == -1 || spg >= 1) << "Empty input should not segfault";
    GTEST_SKIP() << "Awaiting Spglib integration";
}

/// Degenerate lattice (zero volume) → must not segfault
TEST(SpglibTest, DISABLED_NoSegfaultOnDegenerateLattice) {
    TestCell cell;
    // Zero-volume lattice (all zeros)
    std::memset(cell.lattice, 0, sizeof(cell.lattice));
    cell.positions = {{0.0, 0.0, 0.0}};
    cell.types = {6};

    // Should not crash
    // int spg = get_spacegroup(cell, 1e-5);
    // EXPECT_TRUE(spg == -1 || spg >= 1);
    GTEST_SKIP() << "Awaiting Spglib integration";
}
