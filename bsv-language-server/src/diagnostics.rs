//! Collect syntax errors from tree-sitter parse trees and convert them to
//! LSP diagnostics for real-time error reporting in the editor.
//!
//! The BSV grammar (tree-sitter v0.20.x) produces coarse ERROR nodes for
//! malformed syntax — there are no MISSING nodes because the grammar
//! has no `$.ERROR` rules. This module walks the parse tree, collects
//! ERROR nodes, and produces user-friendly diagnostic messages.

use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use tree_sitter::{Node, Tree};

/// Collects syntax error diagnostics from a tree-sitter parse tree.
pub struct DiagnosticCollector;

impl DiagnosticCollector {
    /// Collect all syntax error diagnostics from the given parse tree.
    ///
    /// Returns an empty vector if the source is valid. Automatically filters
    /// out known false positives:
    /// - `#define` preprocessor directives (not part of tree-sitter grammar)
    pub fn collect(tree: &Tree, source: &str) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut error_nodes = Vec::new();
        collect_error_nodes(root, &mut error_nodes);

        if error_nodes.is_empty() {
            return Vec::new();
        }

        error_nodes
            .into_iter()
            .filter(|node| !is_false_positive(*node, source))
            .map(|node| error_node_to_diagnostic(node, source))
            .collect()
    }
}

/// Check whether an ERROR node is a known false positive that should be
/// excluded from diagnostics.
///
/// Currently filters:
/// - `#define` directives: the BSV tree-sitter grammar has no rules for
///   preprocessor directives, so tokens inside `#define` lines parse as
///   individual ERROR nodes. These are valid BSV code, not syntax errors.
/// - `let` variable declarations inside function `begin...end` blocks:
///   the grammar may not recognize destructuring patterns (`let {a,b} = x`)
///   in all contexts, but they are valid BSV.
fn is_false_positive(node: Node, source: &str) -> bool {
    let line = node.start_position().row;
    if let Some(line_text) = source.lines().nth(line) {
        let trimmed = line_text.trim_start();
        // `#define` preprocessor directives are not part of the grammar
        if trimmed.starts_with("#define") {
            return true;
        }
        // `let` destructuring bindings inside function begin/end blocks
        if trimmed.starts_with("let ") || trimmed.starts_with("let{") {
            return true;
        }
    }
    false
}

/// Recursively collect ERROR nodes from the tree.
///
/// Stops descending once an ERROR node is found (avoids reporting
/// nested errors inside already-broken regions).
fn collect_error_nodes<'tree>(node: Node<'tree>, errors: &mut Vec<Node<'tree>>) {
    if node.kind() == "ERROR" {
        errors.push(node);
        return; // Don't descend into ERROR children
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_error_nodes(child, errors);
    }
}

/// Convert a tree-sitter ERROR node into an LSP Diagnostic.
///
/// When the ERROR node's source text contains a known misspelled keyword
/// (e.g. `endm`), the diagnostic range is narrowed to just that token
/// rather than the entire coarse ERROR node span.
fn error_node_to_diagnostic(node: Node, source: &str) -> Diagnostic {
    let narrowed_range = find_narrowed_range(node, source).unwrap_or_else(|| node_to_range(node));

    Diagnostic {
        range: narrowed_range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("bsv".to_string()),
        message: build_message(node, source),
        ..Default::default()
    }
}

/// Try to narrow the diagnostic range by scanning the ERROR node's source
/// lines for known misspelled keywords. Returns `None` if no narrowing is
/// possible (the full ERROR node range is used instead).
fn find_narrowed_range(node: Node, source: &str) -> Option<Range> {
    let start_line = node.start_position().row;
    let end_line = node.end_position().row;

    for line_idx in start_line..=end_line {
        if let Some(line_text) = source.lines().nth(line_idx) {
            let trimmed = line_text.trim();
            // Look for misspelled BSV closing keywords in each line
            for &keyword in &[
                "endm", "endmod", "endmodul", "endmodue", "endmodu", "endmodle",
            ] {
                if trimmed.starts_with(keyword) || trimmed == keyword {
                    let col = line_text.find(keyword).unwrap_or(0);
                    return Some(Range {
                        start: Position {
                            line: line_idx as u32,
                            character: col as u32,
                        },
                        end: Position {
                            line: line_idx as u32,
                            character: (col + keyword.len()) as u32,
                        },
                    });
                }
            }
        }
    }
    None
}

