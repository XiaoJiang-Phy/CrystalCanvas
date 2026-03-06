//! Build script — compiles C++ thin wrapper with cxx_build for Rust/C++ FFI bridge

fn main() {
    // Force macOS deployment target to 10.12 to match Rust's target
    #[cfg(target_os = "macos")]
    {
        std::env::set_var("MACOSX_DEPLOYMENT_TARGET", "10.12");
    }

    println!("cargo:rerun-if-changed=../cpp/src/physics_kernel.cpp");
    println!("cargo:rerun-if-changed=../cpp/include/physics_kernel.hpp");
    println!("cargo:rerun-if-changed=../cpp/src/crystal_parser.cpp");
    println!("cargo:rerun-if-changed=../cpp/include/crystal_parser.hpp");

    cxx_build::bridge("src/ffi/bridge.rs")
        .file("../cpp/src/crystal_parser.cpp")
        .file("../cpp/third_party/gemmi/src/symmetry.cpp")
        .include("../cpp/src") 
        .include("../cpp/include")
        .include("../cpp/third_party/gemmi/include")
        .include("../cpp/third_party/gemmi/third_party")
        .include("../cpp/third_party/spglib/include")
        .include("../cpp/third_party/eigen")
        .std("c++17")
        .warnings(false)
        .compile("crystal_cpp");

    // Use cmake to build the cpp/ directory
    let dst = cmake::Config::new("../cpp")
        .define("CMAKE_CXX_STANDARD", "17")
        .build();

    // Determine the build configuration (Release/Debug) for path searching on Windows
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "release".to_string());
    let config = if profile == "debug" { "Debug" } else { "Release" };

    // Search common CMake output locations for libraries (MSVC configuration-aware)
    let search_paths = [
        dst.join("lib"),
        dst.join("lib").join(config),
        dst.join("build"),
        dst.join("build").join(config),
        dst.join("build").join("third_party").join("spglib"),
        dst.join("build").join("third_party").join("spglib").join(config),
    ];

    for path in search_paths {
        if path.exists() {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
    }

    // Link physics kernel and symmetry library
    // symspg is produced by Spglib, crystal_kernel by our CMake
    println!("cargo:rustc-link-lib=static=symspg");
    println!("cargo:rustc-link-lib=static=crystal_kernel");

    // Tauri build
    tauri_build::build();
}
