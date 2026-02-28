// [Node 3.3] Slab generation (Eigen) tests
//
// Acceptance Criteria:
// - Cleave a 3-layer (1 0 0) slab from an FCC cell.
// - Verify atom counts and coordinates.
// - Verify vacuum padding is added to the c-axis length.
//

#include <gtest/gtest.h>
#include <cmath>
#include <vector>
#include <array>

#include "physics_kernel.hpp"

namespace {

struct SlabInput {
    double lattice[3][3];
    std::vector<std::array<double, 3>> positions;  // Fractional coordinates
    std::vector<int> types;
    int n_atoms;
};

struct SlabResultTest {
    double lattice[3][3];
    std::vector<std::array<double, 3>> positions;  // Fractional coordinates in NEW cell
    std::vector<int> types;
    int n_atoms;
};

/// Create simple cubic cell 
SlabInput make_sc_cell() {
    SlabInput cell;
    double a = 4.0;  // Å

    cell.lattice[0][0] = a;  cell.lattice[0][1] = 0;  cell.lattice[0][2] = 0;
    cell.lattice[1][0] = 0;  cell.lattice[1][1] = a;  cell.lattice[1][2] = 0;
    cell.lattice[2][0] = 0;  cell.lattice[2][1] = 0;  cell.lattice[2][2] = a;

    cell.positions = {
        {0.0, 0.0, 0.0},
    };

    cell.types = {1};
    cell.n_atoms = 1;
    return cell;
}

// Wrapper to call build_slab with simplified interface
SlabResultTest run_slab(const SlabInput& input, const int32_t miller[3], int layers, double vacuum_A) {
    int n_new = get_slab_size(
        &input.lattice[0][0], 
        miller, 
        layers, 
        vacuum_A,
        input.n_atoms
    );
    
    SlabResultTest res;
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
    
    build_slab(
        &input.lattice[0][0],
        flat_pos.data(),
        input.types.data(),
        input.n_atoms,
        miller,
        layers,
        vacuum_A,
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
// Node 3.3 Slab Tests
// ===========================================================================

/// Simple Cubic (100) surface, 3 layers, 10A vacuum
TEST(SlabTest, SC_100_3Layers) {
    auto input = make_sc_cell();
    int32_t miller[3] = {1, 0, 0};
    int layers = 3;
    double vacuum_A = 10.0;
    
    auto result = run_slab(input, miller, layers, vacuum_A);
    
    // Original a=4.0
    // (100) slab means the c-axis of the slab should correspond to the original a-axis direction (or orthogonal to surface).
    // The new c-vector length before vacuum is layers * d_spacing = 3 * 4.0 = 12.0
    // After vacuum: 12.0 + 10.0 = 22.0
    
    EXPECT_EQ(result.n_atoms, 3) << "(100) surface of SC with 3 layers should have 3 atoms";
    
    // Check vacuum padding on c-axis
    // The c-vector is result.lattice[2]
    double c_len = std::sqrt(
        result.lattice[2][0]*result.lattice[2][0] + 
        result.lattice[2][1]*result.lattice[2][1] + 
        result.lattice[2][2]*result.lattice[2][2]
    );
    EXPECT_NEAR(c_len, 22.0, 1e-5);
    
    // Check atoms are within boundaries
    for (int i = 0; i < result.n_atoms; ++i) {
        for (int dim=0; dim<3; ++dim) {
            EXPECT_GE(result.positions[i][dim], 0.0);
            EXPECT_LT(result.positions[i][dim], 1.0);
        }
    }
}