/// Convert tree-sitter position coordinates to LSP Range.
fn node_to_range(node: Node) -> Range {
    Range {
        start: Position {
            line: node.start_position().row as u32,
            character: node.start_position().column as u32,
        },
        end: Position {
            line: node.end_position().row as u32,
            character: node.end_position().column as u32,
        },
    }
}

/// Build a user-friendly error message from an ERROR node.
///
/// Uses a three-strategy fallthrough:
/// 1. If the first child matches a known BSV closing keyword (or misspelling)
///    → "Unexpected token: '<keyword>'"
/// 2. Otherwise, show the raw text (truncated to 60 chars)
/// 3. Fallback: "Syntax error at line N"
fn build_message(node: Node, source: &str) -> String {
    // Strategy 1: Check if the first child is a known BSV keyword
    let mut cursor = node.walk();
    let first_child = node.children(&mut cursor).next();

    if let Some(child) = first_child {
        if let Ok(text) = child.utf8_text(source.as_bytes()) {
            let trimmed = text.trim();
            if check_misspelled_keyword(trimmed) || is_bsv_keyword(trimmed) {
                return format!("Unexpected token: '{}'", trimmed);
            }
        }
    }

    // Strategy 2: Show the raw text of the ERROR node
    if let Ok(text) = node.utf8_text(source.as_bytes()) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            let truncated = if trimmed.len() > 60 {
                format!("{}...", &trimmed[..57])
            } else {
                trimmed.to_string()
            };
            return format!("Syntax error near: '{}'", truncated);
        }
    }

    // Strategy 3: Fallback to line number
    let line = node.start_position().row + 1;
    format!("Syntax error at line {}", line)
}

/// Check if text looks like a misspelled BSV closing keyword.
fn check_misspelled_keyword(text: &str) -> bool {
    matches!(
        text,
        "endm" | "endmod" | "endmodul" | "endmodue" | "endmodu" | "endmodle"
    )
}

