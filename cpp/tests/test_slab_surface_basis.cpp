#include <gtest/gtest.h>
#include <Eigen/Dense>
#include "physics_kernel_internal.hpp"
#include <vector>
#include <cmath>
#include <numeric>

// [Breaker Mode] S1 Gate Tests — High intensity stress on get_surface_basis

namespace {

Eigen::Matrix3d make_cubic_lattice(double a) {
    Eigen::Matrix3d L;
    L << a, 0, 0,
         0, a, 0,
         0, 0, a;
    return L;
}

Eigen::Matrix3d make_fcc_lattice(double a) {
    Eigen::Matrix3d L;
    L << 0, a/2, a/2,
         a/2, 0, a/2,
         a/2, a/2, 0;
    return L;
}

Eigen::Matrix3d make_bcc_lattice(double a) {
    Eigen::Matrix3d L;
    L << -a/2,  a/2,  a/2,
          a/2, -a/2,  a/2,
          a/2,  a/2, -a/2;
    return L;
}

Eigen::Matrix3d make_hcp_lattice(double a, double c) {
    Eigen::Matrix3d L;
    L << a, -a/2, 0,
         0,  a*std::sqrt(3.0)/2.0, 0,
         0,  0, c;
    return L;
}

Eigen::Matrix3d make_pathological_lattice() {
    // Extremely oblique and small volume
    Eigen::Matrix3d L;
    L << 1.0, 0.999, 0,
         0, 0.001, 0,
         0, 0, 1.0;
    return L;
}

void verify_basis_invariants(const Eigen::Matrix3d& lattice, const Eigen::Matrix3i& P, int h, int k, int l) {
    // 1. Determinant must be 1
    EXPECT_EQ(P.determinant(), 1) << "Determinant must be exactly 1 for right-handed basis change.";

    // 2. Orthogonality: G . v1 = 0 and G . v2 = 0
    Eigen::Vector3i g(h, k, l);
    // Normalize g by gcd for proper checking if it was an input
    // But get_surface_basis already does this internally, as should we if we want strict dot check
    EXPECT_EQ(g.dot(P.col(0)), 0) << "v1 must be in-plane (orthogonal to Miller indices).";
    EXPECT_EQ(g.dot(P.col(1)), 0) << "v2 must be in-plane (orthogonal to Miller indices).";

    // 3. Bezout Identity Verification: h*p + k*q + l*r = 1 (if h,k,l are primitive)
    int g_val = std::abs(std::gcd(std::gcd(h, k), l));
    if (g_val == 1) {
        EXPECT_EQ(g.dot(P.col(2)), 1) << "v3 must satisfy the Bezout identity h*p + k*q + l*r = 1 for primitive indices.";
    } else if (g_val > 0) {
        // If h,k,l were not primitive (e.g. 2,0,0), our internal normalization handles it
        // but the resulting p,q,r should satisfy the normalized relation.
        // However, get_surface_basis is defined to handle normalized Miller indices.
        EXPECT_EQ(g.dot(P.col(2)) / g_val, 1) << "v3 must satisfy normalized Bezout identity.";
    }
    
    // 4. Basis completeness is checked by det=1
}

} // namespace

// ===========================================================================
// Happy Path & Basic Invariants
// ===========================================================================

TEST(SlabSurfaceBasisTest, Cubic100) {
    auto L = make_cubic_lattice(5.0);
    auto P = get_surface_basis(L, 1, 0, 0);
    verify_basis_invariants(L, P, 1, 0, 0);
}

TEST(SlabSurfaceBasisTest, Cubic111) {
    auto L = make_cubic_lattice(5.0);
    auto P = get_surface_basis(L, 1, 1, 1);
    verify_basis_invariants(L, P, 1, 1, 1);
}

TEST(SlabSurfaceBasisTest, FCC111) {
    auto L = make_fcc_lattice(4.05);
    auto P = get_surface_basis(L, 1, 1, 1);
    verify_basis_invariants(L, P, 1, 1, 1);
    
    // Shortest vector for FCC (111) plane is a/sqrt(2)
    Eigen::Vector3d v1_cart = L * P.col(0).cast<double>();
    EXPECT_NEAR(v1_cart.norm(), 4.05 / std::sqrt(2.0), 1e-7);
}

