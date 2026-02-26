//! Build script — compiles C++ thin wrapper with cxx_build for Rust/C++ FFI bridge

fn main() {
    // cxx bridge code generation + C++ compilation
    cxx_build::bridge("src/ffi/bridge.rs")
        .file("../cpp/src/crystal_parser.cpp")
        .file("../cpp/third_party/gemmi/src/symmetry.cpp") // space group tables + triplet parser
        .include("../cpp/src")                           // crystal_parser.hpp
        .include("../cpp/third_party/gemmi/include")     // gemmi headers
        .include("../cpp/third_party/gemmi/third_party") // PEGTL (gemmi dependency)
        .std("c++17")
        .warnings(false) // suppress warnings from gemmi/PEGTL headers
        .compile("crystal_cpp");

    // Tauri build
    tauri_build::build();
}
