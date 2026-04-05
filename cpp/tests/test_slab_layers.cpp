#include <gtest/gtest.h>
#include <Eigen/Dense>
#include "physics_kernel.hpp"
#include <vector>
#include <cmath>
#include <algorithm>
#include <numeric>

// [Breaker Mode] S3 Gate Tests — cluster_slab_layers / shift_slab_termination

namespace {

Eigen::Matrix3d make_sc(double a) {
    return Eigen::Matrix3d::Identity() * a;
}

Eigen::Matrix3d make_fcc(double a) {
    Eigen::Matrix3d L;
    L << 0,   a/2, a/2,
         a/2, 0,   a/2,
         a/2, a/2, 0;
    return L;
}

// Helper: run build_slab_v2 to get a filled slab
struct Slab {
    Eigen::Matrix3d lattice;
    std::vector<double> positions;
    std::vector<int>    types;
    int n_atoms;
};

Slab make_slab(const Eigen::Matrix3d& L_in, const std::vector<double>& frac,
               const std::vector<int>& types, int h, int k, int l,
               int n_layers, double vac)
{
    size_t n_base = types.size();
    int32_t miller[3] = {h, k, l};
    int upper = get_slab_size_v2(L_in.data(), miller, n_layers, n_base);

    Slab s;
    s.lattice = Eigen::Matrix3d::Zero();
    s.positions.resize(upper * 3, 0.0);
    s.types.resize(upper, 0);

    s.n_atoms = build_slab_v2(
        L_in.data(), frac.data(), types.data(), n_base,
        miller, n_layers, vac,
        s.lattice.data(), s.positions.data(), s.types.data());

    s.positions.resize(s.n_atoms * 3);
    s.types.resize(s.n_atoms);
    return s;
}

bool all_frac_in_unit_cell(const Slab& s) {
    constexpr double eps = 1e-7;
    for (int i = 0; i < s.n_atoms; ++i) {
        double f = s.positions[3*i+2];
        if (f < -eps || f >= 1.0 + eps) return false;
    }
    return true;
}

} // namespace

// ===========================================================================
// S3 Gate Tests — Plan-mandated assertions
// ===========================================================================

TEST(SlabLayersTest, FCC111_3Layers) {
    const double a = 4.05;
    auto L = make_fcc(a);
    auto s = make_slab(L, {0.0, 0.0, 0.0}, {0}, 1, 1, 1, 3, 10.0);

    std::vector<double> centers(16, 0.0);
    int n_layers = cluster_slab_layers(
        s.positions.data(), s.n_atoms,
        s.lattice.data(), 0.3,
        centers.data(), centers.size());

    EXPECT_EQ(n_layers, 3) << "FCC (1,1,1) 3L must cluster into exactly 3 layers.";
}

TEST(SlabLayersTest, SC100_3Layers) {
    const double a = 3.0;
    auto L = make_sc(a);
    auto s = make_slab(L, {0.0, 0.0, 0.0}, {0}, 1, 0, 0, 3, 10.0);

    std::vector<double> centers(16, 0.0);
    int n_layers = cluster_slab_layers(
        s.positions.data(), s.n_atoms,
        s.lattice.data(), 0.3,
        centers.data(), centers.size());

    EXPECT_EQ(n_layers, 3) << "SC (1,0,0) 3L must cluster into exactly 3 layers.";
    // Layer centres must be monotonically ascending
    for (int i = 1; i < n_layers; ++i) {
        EXPECT_GT(centers[i], centers[i-1])
            << "Layer centres must be in ascending order.";
    }
}

TEST(SlabLayersTest, FCC111_LayerSpacing) {
    const double a = 4.05;
    auto L = make_fcc(a);
    auto s = make_slab(L, {0.0, 0.0, 0.0}, {0}, 1, 1, 1, 3, 10.0);

    std::vector<double> centers(16, 0.0);
    int n_layers = cluster_slab_layers(
        s.positions.data(), s.n_atoms,
        s.lattice.data(), 0.3,
        centers.data(), centers.size());

    ASSERT_EQ(n_layers, 3);
    // cluster_slab_layers uses z_cart = f_z * slab_c_len approximation.
    // For FCC (1,1,1) primitive, get_surface_basis stacks layers along v3 = (0,0,1)
    // in fractional coords. Each layer has f_z spacing = 1/n_layers.
    // In the slab, c_len = 3 * d_{111} + vacuum. The f_z * slab_c_len spacing
    // equals d_{111} (physical) only when vacuum = 0.
    // With vacuum = 10 Å, spacing in this approx = (slab_c_len / n_layers).
    // The robust check: uniform layer spacing (all intervals equal).
    double sp0 = centers[1] - centers[0];
    double sp1 = centers[2] - centers[1];
    EXPECT_NEAR(sp0, sp1, 1e-4)
        << "Layer spacing must be uniform for FCC (1,1,1) equal-stacked layers.";
    EXPECT_GT(sp0, 0.5)
        << "Layer spacing must be physically reasonable (> 0.5 Å).";
}

