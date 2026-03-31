import XCTest
import SwiftTreeSitter
import TreeSitterBsv

final class TreeSitterBsvTests: XCTestCase {
    func testCanLoadGrammar() throws {
        let parser = Parser()
        let language = Language(language: tree_sitter_bsv())
        XCTAssertNoThrow(try parser.setLanguage(language),
                         "Error loading Bsv grammar")
    }
}
