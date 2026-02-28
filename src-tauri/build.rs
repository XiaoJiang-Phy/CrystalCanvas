//! Build script — compiles C++ thin wrapper with cxx_build for Rust/C++ FFI bridge

    cxx_build::bridge("src/ffi/bridge.rs")
        .file("../cpp/src/crystal_parser.cpp")
        .file("../cpp/src/physics_kernel.cpp")
        .file("../cpp/third_party/gemmi/src/symmetry.cpp") // space group tables + triplet parser
        .include("../cpp/src")                           // crystal_parser.hpp
        .include("../cpp/include")                       // physics_kernel.hpp
        .include("../cpp/third_party/gemmi/include")     // gemmi headers
        .include("../cpp/third_party/gemmi/third_party") // PEGTL (gemmi dependency)
        .include("../cpp/third_party/spglib/include")    // spglib headers
        .std("c++17")
        .warnings(false) // suppress warnings from gemmi/PEGTL headers
        .compile("crystal_cpp");

    // Use cmake to build the cpp/ directory
    let dst = cmake::Config::new("../cpp")
        .define("CMAKE_CXX_STANDARD", "17")
        .build();

    // Tell cargo where to find the compiled spglib (symspg) and crystal_kernel
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    // In Spglib cmake, it produces symspg.a
    println!("cargo:rustc-link-lib=static=symspg");
    
    // Tauri build
    tauri_build::build();
}