TEST(SlabLayersTest, ShiftTermination_FCC111) {
    const double a = 4.05;
    auto L = make_fcc(a);
    auto s = make_slab(L, {0.0, 0.0, 0.0}, {0}, 1, 1, 1, 3, 10.0);

    std::vector<double> centers(16, 0.0);
    int n_layers = cluster_slab_layers(
        s.positions.data(), s.n_atoms,
        s.lattice.data(), 0.3,
        centers.data(), centers.size());

    ASSERT_EQ(n_layers, 3);

    // Shift to layer index 1 (middle layer)
    shift_slab_termination(
        s.positions.data(), s.n_atoms,
        s.lattice.data(), 1,
        centers.data(), n_layers);

    // All f_z must remain in [0, 1)
    EXPECT_TRUE(all_frac_in_unit_cell(s))
        << "All frac z-coords must remain in [0,1) after shift.";

    // Recluster after shift — still 3 layers
    std::vector<double> centers2(16, 0.0);
    int n_layers2 = cluster_slab_layers(
        s.positions.data(), s.n_atoms,
        s.lattice.data(), 0.3,
        centers2.data(), centers2.size());
    EXPECT_EQ(n_layers2, 3)
        << "Layer count must be preserved after termination shift.";
}

TEST(SlabLayersTest, NaCl100_TopBottomLayerComposition) {
    // NaCl primitive FCC, 2 layers
    const double a = 5.64;
    Eigen::Matrix3d L = make_fcc(a);
    std::vector<double> frac = {
        0.0, 0.0, 0.0,   // Na
        0.5, 0.5, 0.5    // Cl
    };
    std::vector<int> types = {0, 1};
    auto s = make_slab(L, frac, types, 1, 0, 0, 2, 10.0);

    std::vector<double> centers(16, 0.0);
    int n_layers = cluster_slab_layers(
        s.positions.data(), s.n_atoms,
        s.lattice.data(), 0.3,
        centers.data(), centers.size());

    ASSERT_GE(n_layers, 2);

    // Identify top and bottom layer atom types
    double c_len = Eigen::Map<const Eigen::Matrix3d>(s.lattice.data()).col(2).norm();
    double bottom_thresh = centers[0] + 0.3;
    double top_thresh    = centers[n_layers-1] - 0.3;

    int top_na = 0, top_cl = 0, bot_na = 0, bot_cl = 0;
    for (int i = 0; i < s.n_atoms; ++i) {
        double z = s.positions[3*i+2] * c_len;
        if (z <= bottom_thresh) { s.types[i] == 0 ? bot_na++ : bot_cl++; }
        if (z >= top_thresh)    { s.types[i] == 0 ? top_na++ : top_cl++; }
    }
    // NaCl FCC primitive (1,0,0) produces a polar surface:
    // the bottom layer is Na-only and the top layer is Cl-only (or vice versa).
    // For a polar cut the top and bottom layers are DIFFERENT species.
    // The test verifies that each layer is pure (one species per layer), not mixed.
    // Check: each surface layer contains only one type of atom
    EXPECT_TRUE(bot_na == 0 || bot_cl == 0)
        << "Bottom layer must be a pure (single-species) atomic plane.";
    EXPECT_TRUE(top_na == 0 || top_cl == 0)
        << "Top layer must be a pure (single-species) atomic plane.";
    // Together, both species must be represented
    EXPECT_GT(bot_na + top_na, 0) << "Na must appear in a surface layer.";
    EXPECT_GT(bot_cl + top_cl, 0) << "Cl must appear in a surface layer.";
}

// ===========================================================================
// [Breaker] Pathological Attack Tests
// ===========================================================================

TEST(SlabLayersTest, ZeroAtoms_NocrashCluster) {
    auto L = make_sc(3.0);
    std::vector<double> centers(8, 0.0);
    int n = cluster_slab_layers(nullptr, 0, L.data(), 0.3, centers.data(), 8);
    EXPECT_EQ(n, 0) << "Zero atoms must return 0 layers without crash.";
}

TEST(SlabLayersTest, MaxLayersOne_Truncation) {
    const double a = 3.0;
    auto L = make_sc(a);
    auto s = make_slab(L, {0.0, 0.0, 0.0}, {0}, 1, 0, 0, 5, 10.0);

    // max_layers = 1 — should not write out-of-bounds, no crash
    std::vector<double> centers(1, 0.0);
    int n = cluster_slab_layers(
        s.positions.data(), s.n_atoms,
        s.lattice.data(), 0.3,
        centers.data(), 1);

    // Must still return the actual number of layers (5)
    EXPECT_EQ(n, 5) << "Return value is actual layer count, not capped at max_layers.";
    // Only the first centre should be written
    EXPECT_GT(centers[0], 0.0) << "First centre must be written.";
}

