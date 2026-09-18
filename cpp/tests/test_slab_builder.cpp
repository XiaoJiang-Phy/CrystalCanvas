#include <gtest/gtest.h>
#include <Eigen/Dense>
#include "physics_kernel.hpp"
#include "physics_kernel_internal.hpp"
#include <vector>
#include <cmath>
#include <numeric>
#include <algorithm>

// [Breaker Mode] S2 Gate Tests — build_slab_v2 / get_slab_size_v2

namespace {

using ColMajorMatrix3d = Eigen::Matrix<double, 3, 3, Eigen::ColMajor>;

// ColMajor storage: columns are basis vectors
ColMajorMatrix3d make_sc(double a) {
    ColMajorMatrix3d L = ColMajorMatrix3d::Identity() * a;
    return L;
}

ColMajorMatrix3d make_fcc(double a) {
    ColMajorMatrix3d L;
    L << 0,   a/2, a/2,
         a/2, 0,   a/2,
         a/2, a/2, 0;
    return L;
}

// NaCl conventional: Na at origin (0,0,0); Cl at (0.5,0.5,0.5)
// 2-atom primitive with types {0=Na, 1=Cl}
struct Crystal {
    ColMajorMatrix3d lattice;
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
    c.lattice = ColMajorMatrix3d::Identity() * a;  // cubic
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
    ColMajorMatrix3d lattice;
    std::vector<double> positions;
    std::vector<int>    types;
    int n_atoms;
};

SlabResult run_slab_v2(const Crystal& cr, int h, int k, int l, int n_layers, double vacuum_a) {
    int32_t miller[3] = {h, k, l};
    int upper_bound = get_slab_size_v2(
        cr.lattice.data(), miller, n_layers, cr.n_atoms());

    SlabResult res;
    res.lattice = ColMajorMatrix3d::Zero();
    if (upper_bound <= 0) {
        res.n_atoms = 0;
        return res;
    }
    res.positions.resize(upper_bound * 3, 0.0);
    res.types.resize(upper_bound, 0);

    res.n_atoms = build_slab_v2(
        cr.lattice.data(), cr.positions.data(), cr.types.data(), cr.n_atoms(),
        miller, n_layers, vacuum_a,
        static_cast<size_t>(upper_bound),
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
    ASSERT_EQ(res.n_atoms, 3);

    const double expected_spacing = a / std::sqrt(3.0);
    const ColMajorMatrix3d& L_slab = res.lattice;
    const Eigen::Vector3d normal = L_slab.col(0).cross(L_slab.col(1));
    const double normal_len = normal.stableNorm();
    const double c_len = L_slab.col(2).stableNorm();
    ASSERT_TRUE(std::isfinite(normal_len));
    ASSERT_TRUE(std::isfinite(c_len));
    ASSERT_GT(normal_len, 0.0);
    ASSERT_GT(c_len, 0.0);

    Eigen::Vector3d n_hat = normal / normal_len;
    const Eigen::Vector3d c_hat = L_slab.col(2) / c_len;
    EXPECT_NEAR(std::abs(n_hat.dot(c_hat)), 1.0, 1e-6);
    if (n_hat.dot(c_hat) < 0.0) n_hat = -n_hat;

    std::vector<double> z_projs;
    z_projs.reserve(static_cast<size_t>(res.n_atoms));
    for (int i = 0; i < res.n_atoms; ++i) {
        Eigen::Vector3d f(res.positions[3*i], res.positions[3*i+1], res.positions[3*i+2]);
        Eigen::Vector3d r = L_slab * f;
        z_projs.push_back(r.dot(n_hat));
    }
    std::sort(z_projs.begin(), z_projs.end());

    ASSERT_EQ(z_projs.size(), 3U);
    const double sp1 = z_projs[1] - z_projs[0];
    const double sp2 = z_projs[2] - z_projs[1];
    EXPECT_NEAR(sp1, expected_spacing, 1e-5);
    EXPECT_NEAR(sp2, expected_spacing, 1e-5);
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

TEST(SlabBuilderTest, NegativeVacuumRejected) {
    auto cr = make_sc_crystal(3.0);
    auto res = run_slab_v2(cr, 1, 0, 0, 3, -1.0);
    EXPECT_EQ(res.n_atoms, 0);
}

TEST(SlabBuilderTest, InsufficientOutputCapacityRejectsWithoutWrites) {
    auto cr = make_sc_crystal(3.0);
    int32_t miller[3] = {1, 0, 0};
    const int upper_bound = get_slab_size_v2(
        cr.lattice.data(), miller, 3, cr.n_atoms());
    ASSERT_GT(upper_bound, 1);

    ColMajorMatrix3d out_lattice = ColMajorMatrix3d::Constant(17.0);
    std::vector<double> out_positions(static_cast<size_t>(upper_bound) * 3, 23.0);
    std::vector<int> out_types(static_cast<size_t>(upper_bound), 29);

    const int n_atoms = build_slab_v2(
        cr.lattice.data(), cr.positions.data(), cr.types.data(), cr.n_atoms(),
        miller, 3, 10.0, static_cast<size_t>(upper_bound - 1),
        out_lattice.data(), out_positions.data(), out_types.data());

    EXPECT_EQ(n_atoms, 0);
    EXPECT_EQ(out_lattice, ColMajorMatrix3d::Constant(17.0));
    EXPECT_TRUE(std::all_of(out_positions.begin(), out_positions.end(),
                            [](double value) { return value == 23.0; }));
    EXPECT_TRUE(std::all_of(out_types.begin(), out_types.end(),
                            [](int value) { return value == 29; }));
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

TEST(SlabBuilderTest, CoincidentSitesAndOpaqueTagsArePreserved) {
    auto input = make_sc_crystal(3.0);
    input.positions = {0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1e-6, 0.0, 0.0};
    input.types = {19, 23, 29};
    const auto result = run_slab_v2(input, 1, 1, 1, 3, 10.0);
    ASSERT_EQ(result.n_atoms, 9);
    for (int tag : input.types) EXPECT_EQ(std::count(result.types.begin(), result.types.end(), tag), 3);
}

TEST(SlabBuilderTest, ObliqueCellMetricAndNormalRepeatHeight) {
    auto input = make_sc_crystal(3.0);
    input.lattice << 3.0, 0.7, -0.4, 0.0, 4.0, 0.6, 0.0, 0.0, 5.0;
    input.positions = {0.13, 0.27, 0.39, 0.71, 0.61, 0.53};
    input.types = {101, 203};
    const std::vector<Eigen::Vector3i> planes = {
        {1, 0, 0}, {0, 0, -1}, {1, 1, 1}, {2, -1, 3}, {4, 2, 0}};
    for (const auto& plane : planes) {
        SCOPED_TRACE(plane.transpose());
        const int divisor = std::gcd(std::gcd(std::abs(plane.x()), std::abs(plane.y())), std::abs(plane.z()));
        const Eigen::Vector3d reduced = plane.cast<double>() / divisor;
        const double period = 1.0 / (input.lattice.inverse().transpose() * reduced).norm();
        const auto bare = run_slab_v2(input, plane.x(), plane.y(), plane.z(), 3, 0.0);
        const auto padded = run_slab_v2(input, plane.x(), plane.y(), plane.z(), 3, 11.0);
        ASSERT_EQ(bare.n_atoms, 6);
        ASSERT_EQ(padded.n_atoms, 6);
        EXPECT_NEAR(bare.lattice.col(2).norm(), 3.0 * period, 1e-10);
        EXPECT_NEAR(padded.lattice.col(2).norm(), 3.0 * period + 11.0, 1e-10);
        EXPECT_NEAR(bare.lattice.determinant(), 3.0 * input.lattice.determinant(), 1e-9);
        EXPECT_NEAR(padded.lattice.col(0).dot(padded.lattice.col(2)), 0.0, 1e-10);
        EXPECT_NEAR(padded.lattice.col(1).dot(padded.lattice.col(2)), 0.0, 1e-10);
        Eigen::Matrix<int, 3, 3, Eigen::ColMajor> basis;
        ASSERT_TRUE(get_surface_basis(input.lattice, plane.x(), plane.y(), plane.z(), basis));
        ColMajorMatrix3d reference_cell = input.lattice * basis.cast<double>();
        const Eigen::Vector3d normal = (input.lattice.inverse().transpose() * reduced).normalized();
        reference_cell.col(2) = normal * (3.0 * period + 11.0);
        const ColMajorMatrix3d rotation = padded.lattice * reference_cell.inverse();
        EXPECT_TRUE((rotation.transpose() * rotation).isApprox(ColMajorMatrix3d::Identity(), 1e-10));
        for (int i = 0; i < bare.n_atoms; ++i) {
            const Eigen::Map<const Eigen::Vector3d> f_bare(&bare.positions[3 * i]);
            const Eigen::Map<const Eigen::Vector3d> f_padded(&padded.positions[3 * i]);
            const Eigen::Vector3d delta = padded.lattice * f_padded - bare.lattice * f_bare;
            EXPECT_NEAR(delta.x(), 0.0, 1e-10);
            EXPECT_NEAR(delta.y(), 0.0, 1e-10);
            EXPECT_NEAR(delta.z(), 5.5, 1e-10);
            EXPECT_EQ(bare.types[i], padded.types[i]);
            const int source_index = padded.types[i] == 101 ? 0 : 1;
            const Eigen::Map<const Eigen::Vector3d> source(&input.positions[3 * source_index]);
            const Eigen::Vector3d input_cartesian = rotation.transpose()
                * (padded.lattice * f_padded) - 5.5 * normal;
            const Eigen::Vector3d image = input.lattice.inverse() * input_cartesian - source;
            EXPECT_TRUE((image - image.array().round().matrix()).isZero(1e-10));
        }
    }
}

TEST(SlabBuilderTest, SymmetricModeAcceptsMirrorWithoutInversion) {
    const auto lattice = make_sc(6.0);
    const double positions[] = {0.13, 0.21, 0.2, 0.13, 0.21, 0.8,
                                0.37, 0.42, 0.3, 0.37, 0.42, 0.7,
                                0.61, 0.17, 0.4, 0.61, 0.17, 0.6};
    const int classes[] = {1, 1, 2, 2, 3, 3};
    const int32_t miller[] = {0, 0, 1};
    ColMajorMatrix3d cell;
    std::vector<double> output(18);
    std::vector<int> sources(6);
    ASSERT_EQ(build_symmetric_slab(lattice.data(), positions, classes, 6, miller, 1,
                                  12.0, 6, cell.data(), output.data(), sources.data()), 6);
    EXPECT_NEAR(cell(2, 2), 18.0, 1e-10);
    for (int i = 0; i < 6; ++i) {
        bool found = false;
        for (int j = 0; j < 6; ++j) {
            if (classes[sources[i]] != classes[sources[j]]) continue;
            if (std::abs(output[3*i] - output[3*j]) < 1e-10 &&
                std::abs(output[3*i+1] - output[3*j+1]) < 1e-10 &&
                std::abs(output[3*i+2] + output[3*j+2] - 1.0) < 1e-10) found = true;
        }
        EXPECT_TRUE(found);
    }
}

TEST(SlabBuilderTest, SymmetricModeDoesNotAcceptBulkPeriodicityAsFiniteSymmetry) {
    const auto lattice = make_sc(4.0);
    const double positions[] = {0.0, 0.0, 0.0, 0.0, 0.0, 0.5};
    const int classes[] = {1, 2};
    const int32_t miller[] = {0, 0, 1};
    ColMajorMatrix3d cell = ColMajorMatrix3d::Constant(17.0);
    std::vector<double> output(12, 23.0);
    std::vector<int> sources(4, 29);
    EXPECT_EQ(build_symmetric_slab(lattice.data(), positions, classes, 2, miller, 2,
                                  18.0, 4, cell.data(), output.data(), sources.data()), -2);
    EXPECT_EQ(cell, ColMajorMatrix3d::Constant(17.0));
    EXPECT_EQ(output, std::vector<double>(12, 23.0));
    EXPECT_EQ(sources, std::vector<int>(4, 29));
    EXPECT_EQ(build_symmetric_slab(lattice.data(), positions, classes, 2, miller, 2,
                                  18.0, 3, cell.data(), output.data(), sources.data()), 0);
    EXPECT_EQ(output, std::vector<double>(12, 23.0));
}
