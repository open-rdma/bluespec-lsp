//! BSV code formatter.
//!
//! Uses an AST-guided line-reformatting approach:
//!
//! 1. **Indentation pass** -- Walk the tree-sitter CST and compute an
//!    indent level for each line based on block-structure nesting.
//! 2. **Spacing pass** -- Fix whitespace around operators and keywords.
//! 3. **Cleanup pass** -- Strip trailing whitespace, normalise blank
//!    lines, and ensure the file ends with a single newline.
//!
//! # Design decisions
//!
//! - **Line-based, not AST-rebuild**: Rebuilding source from the CST is
//!   fragile because comments live in the tree-sitter "extras" (outside
//!   the node tree).  Re-indenting each line preserves all comments and
//!   raw tokens while fixing the most visible formatting concerns.
//! - **3-space indent** is the de-facto BSV convention, matching both
//!   `bsc` output and the `bsv_format` formatter from jahagirdar's
//!   bsv-language-server.

use crate::BsvParser;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Block-structure nodes whose interior content gets one extra indent level.
const BLOCK_KINDS: &[&str] = &[
    "moduleDef",
    "interfaceDecl",
    "interfaceExpr",
    "methodDef",
    "functionDef",
    "rule",
    "typeclassDef",
    "typeclassInstanceDef",
    "externModuleImport",
    "actionBlock",
    "actionValueBlock",
    "seqFsmStmt",
    "parFsmStmt",
    "beginEndExpr",
    "subinterfaceDef",
    "typedefStruct",
    "typedefEnum",
    "typedefTaggedUnion",
    "rulesExpr",
];

/// Statement-wrapper kinds that may contain a begin/end or if/for block.
/// These are not always multi-line blocks (they also wrap simple statements),
/// so we only add indent when they actually span multiple lines — which the
/// existing `end_row > start_row` guard already handles.
const BEGIN_END_WRAPPER_KINDS: &[&str] = &[
    "actionStmt",
    "expressionStmt",
    "actionValueStmt",
    "functionBodyStmt",
];

// ── Cached regexes for the spacing pass ─────────────────────────

/// Insert space after `if`, `for`, `while`, `case`, `return`, `action`,
/// etc., when followed immediately by `(`.
static RE_KW_PAREN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(if|for|while|case|return|action|endaction|actionvalue|endactionvalue|endtypeclass|endinstance)\(").unwrap()
});

/// Insert space before `{` in struct literal contexts (e.g. `Foo{` -> `Foo {`).
static RE_STRUCT_BRACE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\w)\{").unwrap()
});

/// Ensure `=` has spaces around it (but not `==`, `!=`, `<=`, `>=`).
///
/// Uses a capture-group approach because Rust's `regex` crate does not
/// support lookbehind assertions.  The pattern matches `=` preceded by
/// a non-operator character and followed by a non-`=` character.
static RE_EQ: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"([^!<>=])=([^=])").unwrap()
});

/// Ensure ` <- ` has spaces around it (connection / action operator).
///
/// Uses capture groups to avoid lookbehind: captures non-whitespace on
/// both sides and collapses any existing whitespace around the operator.
static RE_LARROW: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\S)\s*<-\s*(\S)").unwrap()
});

/// Ensure ` <= ` has spaces around it (less-than-or-equal).
static RE_LE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\S)\s*<=\s*(\S)").unwrap()
});

/// Ensure ` < ` has spaces around it (less-than), but NOT when `<` is
/// followed by `=` (that's `<=`, already handled) or `-` (that would be
/// the tail of `<-`, which was already handled by `RE_LARROW`).
/// Uses `[^=\s-]` to exclude `=`, whitespace, and `-` after `<`.
static RE_LT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\w+)\s*<\s*(\w+)").unwrap()
});

/// Ensure ` + ` has spaces around it, avoiding `++` (list concatenation).
/// Uses `[^+]` capture groups (not lookahead, which `regex` lacks).
static RE_PLUS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"([^+])\+\s*([^+])").unwrap()
});