// ===========================================================================
// Edge Cases: (0,0,l) and Normalized Miller Indices
// ===========================================================================

TEST(SlabSurfaceBasisTest, Axis001) {
    auto L = make_cubic_lattice(5.0);
    auto P = get_surface_basis(L, 0, 0, 1);
    verify_basis_invariants(L, P, 0, 0, 1);
}

TEST(SlabSurfaceBasisTest, NormalizedIndices) {
    auto L = make_cubic_lattice(5.0);
    auto P_100 = get_surface_basis(L, 1, 0, 0);
    auto P_200 = get_surface_basis(L, 2, 0, 0);
    EXPECT_EQ(P_100, P_200) << "(1,0,0) and (2,0,0) should produce the same basis.";
}

// ===========================================================================
// [Breaker] Attack: Pathological Indices & Lattices
// ===========================================================================

TEST(SlabSurfaceBasisTest, ZeroIndexAttack) {
    auto L = make_cubic_lattice(5.0);
    auto P = get_surface_basis(L, 0, 0, 0);
    // Current impl returns Identity for h=0, k=0 (which handles 0,0,0)
    EXPECT_EQ(P, Eigen::Matrix3i::Identity());
}

TEST(SlabSurfaceBasisTest, LargeMillerIndex) {
    auto L = make_cubic_lattice(5.0);
    auto P = get_surface_basis(L, 7, 5, 3);
    verify_basis_invariants(L, P, 7, 5, 3);
}

TEST(SlabSurfaceBasisTest, SingularLattice) {
    Eigen::Matrix3d L = Eigen::Matrix3d::Zero();
    auto P = get_surface_basis(L, 1, 1, 1);
    EXPECT_EQ(P.determinant(), 1);
}

TEST(SlabSurfaceBasisTest, DegenerateLatticePlanar) {
    Eigen::Matrix3d L;
    L << 1, 0, 0,
         0, 1, 0,
         0, 0, 0;
    auto P = get_surface_basis(L, 1, 1, 1);
    EXPECT_EQ(P.determinant(), 1);
}

TEST(SlabSurfaceBasisTest, ObliqueLatticeStress) {
    auto L = make_pathological_lattice();
    auto P = get_surface_basis(L, 1, 1, 0);
    verify_basis_invariants(L, P, 1, 1, 0);
    
    Eigen::Vector3d c1 = L * P.col(0).cast<double>();
    Eigen::Vector3d c2 = L * P.col(1).cast<double>();
    Eigen::Vector3d c3 = L * P.col(2).cast<double>();
    
    EXPECT_LE(std::abs(c3.dot(c1)), 0.51 * c1.squaredNorm());
    EXPECT_LE(std::abs(c3.dot(c2)), 0.51 * c2.squaredNorm());
}

TEST(SlabSurfaceBasisTest, BCC110) {
    auto L = make_bcc_lattice(2.87);
    auto P = get_surface_basis(L, 1, 1, 0);
    verify_basis_invariants(L, P, 1, 1, 0);
}

TEST(SlabSurfaceBasisTest, HCP001) {
    auto L = make_hcp_lattice(3.21, 5.21);
    auto P = get_surface_basis(L, 0, 0, 1);
    verify_basis_invariants(L, P, 0, 0, 1);
}

TEST(SlabSurfaceBasisTest, BezoutIdentityStress) {
    auto L = make_cubic_lattice(5.0);
    // Large coprime triplet
    auto P = get_surface_basis(L, 11, 13, 17);
    verify_basis_invariants(L, P, 11, 13, 17);
}

TEST(SlabSurfaceBasisTest, LargeRandomMiller) {
    auto L = make_fcc_lattice(4.0);
    auto P = get_surface_basis(L, 3, -2, 4);
    verify_basis_invariants(L, P, 3, -2, 4);
}