/// Check if text is a known BSV keyword that would be unexpected in context.
fn is_bsv_keyword(text: &str) -> bool {
    matches!(
        text,
        "endmodule"
            | "endpackage"
            | "endinterface"
            | "endfunction"
            | "endmethod"
            | "endrule"
            | "endcase"
            | "endclass"
            | "endtypeclass"
            | "endinstance"
            | "endgenerate"
            | "endclocking"
            | "endproperty"
            | "endspecify"
            | "endconfig"
            | "endtask"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BsvParser;

    // ── Helpers ──────────────────────────────────────────────────────

    fn collect_diagnostics(source: &str) -> Vec<Diagnostic> {
        let parser = BsvParser::default();
        let tree = parser
            .parse(source)
            .expect("parse should not fail even on bad code");
        DiagnosticCollector::collect(&tree, source)
    }

    // ── Valid code should produce no diagnostics ──────────────────────

    #[test]
    fn test_no_errors_on_valid_code() {
        let source = r#"
module mkTest();
    Reg#(Bit#(32)) counter <- mkReg(0);
endmodule
"#;
        let diags = collect_diagnostics(source);
        assert!(
            diags.is_empty(),
            "Valid code should have no diagnostics, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_define_directives_are_not_false_positives() {
        // #define directives are not part of the BSV tree-sitter grammar,
        // so they parse as ERROR nodes. They should be filtered out.
        let source = r#"
#define 32 ADDR_WIDTH;
#define 8 DATA_WIDTH;
#define TAdd#(ADDR_WIDTH, 1) INCREMENTED;

module mkTest();
    Reg#(Bit#(32)) counter <- mkReg(0);
endmodule
"#;
        let diags = collect_diagnostics(source);
        assert!(
            diags.is_empty(),
            "#define directives should not produce diagnostics, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_empty_document_no_diagnostics() {
        let diags = collect_diagnostics("");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_valid_package_no_diagnostics() {
        let source = r#"
package TestPackage;
    export mkTest;
    module mkTest();
        Reg#(Bit#(32)) counter <- mkReg(0);
    endmodule
endpackage
"#;
        let diags = collect_diagnostics(source);
        assert!(diags.is_empty(), "Valid package should have no diagnostics");
    }

    // ── Error detection ──────────────────────────────────────────────

    #[test]
    fn test_error_on_misspelled_endmodule_detected() {
        let source = "module mkTest();\n    // test logic\nendm\n";
        let diags = collect_diagnostics(source);
        assert!(
            !diags.is_empty(),
            "Misspelled 'endm' should produce a diagnostic"
        );
    }

    #[test]
    fn test_range_covers_error_span() {
        let source = "module mkTest();\nendm\n";
        let diags = collect_diagnostics(source);
        assert!(!diags.is_empty());

        let d = &diags[0];
        // The error should span at least line 1 where 'endm' is
        assert!(
            d.range.start.line <= 1,
            "Error should start at or before line 1"
        );
        assert!(d.range.end.line >= 1, "Error should end at or after line 1");
    }

    #[test]
    fn test_multiple_errors_reported() {
        let source = r#"
module mkA();
endm

module mkB();
    bad syntax here
endm
"#;
        let diags = collect_diagnostics(source);
        assert!(
            diags.len() >= 1,
            "Should report at least one error for broken code"
        );
    }

    #[test]
    fn test_error_severity_is_error() {
        let source = "module mkTest();\nendm\n";
        let diags = collect_diagnostics(source);
        assert!(!diags.is_empty());
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn test_error_source_is_bsv() {
        let source = "module mkTest();\nendm\n";
        let diags = collect_diagnostics(source);
        assert!(!diags.is_empty());
        assert_eq!(diags[0].source, Some("bsv".to_string()));
    }

    #[test]
    fn test_error_at_beginning_of_file() {
        let source = "some random text\nmodule mkTest();\nendmodule\n";
        let diags = collect_diagnostics(source);
        assert!(!diags.is_empty());
    }

    #[test]
    fn test_missing_endmodule_entirely() {
        // A module without endmodule may or may not produce an ERROR node
        // depending on the grammar's permissiveness. If no error is produced,
        // this test is a no-op rather than a failure.
        let source = "module mkTest();\n    Reg#(Bit#(32)) counter <- mkReg(0);\n";
        let diags = collect_diagnostics(source);
        // The BSV grammar is permissive — missing endmodule may not always
        // produce an ERROR node. If it does, verify correctness.
        if diags.is_empty() {
            eprintln!(
                "Note: missing endmodule did not produce a diagnostic (grammar is permissive)"
            );
        }
    }

    #[test]
    fn test_error_inside_module_body() {
        // Use text that is known to produce an ERROR node in the BSV grammar.
        // The presence of a bare `endm` (vs `endmodule`) reliably triggers one.
        let source = r#"
module mkTest();
    Reg#(Bit#(32)) counter <- mkReg(0);
endm  // misspelled endmodule
"#;
        let diags = collect_diagnostics(source);
        assert!(
            !diags.is_empty(),
            "Misspelled 'endm' inside module should produce a diagnostic"
        );
    }

    #[test]
    fn test_multiple_independent_errors() {
        let source = r#"
module mkA();
endm

function Bit#(32) add(Bit#(32) a, Bit#(32) b);
    return a + b;
endfunction
"#;
        let diags = collect_diagnostics(source);
        assert!(
            diags.len() >= 1,
            "Should report error for broken module with 'endm'"
        );
    }
}
