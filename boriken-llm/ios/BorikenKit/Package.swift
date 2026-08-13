// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "BorikenKit",
    platforms: [
        .iOS(.v16),
        .macOS(.v13)
    ],
    products: [
        .library(name: "BorikenKit", targets: ["BorikenKit"])
    ],
    targets: [
        .target(
            name: "BorikenKit",
            path: "Sources/BorikenKit"
        )
    ]
)