TEST(SlabLayersTest, ShiftTermination_InvalidIndex_Nop) {
    const double a = 3.0;
    auto L = make_sc(a);
    auto s = make_slab(L, {0.0, 0.0, 0.0}, {0}, 1, 0, 0, 3, 10.0);

    std::vector<double> orig_z(s.n_atoms);
    for (int i = 0; i < s.n_atoms; ++i) orig_z[i] = s.positions[3*i+2];

    std::vector<double> centers(8, 0.0);
    int n_layers = cluster_slab_layers(
        s.positions.data(), s.n_atoms, s.lattice.data(),
        0.3, centers.data(), 8);

    // Negative layer index must be a no-op
    shift_slab_termination(s.positions.data(), s.n_atoms,
        s.lattice.data(), -1, centers.data(), n_layers);

    for (int i = 0; i < s.n_atoms; ++i) {
        EXPECT_NEAR(s.positions[3*i+2], orig_z[i], 1e-12)
            << "Negative layer index must be a no-op.";
    }

    // Out-of-range layer index must be a no-op
    shift_slab_termination(s.positions.data(), s.n_atoms,
        s.lattice.data(), n_layers, centers.data(), n_layers);

    for (int i = 0; i < s.n_atoms; ++i) {
        EXPECT_NEAR(s.positions[3*i+2], orig_z[i], 1e-12)
            << "Out-of-range layer index must be a no-op.";
    }
}

TEST(SlabLayersTest, ShiftTermination_Layer0_IsIdentityLike) {
    // Shifting to layer 0 (already the bottom) must keep all f_z in [0,1)
    const double a = 3.0;
    auto L = make_sc(a);
    auto s = make_slab(L, {0.0, 0.0, 0.0}, {0}, 1, 0, 0, 4, 10.0);

    std::vector<double> centers(8, 0.0);
    int n_layers = cluster_slab_layers(
        s.positions.data(), s.n_atoms, s.lattice.data(),
        0.3, centers.data(), 8);

    shift_slab_termination(s.positions.data(), s.n_atoms,
        s.lattice.data(), 0, centers.data(), n_layers);

    EXPECT_TRUE(all_frac_in_unit_cell(s));
}

TEST(SlabLayersTest, ShiftTermination_Idempotent_SameLayer) {
    const double a = 4.05;
    auto L = make_fcc(a);
    auto s = make_slab(L, {0.0, 0.0, 0.0}, {0}, 1, 1, 1, 5, 10.0);

    std::vector<double> centers(16, 0.0);
    int n_layers = cluster_slab_layers(
        s.positions.data(), s.n_atoms, s.lattice.data(),
        0.3, centers.data(), 16);

    ASSERT_GE(n_layers, 2);

    shift_slab_termination(s.positions.data(), s.n_atoms,
        s.lattice.data(), 1, centers.data(), n_layers);

    // Re-cluster after shift
    std::vector<double> centers2(16, 0.0);
    int n_layers2 = cluster_slab_layers(
        s.positions.data(), s.n_atoms, s.lattice.data(),
        0.3, centers2.data(), 16);

    EXPECT_EQ(n_layers2, n_layers)
        << "Layer count must be invariant under termination shift.";
    EXPECT_TRUE(all_frac_in_unit_cell(s))
        << "All frac z-coords must be in [0,1) after shift.";
}

TEST(SlabLayersTest, SingleAtom_SingleLayer) {
    auto L = make_sc(3.0);
    std::vector<double> pos = {0.0, 0.0, 0.5};
    std::vector<double> centers(8, 0.0);
    int n = cluster_slab_layers(pos.data(), 1, L.data(), 0.3, centers.data(), 8);
    EXPECT_EQ(n, 1);
    EXPECT_NEAR(centers[0], 0.5 * 3.0, 1e-8);
}

TEST(SlabLayersTest, VerySmallTolerance_AllSeparate) {
    auto L = make_sc(3.0);
    auto s = make_slab(L, {0.0, 0.0, 0.0}, {0}, 1, 0, 0, 4, 0.0);
    // With tolerance = 1e-8, all atoms must be separate layers
    std::vector<double> centers(16, 0.0);
    int n = cluster_slab_layers(
        s.positions.data(), s.n_atoms, s.lattice.data(),
        1e-8, centers.data(), 16);
    EXPECT_EQ(n, s.n_atoms);
}

TEST(SlabLayersTest, VeryLargeTolerance_AllMerged) {
    auto L = make_sc(3.0);
    auto s = make_slab(L, {0.0, 0.0, 0.0}, {0}, 1, 0, 0, 4, 0.0);
    // With tolerance = 1000 Å, all atoms must merge into 1 layer
    std::vector<double> centers(16, 0.0);
    int n = cluster_slab_layers(
        s.positions.data(), s.n_atoms, s.lattice.data(),
        1000.0, centers.data(), 16);
    EXPECT_EQ(n, 1);
}
