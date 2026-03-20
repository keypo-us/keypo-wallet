// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "keypo-signer",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(name: "keypo-signer", targets: ["keypo-signer"]),
        .library(name: "KeypoCore", targets: ["KeypoCore"]),
    ],
    dependencies: [
        .package(url: "https://github.com/apple/swift-argument-parser.git", from: "1.3.0"),
        .package(url: "https://github.com/jedisct1/swift-sodium.git", from: "0.9.1"),
    ],
    targets: [
        .executableTarget(
            name: "keypo-signer",
            dependencies: [
                "KeypoCore",
                .product(name: "ArgumentParser", package: "swift-argument-parser"),
            ]
        ),
        .target(
            name: "KeypoCore",
            dependencies: [
                .product(name: "Sodium", package: "swift-sodium"),
            ]
        ),
        .testTarget(
            name: "KeypoCoreTests",
            dependencies: ["KeypoCore"]
        ),
    ]
)
