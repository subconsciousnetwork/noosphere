// swift-tools-version:5.5
// The swift-tools-version declares the minimum version of Swift required to build this package.
import PackageDescription

let package = Package(
    name: "SwiftNoosphere",
    platforms: [
        .iOS(.v13),
        .macOS(.v11)
    ],
    products: [
        // Products define the executables and libraries a package produces, and make them visible to other packages.
        .library(
            name: "SwiftNoosphere",
            targets: ["SwiftNoosphere"]),
    ],
    dependencies: [
        // Dependencies declare other packages that this package depends on.
        // .package(url: /* package url */, from: "1.0.0"),
    ],
    targets: [
        // Targets are the basic building blocks of a package. A target can define a module or a test suite.
        // Targets can depend on other targets in this package, and on products in packages this package depends on.
        .target(
            name: "SwiftNoosphere",
            dependencies: ["LibNoosphere"],
            path: "swift/Sources/SwiftNoosphere"),
        .binaryTarget(
            name: "LibNoosphere",
            url: "https://github.com/subconsciousnetwork/noosphere/releases/download/noosphere-v0.10.8/libnoosphere-apple-xcframework.zip",
            checksum: "5f2d1902906e50f62f3ac120fb919064c3b312ac306d5e6a556bc16a3d1bfc50"),
        .testTarget(
            name: "SwiftNoosphereTests",
            dependencies: ["SwiftNoosphere"],
            path: "swift/Tests/SwiftNoosphereTests"),
    ]
)
