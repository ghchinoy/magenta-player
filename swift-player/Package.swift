// swift-tools-version: 5.7
import PackageDescription
import Foundation

// ---------------------------------------------------------------------------
// Locate the shared MRT2 build artifacts.
//
// Resolution order (first found wins):
//  1. MRT2_BUILD_DIR environment variable — set by the root Makefile via
//     `export MRT2_BUILD_DIR := $(abspath mrt2-build)`.
//  2. ../mrt2-build relative to this Package.swift.
// ---------------------------------------------------------------------------

let packageDir         = URL(fileURLWithPath: #file).deletingLastPathComponent()
let bridgeHeaderPath   = packageDir.appendingPathComponent("bridge/include/magentart_bridge.h").path

let mrt2BuildDir: String = {
    if let env = ProcessInfo.processInfo.environment["MRT2_BUILD_DIR"], !env.isEmpty {
        return env
    }
    return packageDir.appendingPathComponent("../mrt2-build").standardized.path
}()

let mrt2IncludeDir = "\(mrt2BuildDir)/include"
let mrt2LibDir     = mrt2BuildDir

let package = Package(
    name: "MagentaSwiftPlayer",
    // The C++ engine was compiled for macOS 14.0; match the deployment target.
    // .v14 requires PackageDescription 5.9; use the string initialiser for 5.7 compat.
    platforms: [.macOS("14.0")],
    products: [
        .executable(name: "magenta-player", targets: ["magenta-player"])
    ],
    targets: [

        // ------------------------------------------------------------------ //
        // MagentaBridge: Objective-C++ wrapper exposing the C++ engine       //
        // to Swift via a flat C API (magentart_bridge.h).                    //
        // ------------------------------------------------------------------ //
        .target(
            name: "MagentaBridge",
            path: "bridge",
            sources: ["magentart_bridge.mm"],
            publicHeadersPath: "include",
            cxxSettings: [
                .unsafeFlags(["-I\(mrt2IncludeDir)"]),
            ],
            linkerSettings: [
                // libmagentart_all.a bundles magentart_core + all transitive
                // deps (TFLite, MLX, SentencePiece, abseil, ruy, …) merged
                // by `libtool -static` into one fat archive.
                .linkedLibrary("magentart_all"),
                .unsafeFlags(["-L\(mrt2LibDir)"]),

                // Frameworks required by the engine (mirrors cmake link.txt)
                .linkedFramework("Foundation"),
                .linkedFramework("Metal"),
                .linkedFramework("MetalPerformanceShaders"),
                .linkedFramework("QuartzCore"),
                .linkedFramework("Accelerate"),
                .linkedFramework("AudioToolbox"),
                .linkedFramework("AVFoundation"),
                .linkedFramework("CoreAudio"),
                .linkedFramework("CoreMIDI"),
                .linkedFramework("AppKit"),
            ]
        ),

        // ------------------------------------------------------------------ //
        // magenta-player: the SwiftUI application.                           //
        // ------------------------------------------------------------------ //
        .executableTarget(
            name: "magenta-player",
            dependencies: ["MagentaBridge"],
            path: "src",
            swiftSettings: [
                .unsafeFlags([
                    "-import-objc-header", bridgeHeaderPath,
                ]),
            ]
        ),

        // ------------------------------------------------------------------ //
        // Tests                                                               //
        // ------------------------------------------------------------------ //
        .testTarget(
            name: "MagentaPlayerTests",
            dependencies: ["magenta-player"],
            path: "Tests"
        )
    ],
    cxxLanguageStandard: .cxx17
)
