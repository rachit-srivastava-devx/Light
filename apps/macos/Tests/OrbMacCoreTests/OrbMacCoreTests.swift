import XCTest
@testable import OrbMacCore

final class OrbMacCoreTests: XCTestCase {
    /// Trivial smoke test: proves the OrbMacCore library target builds,
    /// links, and is importable from a test target. Later units add real
    /// coverage for the protocol client / audio / state machine here.
    func testModuleNameIsSet() {
        XCTAssertEqual(OrbMacCore.moduleName, "OrbMacCore")
    }
}
