use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};

fn discover_libs(dir: &Path, search_paths: &mut HashSet<PathBuf>, libs: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                discover_libs(&path, search_paths, libs);
            } else if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "a" {
                        if let Some(parent) = path.parent() {
                            search_paths.insert(parent.to_path_buf());
                        }
                        if let Some(file_stem) = path.file_stem() {
                            let stem = file_stem.to_string_lossy();
                            if stem.starts_with("lib") {
                                let lib_name = &stem[3..];
                                // We will link magentart_core manually first, so skip it here
                                if lib_name != "magentart_core" && !libs.contains(&lib_name.to_string()) {
                                    libs.push(lib_name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    // Determine the path to the shared mrt2-build folder
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    
    // Allow overriding via environment variable, otherwise default to relative path from crate root
    let build_dir_env = env::var("MRT2_BUILD_DIR").ok().map(PathBuf::from);
    let build_dir = build_dir_env.unwrap_or_else(|| manifest_dir.parent().unwrap().join("mrt2-build"));

    let include_dir = build_dir.join("include");
    let key_header = include_dir.join("magentart/realtime_runner.h");
    
    // Check if the build directory and headers are ready.
    // If not, we print a helpful error message and panic.
    if !build_dir.exists() {
        panic!(
            "\n\n[ERROR] Shared MRT2 build directory not found at: {:?}\n\
             Please build the MRT2 C++ library first using the root or swift-player Makefile:\n\
               make build-mrt2\n\n",
            build_dir
        );
    }

    if !key_header.exists() {
        panic!(
            "\n\n[ERROR] MRT2 headers (e.g. realtime_runner.h) not found at: {:?}\n\
             Please run the C++ build step first to compile the MRT2 core and populate headers:\n\
               make build-mrt2\n\n",
            key_header
        );
    }

    // Build the C++ bridge using cxx-build
    cxx_build::bridge("src/main.rs")
        .include(&include_dir)
        .flag_if_supported("-std=c++17")
        .compile("magenta_rust_bridge");

    // 1. Link our primary shared MRT2 library
    println!("cargo:rustc-link-search=native={}", build_dir.display());
    println!("cargo:rustc-link-lib=static=magentart_core");

    // 2. Recursively discover and link all static dependencies compiled under magenta-realtime/build
    let cpp_build_dir = build_dir.join("magenta-realtime/build");
    if cpp_build_dir.exists() {
        let mut search_paths = HashSet::new();
        let mut libs = Vec::new();
        
        discover_libs(&cpp_build_dir, &mut search_paths, &mut libs);
        
        // Add discovered search paths
        for path in search_paths {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
        
        // Link discovered libraries
        for lib in libs {
            println!("cargo:rustc-link-lib=static={}", lib);
        }
    }

    // 3. Link required macOS system frameworks
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=AudioToolbox");
    println!("cargo:rustc-link-lib=framework=AVFoundation");
    println!("cargo:rustc-link-lib=framework=CoreAudio");
    println!("cargo:rustc-link-lib=framework=CoreMIDI");
    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=QuartzCore");
    println!("cargo:rustc-link-lib=framework=Accelerate");
    println!("cargo:rustc-link-lib=framework=WebKit");
    println!("cargo:rustc-link-lib=framework=UniformTypeIdentifiers");

    // Rebuild if our source files or this build script change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/main.rs");
    println!("cargo:rerun-if-changed=src/bridge.h");
}
