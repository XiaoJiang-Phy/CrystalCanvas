#include <gtest/gtest.h>
#include <Eigen/Dense>
#include "physics_kernel.hpp"
#include <vector>
#include <cmath>
#include <numeric>
#include <algorithm>

// [Breaker Mode] S2 Gate Tests — build_slab_v2 / get_slab_size_v2

namespace {

// ColMajor storage: columns are basis vectors
Eigen::Matrix3d make_sc(double a) {
    Eigen::Matrix3d L = Eigen::Matrix3d::Identity() * a;
    return L;
}

Eigen::Matrix3d make_fcc(double a) {
    Eigen::Matrix3d L;
    L << 0,   a/2, a/2,
         a/2, 0,   a/2,
         a/2, a/2, 0;
    return L;
}

// NaCl conventional: Na at origin (0,0,0); Cl at (0.5,0.5,0.5)
// 2-atom primitive with types {0=Na, 1=Cl}
struct Crystal {
    Eigen::Matrix3d lattice;        // ColMajor
    std::vector<double> positions;  // fractional, flat [x0,y0,z0, x1,y1,z1...]
    std::vector<int>    types;
    size_t n_atoms() const { return types.size(); }
};

Crystal make_sc_crystal(double a) {
    Crystal c;
    c.lattice = make_sc(a);
    c.positions = {0.0, 0.0, 0.0};
    c.types = {0};
    return c;
}

Crystal make_fcc_crystal(double a) {
    Crystal c;
    c.lattice = make_fcc(a);
    c.positions = {0.0, 0.0, 0.0};
    c.types = {0};
    return c;
}

// 4-atom conventional FCC cell
Crystal make_fcc_conventional_crystal(double a) {
    Crystal c;
    c.lattice = Eigen::Matrix3d::Identity() * a;  // cubic
    c.positions = {
        0.0, 0.0, 0.0,
        0.5, 0.5, 0.0,
        0.5, 0.0, 0.5,
        0.0, 0.5, 0.5
    };
    c.types = {0, 0, 0, 0};
    return c;
}

Crystal make_nacl_crystal(double a) {
    // NaCl primitive cell: a=5.64 Å
    // Basis: Na (0,0,0), Cl (0.5,0.5,0.5) in fractional of FCC primitive cell
    Crystal c;
    c.lattice = make_fcc(a);
    c.positions = {
        0.0, 0.0, 0.0,      // Na
        0.5, 0.5, 0.5       // Cl
    };
    c.types = {0, 1};
    return c;
}

struct SlabResult {
    Eigen::Matrix3d  lattice;
    std::vector<double> positions;
    std::vector<int>    types;
    int n_atoms;
};

SlabResult run_slab_v2(const Crystal& cr, int h, int k, int l, int n_layers, double vacuum_a) {
    int32_t miller[3] = {h, k, l};
    int upper_bound = get_slab_size_v2(
        cr.lattice.data(), miller, n_layers, cr.n_atoms());

    SlabResult res;
    res.lattice = Eigen::Matrix3d::Zero();
    res.positions.resize(upper_bound * 3, 0.0);
    res.types.resize(upper_bound, 0);

    res.n_atoms = build_slab_v2(
        cr.lattice.data(), cr.positions.data(), cr.types.data(), cr.n_atoms(),
        miller, n_layers, vacuum_a,
        res.lattice.data(), res.positions.data(), res.types.data());

    res.positions.resize(res.n_atoms * 3);
    res.types.resize(res.n_atoms);
    return res;
}

bool has_duplicates(const SlabResult& res, const double tol = 1e-4) {
    for (int i = 0; i < res.n_atoms; ++i) {
        Eigen::Vector3d ri = res.lattice * Eigen::Vector3d(
            res.positions[3*i], res.positions[3*i+1], res.positions[3*i+2]);
        for (int j = i+1; j < res.n_atoms; ++j) {
            Eigen::Vector3d rj = res.lattice * Eigen::Vector3d(
                res.positions[3*j], res.positions[3*j+1], res.positions[3*j+2]);
            if ((ri - rj).norm() < tol) return true;
        }
    }
    return false;
}

bool all_frac_in_unit_cell(const SlabResult& res) {
    constexpr double eps = 1e-7;
    for (int i = 0; i < res.n_atoms; ++i) {
        for (int d = 0; d < 3; ++d) {
            double f = res.positions[3*i + d];
            if (f < -eps || f >= 1.0 + eps) return false;
        }
    }
    return true;
}

// Reciprocal-lattice surface normal for Miller (h,k,l): n_hat = (L^{-T} * G).normalized()
Eigen::Vector3d surface_normal(const Eigen::Matrix3d& L, int h, int k, int l) {
    Eigen::Vector3d G(h, k, l);
    // n_hat = L^{-T} G / |L^{-T} G|  — exact normal to the (hkl) plane
    Eigen::Vector3d n = L.inverse().transpose() * G;
    return n.normalized();
}

} // namespace

