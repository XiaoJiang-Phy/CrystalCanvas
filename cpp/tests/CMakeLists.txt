# CMakeLists.txt for CrystalCanvas C++ kernel tests (GTest)
#
# Usage:
#   cd cpp/tests
#   cmake -B build -DCMAKE_BUILD_TYPE=Debug
#   cmake --build build
#   cd build && ctest --output-on-failure
#
# All test names prefixed with DISABLED_ will be skipped until
# the corresponding kernel modules are implemented.

cmake_minimum_required(VERSION 3.14)
project(CrystalCanvasCppTests LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# ---------------------------------------------------------------------------
# GoogleTest via FetchContent
# ---------------------------------------------------------------------------
include(FetchContent)
FetchContent_Declare(
    googletest
    GIT_REPOSITORY https://github.com/google/googletest.git
    GIT_TAG        v1.14.0
)
# Prevent GTest from overriding our compiler/linker options
set(gtest_force_shared_crt ON CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(googletest)

enable_testing()

# ---------------------------------------------------------------------------
# Third-party paths (relative to project root)
# ---------------------------------------------------------------------------
set(THIRD_PARTY_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../third_party")

# Eigen (header-only)
set(EIGEN_INCLUDE_DIR "${THIRD_PARTY_DIR}/eigen")

# Spglib
set(SPGLIB_INCLUDE_DIR "${THIRD_PARTY_DIR}/spglib/src")

# ---------------------------------------------------------------------------
# [Node 3.1] Spglib Robustness Tests
# ---------------------------------------------------------------------------
add_executable(test_spglib_robustness test_spglib_robustness.cpp)
target_link_libraries(test_spglib_robustness PRIVATE GTest::gtest_main)
target_include_directories(test_spglib_robustness PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../src
    ${SPGLIB_INCLUDE_DIR}
)
# TODO: target_link_libraries(test_spglib_robustness PRIVATE spglib)

add_test(NAME SpglibTest COMMAND test_spglib_robustness)

# ---------------------------------------------------------------------------
# [Node 3.2] Supercell Construction Tests
# ---------------------------------------------------------------------------
add_executable(test_supercell_eigen test_supercell_eigen.cpp)
target_link_libraries(test_supercell_eigen PRIVATE GTest::gtest_main)
target_include_directories(test_supercell_eigen PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../src
    ${EIGEN_INCLUDE_DIR}
)

add_test(NAME SupercellTest COMMAND test_supercell_eigen)
