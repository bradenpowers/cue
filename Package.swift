// swift-tools-version: 5.9
import PackageDescription
let package = Package(name: "Cue", platforms: [.macOS(.v14)], products: [.executable(name: "Cue", targets: ["Cue"])], targets: [.executableTarget(name: "Cue"), .testTarget(name: "CueTests", dependencies: ["Cue"])])
