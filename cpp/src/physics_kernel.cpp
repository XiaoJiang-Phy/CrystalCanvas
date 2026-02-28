// Spglib C Wrapper Implementation
#include "physics_kernel.hpp"
#include <iostream>
#include <stdexcept>
#include <vector>

// Include spglib
extern "C" {
#include "spglib.h"
}

int get_spacegroup(const double *lattice, const double *positions,
                   const int *types, size_t n_atoms, double symprec) {
  try {
    // Spglib modifies the dataset and expects positions to be mutable if it
    // needs to standardize. For spg_get_dataset, we just need to pass the raw
    // arrays. However, spgat_get_dataset requires double lattice[3][3], double
    // position[][3], int types[].

    // Convert flat 1D lattice (assuming row-major 3x3) to 2D array for spglib
    double spg_lattice[3][3] = {{lattice[0], lattice[1], lattice[2]},
                                {lattice[3], lattice[4], lattice[5]},
                                {lattice[6], lattice[7], lattice[8]}};

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
    spg_free_dataset(dataset);

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
