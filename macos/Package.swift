// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "MyDevice",
    platforms: [.macOS("26.0")],
    targets: [
        // The Rust core's C interface; build.sh generates the header and builds .rust/libmydevice_core.a.
        .systemLibrary(name: "MyDeviceCoreFFI"),
        .executableTarget(
            name: "MyDevice",
            dependencies: ["MyDeviceCoreFFI"],
            linkerSettings: [.unsafeFlags(["-L\(Context.packageDirectory)/.rust"])]
        ),
    ]
)
