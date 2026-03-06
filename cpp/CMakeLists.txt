cmake_minimum_required(VERSION 3.25)
project(crystal_kernel CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Ensure ColMajor is the default for Eigen to match Rust/Physics conventions
add_compile_definitions(EIGEN_DEFAULT_TO_ROW_MAJOR=0)

# Add Spglib as a subdirectory (statically linked)
set(SPGLIB_INSTALL ON CACHE BOOL "" FORCE)
add_subdirectory(third_party/spglib)

# Add Eigen (header-only)
add_subdirectory(third_party/eigen EXCLUDE_FROM_ALL)

# Core physics library
add_library(crystal_kernel STATIC src/physics_kernel.cpp)

target_include_directories(crystal_kernel
    PUBLIC include
    PRIVATE third_party/gemmi/include
    PRIVATE third_party/eigen
    PRIVATE third_party/spglib/include
)

target_link_libraries(crystal_kernel PUBLIC Spglib_symspg)

# The CXX bridge will link against crystal_kernel

# Ensure libraries are installed so cmake-rs can find them in the prefix
install(TARGETS crystal_kernel
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
    RUNTIME DESTINATION bin
)