// ===========================================================================
// S2 Gate Tests — Plan-mandated assertions
// ===========================================================================

TEST(SlabBuilderTest, SC100_AtomCount) {
    auto cr = make_sc_crystal(3.0);
    auto res = run_slab_v2(cr, 1, 0, 0, 3, 10.0);
    EXPECT_EQ(res.n_atoms, 3) << "SC (1,0,0) 3L must have exactly 3 atoms.";
}

// FCC primitive cell (1,0,0) 3L: surface basis det=1 × 3 layers × 1 atom/cell = 3
TEST(SlabBuilderTest, FCC_Primitive100_AtomCount) {
    auto cr = make_fcc_crystal(4.05);
    auto res = run_slab_v2(cr, 1, 0, 0, 3, 10.0);
    EXPECT_EQ(res.n_atoms, 3) << "FCC primitive (1,0,0) 3L must have exactly 3 atoms.";
}

// FCC conventional cell (1,0,0) 3L: 4 atoms/cell × 3 planes = 12 atoms
TEST(SlabBuilderTest, FCC_Conventional100_AtomCount) {
    auto cr = make_fcc_conventional_crystal(4.05);
    auto res = run_slab_v2(cr, 1, 0, 0, 3, 10.0);
    EXPECT_EQ(res.n_atoms, 12) << "FCC conventional (1,0,0) 3L must have exactly 12 atoms.";
}

TEST(SlabBuilderTest, FCC111_AtomCount) {
    auto cr = make_fcc_crystal(4.05);
    auto res = run_slab_v2(cr, 1, 1, 1, 3, 10.0);
    EXPECT_EQ(res.n_atoms, 3) << "FCC (1,1,1) 3L must have exactly 3 atoms — one per layer.";
}

TEST(SlabBuilderTest, FCC111_LayerSpacing) {
    const double a = 4.05;
    auto cr = make_fcc_crystal(a);
    auto res = run_slab_v2(cr, 1, 1, 1, 3, 10.0);

    // d_{111} for FCC primitive = a / sqrt(3).
    // We project each atom onto the physical surface normal n_hat = (L^{-T} * G).normalized()
    // not the c-axis, which is non-orthogonal to the surface in primitive cells.
    const double expected_spacing = a / std::sqrt(3.0);

    const Eigen::Matrix3d& L_slab = res.lattice;
    Eigen::Vector3d n_hat = surface_normal(cr.lattice, 1, 1, 1);

    std::vector<double> z_projs;
    for (int i = 0; i < res.n_atoms; ++i) {
        Eigen::Vector3d f(res.positions[3*i], res.positions[3*i+1], res.positions[3*i+2]);
        Eigen::Vector3d r = L_slab * f;
        z_projs.push_back(r.dot(n_hat));
    }
    std::sort(z_projs.begin(), z_projs.end());

    ASSERT_EQ((int)z_projs.size(), 3);
    double sp1 = z_projs[1] - z_projs[0];
    double sp2 = z_projs[2] - z_projs[1];
    EXPECT_NEAR(sp1, expected_spacing, 1e-4);
    EXPECT_NEAR(sp2, expected_spacing, 1e-4);
}

TEST(SlabBuilderTest, SC100_VacuumLength) {
    const double a = 3.0;
    const double vac = 15.0;
    auto cr = make_sc_crystal(a);
    auto res = run_slab_v2(cr, 1, 0, 0, 3, vac);

    double c_len = res.lattice.col(2).norm();
    EXPECT_NEAR(c_len, 3 * a + vac, 1e-4)
        << "c-axis must equal 3a + vacuum.";
}

TEST(SlabBuilderTest, FCC111_NoDuplicates_5L) {
    auto cr = make_fcc_crystal(4.05);
    auto res = run_slab_v2(cr, 1, 1, 1, 5, 10.0);
    EXPECT_FALSE(has_duplicates(res))
        << "No duplicate atoms allowed after deduplication.";
}