/// Ensure ` : ` after struct field names (e.g. `a:1` -> `a: 1`).
/// Does not match `#(type)` or `::` contexts because those don't have
/// `\w` immediately before `:`.
static RE_COLON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\w):(\S)").unwrap()
});

/// Ensure ` ; ` has a space after it in for-loop headers and similar
/// contexts (e.g. `i=0;i<10` -> `i = 0; i < 10`).
static RE_SEMI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r";(\S)").unwrap()
});

// ── Public API ──────────────────────────────────────────────────

/// The BSV code formatter.
#[derive(Default)]
pub struct BsvFormatter {
    parser: BsvParser,
}

impl BsvFormatter {
    /// Create a new formatter (wraps the shared tree-sitter parser).
    pub fn new() -> Self {
        Self::default()
    }

    /// Format an entire BSV source document.
    ///
    /// Returns `None` when the source cannot be parsed (the caller
    /// should fall back to returning the original text unchanged
    /// with only whitespace cleanup).
    pub fn format(&self, source: &str) -> Option<String> {
        // Replace preprocessor lines (backtick-prefixed) with line comments
        // before parsing, because tree-sitter does not recognise them and
        // produces a broken AST.  Comment lines (//...) live in tree-sitter
        // "extras" and are invisible to the grammar, so they don't affect
        // the parse tree at all -- line numbers are preserved perfectly.
        let sanitised: String = source
            .lines()
            .map(|line| {
                if line.trim_start().starts_with('`') {
                    let indent = &line[..line.len() - line.trim_start().len()];
                    format!("{}// preprocessor", indent)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let tree = self.parser.parse(&sanitised).ok()?;

        // ── Phase 1: Compute per-line indent levels ──────────────
        let mut indent_map = Self::compute_indent_map(&tree);

        // ── Phase 1b: Correct indent levels near preprocessor   ──
        //     conditionals (tree-sitter cannot parse `ifdef/`else/
        //     `endif and creates phantom nesting).
        Self::correct_preprocessor_indents(source, &mut indent_map);

        // ── Phase 2: Re-indent & apply spacing passes ────────────
        let lines: Vec<&str> = source.lines().collect();
        let mut result_lines: Vec<String> = Vec::with_capacity(lines.len());

        let mut in_block_comment = false;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Detect block comment regions -- preserve interior lines as-is.
            if trimmed.starts_with("/*") {
                in_block_comment = true;
            }
            if in_block_comment {
                result_lines.push(Self::cleanup_line(line));
                if trimmed.contains("*/") {
                    in_block_comment = false;
                }
                continue;
            }

            // Preserve preprocessor lines (backtick-prefixed) -- only cleanup.
            if trimmed.starts_with('`') {
                result_lines.push(Self::cleanup_line(line));
                continue;
            }

            // Preserve blank lines (collapse multiple to one later).
            if trimmed.is_empty() {
                result_lines.push(String::new());
                continue;
            }

            let indent_level = indent_map.get(&i).copied().unwrap_or(0);
            let indent_str = "   ".repeat(indent_level);

            let mut formatted = format!("{}{}", indent_str, trimmed);

            // ── Phase 3: Spacing pass (per-line) ─────────────────
            formatted = Self::apply_spacing_rules(&formatted);

            result_lines.push(formatted);
        }

        // ── Phase 4: Global cleanup ──────────────────────────────
        let mut output = Self::collapse_blank_lines(&result_lines);

        // Ensure trailing newline.
        if !output.ends_with('\n') {
            output.push('\n');
        }

        Some(output)
    }

    // ── Indentation computation ─────────────────────────────────

    /// Correct indent levels that tree-sitter got wrong inside
    /// preprocessor conditional regions (`ifdef/`else/`endif).
    ///
    /// Tree-sitter cannot parse backtick directives and produces
    /// phantom nesting when conditionals split structural blocks
    /// (e.g. two `module` declarations separated by `` `else ``).
    /// The `ifdef..else` branch is parsed correctly; the `else..endif`
    /// branch gets +1 phantom indent because tree-sitter sees it as
    /// nested inside the first branch's outer structural block.
    fn correct_preprocessor_indents(source: &str, indent_map: &mut HashMap<usize, usize>) {
        let lines: Vec<&str> = source.lines().collect();

        // Stack of `ifdef positions paired with `else positions.
        // Each entry: (ifdef_line, else_line_or_none)
        let mut stack: Vec<(usize, Option<usize>)> = Vec::new();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if !trimmed.starts_with('`') {
                continue;
            }
            let directive = trimmed
                .trim_start_matches('`')
                .split_whitespace()
                .next()
                .unwrap_or("");

            match directive {
                "ifdef" | "ifndef" => {
                    stack.push((i, None));
                }
                "elsif" | "else" => {
                    if let Some(entry) = stack.last_mut() {
                        entry.1 = Some(i);
                    }
                }
                "endif" => {
                    if let Some((_ifdef_line, Some(else_ln))) = stack.pop() {
                        // Correct `else..endif: subtract 1 from
                        // non-preprocessor lines
                        for (j, l) in lines.iter().enumerate().take(i).skip(else_ln + 1) {
                            if !l.trim_start().starts_with('`') {
                                if let Some(level) = indent_map.get_mut(&j) {
                                    *level = level.saturating_sub(1);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Walk the CST and build a `line -> indent_level` map.
    ///
    /// The algorithm processes lines sequentially:
    ///
    /// 1. For each line, subtract closing blocks (blocks whose end line
    ///    equals this line) to get the line's indent.
    /// 2. Apply the indent to the line.
    /// 3. Add opening blocks (blocks whose start line equals this line)
    ///    so subsequent lines see the deeper level.
    fn compute_indent_map(tree: &tree_sitter::Tree) -> HashMap<usize, usize> {
        let mut start_counts: HashMap<usize, usize> = HashMap::new();
        let mut end_counts: HashMap<usize, usize> = HashMap::new();

        let root = tree.root_node();
        Self::collect_block_lines(root, &mut start_counts, &mut end_counts, None);

        let max_line = start_counts
            .keys()
            .chain(end_counts.keys())
            .max()
            .copied()
            .unwrap_or(0);

        let mut indent_map: HashMap<usize, usize> = HashMap::new();
        let mut current_indent: usize = 0;

        for line in 0..=max_line {
            // De-dent before applying: closing keywords sit one level down.
            let closing = end_counts.get(&line).copied().unwrap_or(0);
            let indent = current_indent.saturating_sub(closing);
            indent_map.insert(line, indent);

            // Update indent for subsequent lines.
            let opening = start_counts.get(&line).copied().unwrap_or(0);
            current_indent = current_indent
                .saturating_sub(closing)
                .saturating_add(opening);
        }

        indent_map
    }

    /// Recursively walk nodes and count block start/end lines.
    ///
    /// `phantom_nesting` tracks the block kind when we are already inside
    /// a node of that kind (e.g. `moduleDef` inside `moduleDef`).  BSV
    /// never nests modules, so any such occurrence is a phantom created
    /// by preprocessor directives fragmented across the AST.  Phantom
    /// nodes are skipped — their start/end lines are not registered.
    fn collect_block_lines(
        node: tree_sitter::Node,
        starts: &mut HashMap<usize, usize>,
        ends: &mut HashMap<usize, usize>,
        phantom_nesting: Option<&str>,
    ) {
        let kind = node.kind();

        // Detect phantom nesting: a block node inside a parent of the
        // same kind (e.g. moduleDef inside moduleDef).  BSV never nests
        // these structurally; the duplication is a preprocessor artifact.
        let is_phantom = phantom_nesting == Some(kind);

        if !is_phantom && BLOCK_KINDS.contains(&kind) {
            let start_row = node.start_position().row;
            let end_row = node.end_position().row;

            // Only register blocks spanning at least 2 lines.
            if end_row > start_row {
                *starts.entry(start_row).or_insert(0) += 1;
                *ends.entry(end_row).or_insert(0) += 1;
            }
        } else if !is_phantom && BEGIN_END_WRAPPER_KINDS.contains(&kind) {
            // Wrapper nodes (e.g. actionStmt wrapping begin/end inside action)
            // create indent only when their interior spans multiple lines AND
            // they wrap a begin/end keyword pair.
            let start_row = node.start_position().row;
            let end_row = node.end_position().row;
            if end_row > start_row + 1 {
                if let Some(last) = node.child(node.child_count().saturating_sub(1)) {
                    if last.kind() == "end" {
                        *starts.entry(start_row).or_insert(0) += 1;
                        *ends.entry(end_row).or_insert(0) += 1;
                    }
                }
            }
        }

        // Determine the phantom kind for children.
        // If this node is a block kind and NOT itself phantom, children inherit it.
        let child_phantom = if !is_phantom && BLOCK_KINDS.contains(&kind) {
            Some(kind)
        } else {
            phantom_nesting
        };

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::collect_block_lines(child, starts, ends, child_phantom);
        }
    }

    // ── Spacing rules ────────────────────────────────────────────

    /// Apply spacing fixes to a single (already indented) line.
    fn apply_spacing_rules(line: &str) -> String {
        let mut s = line.to_string();

        // 1. Keyword + paren: `if(` -> `if (`
        s = RE_KW_PAREN
            .replace_all(&s, |caps: &regex::Captures| format!("{} (", &caps[1]))
            .to_string();

        // 2. Struct literal brace: `Foo{` -> `Foo {`
        s = RE_STRUCT_BRACE
            .replace_all(&s, |caps: &regex::Captures| format!("{} {{", &caps[1]))
            .to_string();

        // 3. `=`: ensure spaces around it (but not `==`, `!=`, `<=`, `>=`)
        s = RE_EQ.replace_all(&s, "$1 = $2").to_string();

        // 4. `<-`: ensure spaces
        s = RE_LARROW
            .replace_all(&s, "$1 <- $2")
            .to_string();

        // 5. `<=`: ensure spaces
        s = RE_LE
            .replace_all(&s, "$1 <= $2")
            .to_string();

        // 6. `<`: ensure spaces (but only after `<=` has been handled)
        s = RE_LT
            .replace_all(&s, "$1 < $2")
            .to_string();

        // 7. `+`: ensure spaces (avoid `++`)
        s = RE_PLUS
            .replace_all(&s, "$1 + $2")
            .to_string();

        // 8. `:`: ensure space after colon in struct fields (e.g. `a:1` -> `a: 1`)
        s = RE_COLON
            .replace_all(&s, "$1: $2")
            .to_string();

        // 9. `;`: ensure space after semicolons in for-loop headers
        //     (e.g. `i=0;i<10` -> `i=0; i<10`)
        s = RE_SEMI
            .replace_all(&s, "; $1")
            .to_string();

        // Collapse multiple spaces (but preserve leading indentation).
        let trimmed = s.trim_start();
        let indent_len = s.len() - trimmed.len();
        let indent = &s[..indent_len];
        let collapsed = RE_MULTI_SPACE.replace_all(trimmed, " ").to_string();
        format!("{}{}", indent, collapsed)
    }

    // ── Cleanup helpers ──────────────────────────────────────────

    /// Strip trailing whitespace from a single line.
    fn cleanup_line(line: &str) -> String {
        line.trim_end().to_string()
    }

    /// Collapse consecutive blank lines into at most one.
    fn collapse_blank_lines(lines: &[String]) -> String {
        let mut out = String::new();
        let mut prev_blank = false;

        for line in lines {
            let is_blank = line.trim().is_empty();
            if is_blank && prev_blank {
                continue;
            }
            prev_blank = is_blank;
            out.push_str(line);
            out.push('\n');
        }

        out
    }
}

static RE_MULTI_SPACE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[ ]{2,}").unwrap());

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(source: &str) -> String {
        let formatter = BsvFormatter::new();
        formatter.format(source).unwrap()
    }

    // ── Indentation ───────────────────────────────────────────────

    #[test]
    fn test_empty_source() {
        assert_eq!(fmt(""), "\n");
    }

    #[test]
    fn test_single_line() {
        let input = "  module mkFoo(); endmodule  ";
        let expected = "module mkFoo(); endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_module_body_indented() {
        let input = "\
module mkFoo();
rule r;
$display(\"hello\");
endrule
endmodule";
        let expected = "\
module mkFoo();
   rule r;
      $display(\"hello\");
   endrule
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_nested_rules() {
        let input = "\
module mkTop();
rule ra;
let x = 1;
endrule
rule rb;
let y = 2;
endrule
endmodule";
        let expected = "\
module mkTop();
   rule ra;
      let x = 1;
   endrule
   rule rb;
      let y = 2;
   endrule
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_nested_if_in_action() {
        let input = "\
module mkTest();
rule r;
action
if (x) y();
else z();
endaction
endrule
endmodule";
        let expected = "\
module mkTest();
   rule r;
      action
         if (x) y();
         else z();
      endaction
   endrule
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_function_indentation() {
        let input = "\
function Bit#(32) add(Bit#(32) a, Bit#(32) b);
return a + b;
endfunction";
        let expected = "\
function Bit#(32) add(Bit#(32) a, Bit#(32) b);
   return a + b;
endfunction\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_interface_indentation() {
        let input = "\
interface I;
method Bit#(8) get();
endmethod
endinterface";
        let expected = "\
interface I;
   method Bit#(8) get();
   endmethod
endinterface\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_typedef_struct_indentation() {
        let input = "\
typedef struct {
Bit#(8) field;
Bit#(16) other;
} S deriving(Bits);";
        let expected = "\
typedef struct {
   Bit#(8) field;
   Bit#(16) other;
} S deriving(Bits);\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_typedef_enum_indentation() {
        let input = "\
typedef enum {
Idle,
Busy,
Done
} State deriving(Bits);";
        let expected = "\
typedef enum {
   Idle,
   Busy,
   Done
} State deriving(Bits);\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_begin_end_in_action() {
        let input = "\
module mkTest();
rule r;
action
begin
$display(\"a\");
$display(\"b\");
end
endaction
endrule
endmodule";
        let expected = "\
module mkTest();
   rule r;
      action
         begin
            $display(\"a\");
            $display(\"b\");
         end
      endaction
   endrule
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    // ── Idempotency ───────────────────────────────────────────────

    #[test]
    fn test_already_formatted_is_idempotent() {
        let input = "\
module mkFoo();
   rule r;
      $display(\"ok\");
   endrule
endmodule\n";
        assert_eq!(fmt(input), input);
    }

    #[test]
    fn test_double_formatting_is_stable() {
        let input = "\
module mkFoo();
   rule r;
      $display(\"ok\");
   endrule
endmodule\n";
        let once = fmt(input);
        let twice = fmt(&once);
        assert_eq!(once, twice);
    }

    // ── Spacing ───────────────────────────────────────────────────

    #[test]
    fn test_spacing_larrow() {
        let input = "module mkTest();
let x<-mkReg(0);
endmodule";
        let expected = "\
module mkTest();
   let x <- mkReg(0);
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_spacing_reg_write() {
        let input = "module mkTest();
rule r;
x<=y;
endrule
endmodule";
        let expected = "\
module mkTest();
   rule r;
      x <= y;
   endrule
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_spacing_if_keyword() {
        let input = "module mkTest();
rule r;
if(x) y();
endrule
endmodule";
        let expected = "\
module mkTest();
   rule r;
      if (x) y();
   endrule
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_spacing_for_keyword() {
        let input = "module mkTest();
rule r;
for(i=0;i<10;i=i+1) y();
endrule
endmodule";
        let expected = "\
module mkTest();
   rule r;
      for (i = 0; i < 10; i = i + 1) y();
   endrule
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_spacing_struct_literal() {
        let input = "module mkTest();
let s = Foo{a:1, b:2};
endmodule";
        let expected = "\
module mkTest();
   let s = Foo {a: 1, b: 2};
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_spacing_return() {
        let input = "function Bit#(32) add(Bit#(32) a, Bit#(32) b);
return a+b;
endfunction";
        let expected = "\
function Bit#(32) add(Bit#(32) a, Bit#(32) b);
   return a + b;
endfunction\n";
        assert_eq!(fmt(input), expected);
    }

    // ── Comment preservation ──────────────────────────────────────

    #[test]
    fn test_line_comments_preserved() {
        let input = "\
// top comment
module mkFoo();
// inside comment
rule r;
$display(\"hi\"); // inline
endrule
endmodule";
        let expected = "\
// top comment
module mkFoo();
   // inside comment
   rule r;
      $display(\"hi\"); // inline
   endrule
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_block_comments_preserved() {
        let input = "\
module mkFoo();
/*
 * multi-line
 * comment
 */
rule r;
$display(\"hi\");
endrule
endmodule";
        let expected = "\
module mkFoo();
/*
 * multi-line
 * comment
 */
   rule r;
      $display(\"hi\");
   endrule
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    // ── Cleanup ───────────────────────────────────────────────────

    #[test]
    fn test_trailing_whitespace_removed() {
        let input =
            "module mkFoo();   \n   rule r;  \n      $display(\"hi\");   \n   endrule   \nendmodule   \n";
        let expected = "\
module mkFoo();
   rule r;
      $display(\"hi\");
   endrule
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_consecutive_blank_lines_collapsed() {
        let input = "\
module mkFoo();


   rule r;



      $display(\"hi\");
   endrule
endmodule";
        let expected = "\
module mkFoo();

   rule r;

      $display(\"hi\");
   endrule
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_preprocessor_lines_preserved() {
        let input = "\
`ifdef BSIM
module mkSim();
`else
module mkReal();
`endif
rule r;
$display(\"hi\");
endrule
endmodule";
        let expected = "\
`ifdef BSIM
module mkSim();
`else
module mkReal();
`endif
   rule r;
      $display(\"hi\");
   endrule
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    // ── Edge cases ────────────────────────────────────────────────

    #[test]
    fn test_mixed_indentation_normalized() {
        let input = "\
module mkFoo();
    rule r;
        $display(\"mixed\");
    endrule
  endmodule";
        let expected = "\
module mkFoo();
   rule r;
      $display(\"mixed\");
   endrule
endmodule\n";
        assert_eq!(fmt(input), expected);
    }

    #[test]
    fn test_whole_document_from_correct_fixture() {
        let source = include_str!("../test_fixtures/correct.bsv");
        let formatter = BsvFormatter::new();
        let result = formatter.format(source);
        assert!(result.is_some(), "Formatter should handle correct.bsv");
        let output = result.unwrap();
        // Should not be empty and should end with newline
        assert!(!output.is_empty());
        assert!(output.ends_with('\n'));
        // Should be idempotent
        let second = formatter.format(&output).unwrap();
        assert_eq!(output, second);
    }

    #[test]
    fn test_constants_fixture() {
        let source = include_str!("../test_fixtures/constants.bsv");
        let formatter = BsvFormatter::new();
        let result = formatter.format(source);
        assert!(result.is_some(), "Formatter should handle constants.bsv");
        let output = result.unwrap();
        assert!(!output.is_empty());
        // Idempotent
        let second = formatter.format(&output).unwrap();
        assert_eq!(output, second);
    }
}