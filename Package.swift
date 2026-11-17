// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Drift",
    platforms: [.macOS(.v14)],
    targets: [
        .executableTarget(
            name: "Drift",
            path: "Sources/Drift",
            // Same reasoning as the canvas app next door: the whole interface is
            // main-thread by nature, and Swift 6's strict isolation buys nothing
            // here but ceremony.
            swiftSettings: [.swiftLanguageMode(.v5)]
        ),
        .testTarget(
            name: "DriftTests",
            dependencies: ["Drift"],
            path: "Tests/DriftTests",
            swiftSettings: [.swiftLanguageMode(.v5)]
        )
    ]
)
