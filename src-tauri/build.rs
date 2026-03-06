//! Build script — compiles C++ thin wrapper with cxx_build for Rust/C++ FFI bridge

fn main() {
    // Force macOS deployment target to 10.12 to match Rust's target and prevent ld from silently dropping objects
    #[cfg(target_os = "macos")]
    unsafe {
        std::env::set_var("MACOSX_DEPLOYMENT_TARGET", "10.12");
    }

    println!("cargo:rerun-if-changed=../cpp/src/physics_kernel.cpp");
    println!("cargo:rerun-if-changed=../cpp/include/physics_kernel.hpp");
    println!("cargo:rerun-if-changed=../cpp/src/crystal_parser.cpp");
    println!("cargo:rerun-if-changed=../cpp/include/crystal_parser.hpp");

    #[cfg(target_os = "linux")]
    {
        // Placeholder for Ubuntu (Vulkan) / Linux specific linking rules
        // Consider setting C++ ABI or static linking stdc++ when migrating to Linux
    }

    cxx_build::bridge("src/ffi/bridge.rs")
        .file("../cpp/src/crystal_parser.cpp")
        .file("../cpp/third_party/gemmi/src/symmetry.cpp") // space group tables + triplet parser
        .include("../cpp/src") // crystal_parser.hpp
        .include("../cpp/include") // physics_kernel.hpp
        .include("../cpp/third_party/gemmi/include") // gemmi headers
        .include("../cpp/third_party/gemmi/third_party") // PEGTL (gemmi dependency)
        .include("../cpp/third_party/spglib/include") // spglib headers
        .include("../cpp/third_party/eigen") // Eigen3 headers
        .std("c++17")
        .warnings(false) // suppress warnings from gemmi/PEGTL headers
        .compile("crystal_cpp");

    // Use cmake to build the cpp/ directory
    let dst = cmake::Config::new("../cpp")
        .define("CMAKE_CXX_STANDARD", "17")
        .build();

    // Tell cargo where to find the compiled spglib (symspg) and crystal_kernel
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "release".to_string());
    let config = if profile == "debug" { "Debug" } else { "Release" };

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-search=native={}/lib/{}", dst.display(), config);
    println!("cargo:rustc-link-search=native={}/build", dst.display());
    println!("cargo:rustc-link-search=native={}/build/{}", dst.display(), config);
    println!(
        "cargo:rustc-link-search=native={}/build/third_party/spglib",
        dst.display()
    );
    println!(
        "cargo:rustc-link-search=native={}/build/third_party/spglib/{}",
        dst.display(),
        config
    );

    // In Spglib cmake, it produces symspg.a or symspg.lib
    println!("cargo:rustc-link-lib=static=symspg");
    // In our CMakeLists.txt, we produce crystal_kernel.a or crystal_kernel.lib
    println!("cargo:rustc-link-lib=static=crystal_kernel");

    // Tauri build
    tauri_build::build();
}
