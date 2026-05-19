//! Integration tests for LSP diagnostics using real BSV fixture files.
//!
//! These tests verify that the DiagnosticCollector produces correct results
//! when processing full `.bsv` files from the test_fixtures directory.

use bsv_language_server::diagnostics::DiagnosticCollector;
use bsv_language_server::BsvParser;

/// Path helper: returns the full path to a fixture file.
fn fixture_path(name: &str) -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("test_fixtures");
    path.push(name);
    path
}

/// Parse a fixture file and collect diagnostics.
fn collect_fixture_diagnostics(name: &str) -> Vec<lsp_types::Diagnostic> {
    let source = std::fs::read_to_string(fixture_path(name))
        .unwrap_or_else(|e| panic!("Failed to read fixture '{}': {}", name, e));

    let parser = BsvParser::default();
    let tree = parser
        .parse(&source)
        .expect("parse should succeed even on broken code");

    DiagnosticCollector::collect(&tree, &source)
}

#[test]
fn test_broken_fixture_has_diagnostics() {
    let diags = collect_fixture_diagnostics("broken.bsv");
    assert!(
        !diags.is_empty(),
        "broken.bsv should produce at least one diagnostic"
    );
}

#[test]
fn test_correct_fixture_no_diagnostics() {
    let diags = collect_fixture_diagnostics("correct.bsv");
    assert!(
        diags.is_empty(),
        "correct.bsv should produce no diagnostics, got: {:?}",
        diags
    );
}

#[test]
fn test_broken_fixture_diagnostic_mentions_syntax() {
    let diags = collect_fixture_diagnostics("broken.bsv");
    assert!(!diags.is_empty());
    let messages: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
    eprintln!("Diagnostic messages: {:?}", messages);
    assert!(
        diags.iter().any(|d| d.message.contains("endm")
            || d.message.contains("endmodule")
            || d.message.contains("Syntax error")),
        "Expected a diagnostic mentioning the misspelled 'endm' or syntax error, got: {:?}",
        diags
    );
}

#[test]
fn test_broken_fixture_error_range() {
    let diags = collect_fixture_diagnostics("broken.bsv");
    assert!(!diags.is_empty());
    // The narrowed range targets the `endm` keyword on line 12 (0-indexed).
    let covers_error = diags
        .iter()
        .any(|d| d.range.start.line == 12 && d.range.end.line == 12);
    assert!(
        covers_error,
        "Expected diagnostic range targeted at line 12 (the 'endm' line), got ranges: {:?}",
        diags.iter().map(|d| d.range).collect::<Vec<_>>()
    );
}
