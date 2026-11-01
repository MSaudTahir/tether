// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "Tether",
    platforms: [.macOS("26.0")],
    targets: [
        // The Rust core's C interface; build.sh generates the header and builds .rust/libtether_core.a.
        .systemLibrary(name: "TetherCoreFFI"),
        .executableTarget(
            name: "Tether",
            dependencies: ["TetherCoreFFI"],
            linkerSettings: [.unsafeFlags(["-L\(Context.packageDirectory)/.rust"])]
        ),
    ]
)
