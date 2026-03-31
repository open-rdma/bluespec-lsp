// swift-tools-version:5.3
import PackageDescription

let package = Package(
    name: "TreeSitterBsv",
    products: [
        .library(name: "TreeSitterBsv", targets: ["TreeSitterBsv"]),
    ],
    dependencies: [
        .package(url: "https://github.com/ChimeHQ/SwiftTreeSitter", from: "0.8.0"),
    ],
    targets: [
        .target(
            name: "TreeSitterBsv",
            dependencies: [],
            path: ".",
            sources: [
                "src/parser.c",
                // NOTE: if your language has an external scanner, add it here.
            ],
            resources: [
                .copy("queries")
            ],
            publicHeadersPath: "bindings/swift",
            cSettings: [.headerSearchPath("src")]
        ),
        .testTarget(
            name: "TreeSitterBsvTests",
            dependencies: [
                "SwiftTreeSitter",
                "TreeSitterBsv",
            ],
            path: "bindings/swift/TreeSitterBsvTests"
        )
    ],
    cLanguageStandard: .c11
)
