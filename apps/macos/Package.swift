// swift-tools-version:5.10
import PackageDescription

let package = Package(
    name: "OrbMac",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "OrbMac", targets: ["OrbMac"]),
        .library(name: "OrbMacCore", targets: ["OrbMacCore"])
    ],
    targets: [
        .target(
            name: "OrbMacCore",
            dependencies: []
        ),
        .executableTarget(
            name: "OrbMac",
            dependencies: ["OrbMacCore"]
        ),
        .testTarget(
            name: "OrbMacCoreTests",
            dependencies: ["OrbMacCore"]
        )
    ]
)