TEST(SlabBuilderTest, FracBounds_All) {
    auto cr_sc  = make_sc_crystal(3.0);
    auto cr_fcc = make_fcc_crystal(4.05);

    auto res1 = run_slab_v2(cr_sc,  1, 0, 0, 3, 10.0);
    auto res2 = run_slab_v2(cr_fcc, 1, 0, 0, 3, 10.0);
    auto res3 = run_slab_v2(cr_fcc, 1, 1, 1, 5, 10.0);

    for (auto* res : {&res1, &res2, &res3}) {
        for (int i = 0; i < res->n_atoms; ++i) {
            for (int d = 0; d < 3; ++d) {
                double f = res->positions[3*i + d];
                EXPECT_GE(f, -1e-7) << "Fractional coord must be >= 0";
                EXPECT_LT(f, 1.0 + 1e-7) << "Fractional coord must be < 1";
            }
        }
    }
}

TEST(SlabBuilderTest, TypePreservation_NaCl100) {
    const double a = 5.64;
    auto cr = make_nacl_crystal(a);
    auto res = run_slab_v2(cr, 1, 0, 0, 3, 10.0);

    int n_na = std::count(res.types.begin(), res.types.end(), 0);
    int n_cl = std::count(res.types.begin(), res.types.end(), 1);
    EXPECT_EQ(n_na + n_cl, res.n_atoms) << "All atoms must be Na or Cl.";
    EXPECT_GT(n_na, 0) << "Na must be present.";
    EXPECT_GT(n_cl, 0) << "Cl must be present.";
    // For FCC-primitive NaCl (1,0,0) 3L, stoichiometry must be 1:1
    EXPECT_EQ(n_na, n_cl) << "NaCl (1,0,0) stoichiometry must be 1:1.";
}

// ===========================================================================
// [Breaker] Pathological Attack Tests
// ===========================================================================

TEST(SlabBuilderTest, ZeroVacuum) {
    auto cr = make_sc_crystal(3.0);
    auto res = run_slab_v2(cr, 1, 0, 0, 3, 0.0);
    // Should not crash; atom count must still be 3
    EXPECT_EQ(res.n_atoms, 3);
    double c_len = res.lattice.col(2).norm();
    EXPECT_NEAR(c_len, 9.0, 1e-4);
}

TEST(SlabBuilderTest, NegativeVacuumClamped) {
    auto cr = make_sc_crystal(3.0);
    // vacuum_a = -1 must be clamped to 0, not crash or distort
    auto res = run_slab_v2(cr, 1, 0, 0, 3, -1.0);
    EXPECT_EQ(res.n_atoms, 3);
    double c_len = res.lattice.col(2).norm();
    EXPECT_NEAR(c_len, 9.0, 1e-4) << "Negative vacuum must be clamped to 0.";
}

TEST(SlabBuilderTest, SingleLayer) {
    auto cr = make_sc_crystal(3.0);
    auto res = run_slab_v2(cr, 1, 0, 0, 1, 10.0);
    EXPECT_EQ(res.n_atoms, 1);
}

TEST(SlabBuilderTest, LargeLayerCount) {
    auto cr = make_sc_crystal(3.0);
    auto res = run_slab_v2(cr, 1, 0, 0, 10, 10.0);
    EXPECT_EQ(res.n_atoms, 10);
    EXPECT_FALSE(has_duplicates(res));
}

TEST(SlabBuilderTest, LargeMillerIndex_FCC) {
    auto cr = make_fcc_crystal(4.05);
    // High-index surface — should not crash, atom count > 0
    auto res = run_slab_v2(cr, 2, 1, 0, 3, 10.0);
    EXPECT_GT(res.n_atoms, 0);
    EXPECT_FALSE(has_duplicates(res));
}

TEST(SlabBuilderTest, GetSlabSizeV2_MatchesBuildOutput) {
    auto cr = make_fcc_crystal(4.05);
    int32_t miller[3] = {1, 1, 1};
    int upper = get_slab_size_v2(cr.lattice.data(), miller, 3, cr.n_atoms());
    auto res = run_slab_v2(cr, 1, 1, 1, 3, 10.0);
    // Upper bound must be >= actual atom count, and not absurdly large
    EXPECT_GE(upper, res.n_atoms);
    EXPECT_LE(upper, res.n_atoms * 4);
}
