// swift-tools-version:5.10
import PackageDescription

let package = Package(
    name: "OrbMac",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "OrbMac", targets: ["OrbMac"]),
        .executable(name: "OrbMacShot", targets: ["OrbMacShot"]),
        .library(name: "OrbMacCore", targets: ["OrbMacCore"]),
        .library(name: "OrbMacUI", targets: ["OrbMacUI"])
    ],
    targets: [
        .target(
            name: "OrbMacCore",
            dependencies: []
        ),
        // F37: SwiftUI chat views split into their own library target (mirrors why OrbMacCore is
        // its own target — a separate target is testable/importable without a window server, and
        // an executable target like OrbMac cannot be imported by a test target or by OrbMacShot).
        .target(
            name: "OrbMacUI",
            dependencies: ["OrbMacCore"]
        ),
        .executableTarget(
            name: "OrbMac",
            dependencies: ["OrbMacCore", "OrbMacUI"]
        ),
        // F37 §4.3 evidence harness: renders the REAL ChatPaneView (from OrbMacUI) against a real
        // relay-rs + relay-py stack and rasterises/OCRs it — see apps/macos/tools/f37-e2e.sh.
        .executableTarget(
            name: "OrbMacShot",
            dependencies: ["OrbMacCore", "OrbMacUI"]
        ),
        .testTarget(
            name: "OrbMacCoreTests",
            dependencies: ["OrbMacCore"]
        ),
        .testTarget(
            name: "OrbMacUITests",
            dependencies: ["OrbMacUI", "OrbMacCore"]
        )
    ]
)
