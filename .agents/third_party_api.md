# Third-Party API Reference

> **Purpose**: LLMs consult this document for third-party library usage patterns instead of re-scanning library source code.
> **Update rule**: Must be updated whenever a new third-party API is introduced.

---

## 1. Gemmi — Crystallography File Parsing Library

- **Repo path**: `cpp/third_party/gemmi/`
- **Header root**: `cpp/third_party/gemmi/include/gemmi/`
- **PEGTL dependency**: `cpp/third_party/gemmi/third_party/` (bundled, no extra install)
- **C++ standard**: C++17
- **Compile definition**: `GEMMI_BUILD` is NOT needed for header-only usage

### 1.1 CIF File Parsing (header-only, no zlib required)

```cpp
#include <gemmi/cif.hpp>     // CIF parser (PEGTL-based, header-only)
#include <gemmi/smcif.hpp>   // SmallStructure <-> CIF conversion

// Read directly from file (uncompressed, header-only, no zlib dependency)
gemmi::cif::Document doc = gemmi::cif::read_file("path/to/file.cif");

// Read from memory buffer
gemmi::cif::Document doc = gemmi::cif::read_memory(data, size, "name");

// Get first block and convert to SmallStructure
const gemmi::cif::Block& block = doc.blocks.at(0);
gemmi::SmallStructure st = gemmi::make_small_structure_from_block(block);
```

### 1.2 SmallStructure Key Fields

```cpp
// From <gemmi/small.hpp>
struct SmallStructure {
    std::string name;
    UnitCell cell;                        // Unit cell parameters (see 1.3)
    const SpaceGroup* spacegroup;         // Space group pointer (may be nullptr)
    std::string spacegroup_hm;            // Hermann-Mauguin symbol
    int spacegroup_number;                // IT number
    std::vector<Site> sites;              // Atom site list

    struct Site {
        std::string label;                // Atom label (e.g. "Na1")
        std::string type_symbol;          // Element symbol (e.g. "Na")
        Fractional fract;                 // Fractional coordinates {x, y, z}
        double occ;                       // Occupancy
        Element element;                  // Element enum (El::Na, El::Cl, ...)
        signed char charge;               // Charge [-8, +8]
    };

    // Expand all symmetry-equivalent positions
    std::vector<Site> get_all_unit_cell_sites() const;
};
```

### 1.3 UnitCell Key API

```cpp
// From <gemmi/unitcell.hpp>
struct UnitCell : UnitCellParameters {
    double a, b, c, alpha, beta, gamma;   // Cell parameters (Å, °)
    double volume;                         // Volume

    // Fractional coords -> Cartesian coords
    Position orthogonalize(const Fractional& f) const;
    // Cartesian coords -> Fractional coords
    Fractional fractionalize(const Position& o) const;
};

// Position = Vec3 {double x, y, z}  Cartesian coordinates (Å)
// Fractional = Vec3 {double x, y, z}  Fractional coordinates
```

### 1.4 Element API

```cpp
// From <gemmi/elem.hpp>
struct Element {
    El elem;                         // Enum value
    const char* name() const;        // Element symbol string ("Na", "Cl")
    int atomic_number() const;       // Atomic number
    bool is_hydrogen() const;
};
```

### 1.5 Compressed File Support (requires zlib, NOT used in M1)

```cpp
// Requires compiling src/read_cif.cpp + gz.cpp, and linking zlib
#include <gemmi/read_cif.hpp>
gemmi::cif::Document doc = gemmi::read_cif_gz("file.cif.gz");
```

### 1.6 Build Notes

| Item | Details |
|---|---|
| **Include path** | `cpp/third_party/gemmi/include` |
| **PEGTL include path** | `cpp/third_party/gemmi/third_party` (cif.hpp depends on PEGTL) |
| **Minimum compiled sources** | Header-only: no `.cpp` files needed. `cif::read_file()` and `make_small_structure_from_block()` are both inline/template |
| **Required .cpp files** | Only if using `read_cif_gz()` etc.: compile `src/read_cif.cpp` + `src/gz.cpp` |

---

## 2. Spglib — Space Group Identification Library

- **Repo path**: `cpp/third_party/spglib/`
- **Language**: C (C11)
- **CMake minimum**: 3.25
- **Status**: Not integrated in M1, planned for M5

> API details will be added upon M5 integration.

---

## 3. Eigen — Linear Algebra Library

- **Repo path**: `cpp/third_party/eigen/`
- **Language**: C++ (header-only)
- **Status**: Not used in M1, planned for M5 (slab generation)

> API details will be added upon M5 integration.

---

## 4. cxx — Rust ↔ C++ FFI Generator

- **Crate**: `cxx = "1.0.194"` (in Cargo.toml)
- **Docs**: https://cxx.rs/

### 4.1 Basic Pattern

```rust
// Rust-side bridge definition
#[cxx::bridge]
mod ffi {
    // Shared POD struct (visible to both Rust and C++)
    struct MyData {
        name: String,  // auto-maps to rust::String
        value: f64,
    }

    // Functions exported from C++
    unsafe extern "C++" {
        include!("path/to/header.hpp");
        fn my_function(path: &str) -> Result<MyData>;
    }
}
```

### 4.2 Type Mapping

| Rust type | C++ type | Notes |
|---|---|---|
| `String` | `rust::String` | Owned string |
| `&str` | `rust::Str` | Borrowed string |
| `Vec<T>` | `rust::Vec<T>` | Dynamic array |
| `Result<T>` | exception → `rust::Error` | C++ exceptions auto-converted |
| `f64` / `i32` / `u8` | `double` / `int32_t` / `uint8_t` | Direct mapping |
| `bool` | `bool` | Direct mapping |

### 4.3 C++ Header Requirements

```cpp
// Must include cxx-generated header
#include "rust/cxx.h"

// Use rust:: namespace types
rust::String, rust::Str, rust::Vec<T>, rust::Box<T>
```

### 4.4 build.rs Integration

```rust
// build.rs
fn main() {
    // cxx bridge code generation
    cxx_build::bridge("src/ffi/bridge.rs")
        .file("cpp/src/my_wrapper.cpp")         // C++ source file
        .include("cpp/third_party/lib/include")  // Header search path
        .std("c++17")
        .compile("crystal_cpp");                  // Output static lib name
}
```

### 4.5 Critical: Shared Struct Ownership

> **WARNING**: Shared structs defined in `#[cxx::bridge]` are auto-generated by cxx for
> BOTH Rust and C++. Do NOT re-define them in your C++ header — this causes
> "redefinition" compile errors. Instead, `#include` the cxx-generated header:

```cpp
// In your C++ header — include the cxx-generated types
#include "my-crate/src/ffi/bridge.rs.h"  // path format: <crate>/<bridge_path>.h

// Only declare your function, NOT the structs
MyData my_function(rust::Str arg);
```

---

## 5. Gemmi Build Notes (Discovered During M1)

| Symbol | Requires | Source |
|---|---|---|
| `cif::read_file()` | Header-only | `<gemmi/cif.hpp>` |
| `make_small_structure_from_block()` | Header-only | `<gemmi/smcif.hpp>` |
| `parse_triplet()`, `find_spacegroup_by_name()`, `spacegroup_tables` | **symmetry.cpp** | `gemmi/src/symmetry.cpp` |

> Always include `gemmi/src/symmetry.cpp` in the build if using any space group functionality.
