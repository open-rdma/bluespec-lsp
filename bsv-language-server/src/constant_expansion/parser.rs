// Parser for #define constant definitions

use crate::constant_expansion::types::ConstantDef;
use lsp_types::{Position, Range};
use regex::Regex;

/// Parser for BSV #define constant definitions
pub struct ConstantParser {
    /// Regex for matching #define statements
    define_regex: Regex,
}

impl ConstantParser {
    pub fn new() -> Self {
        // Match #define patterns like:
        // #define 2 FOO;
        // #define TAdd#(FOO, 1) BAR;
        // The pattern: #define <value/expression> <name>;
        //
        // The value can be:
        // - A simple number: 2, 42, 0x10
        // - A type function: TAdd#(FOO, 1), TMul#(WIDTH, 2)
        // - A constant name: FOO, BAR
        //
        // We need to match until we find the name (identifier) followed by semicolon
        let define_regex = Regex::new(r#"#define\s+([^\n;]+)\s+([A-Za-z_][A-Za-z0-9_]*)\s*;"#)
            .expect("Invalid regex");

        Self { define_regex }
    }

    /// Parse all #define constants from source code
    pub fn parse(&self, source: &str) -> Vec<ConstantDef> {
        let mut constants = Vec::new();

        log::info!(
            "Parsing source for #define constants, length={}",
            source.len()
        );

        for caps in self.define_regex.captures_iter(source) {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let name = caps.get(2).map(|m| m.as_str()).unwrap_or("");

            if name.is_empty() || value.is_empty() {
                continue;
            }

            // Calculate the range
            let full_match = caps.get(0).unwrap();
            let start = full_match.start();
            let end = full_match.end();

            let (start_line, start_col) = self.byte_offset_to_position(source, start);
            let (end_line, end_col) = self.byte_offset_to_position(source, end);

            let range = Range {
                start: Position {
                    line: start_line,
                    character: start_col,
                },
                end: Position {
                    line: end_line,
                    character: end_col,
                },
            };

            // Determine if this is a simple numeric constant
            let is_simple = parse_i64(value).is_ok();

            constants.push(ConstantDef {
                name: name.to_string(),
                value: value.to_string(),
                range,
                is_simple,
            });

            log::info!("Found #define: {} = {}", name, value);
        }

        log::info!("Parsed {} #define constants", constants.len());
        constants
    }

    /// Convert byte offset to line/column position
    fn byte_offset_to_position(&self, source: &str, offset: usize) -> (u32, u32) {
        let mut line = 0u32;
        let mut col = 0u32;

        for (i, ch) in source.char_indices() {
            if i >= offset {
                break;
            }

            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }

        (line, col)
    }

    /// Find the constant definition at a given position
    pub fn find_constant_at_position(
        &self,
        source: &str,
        position: Position,
    ) -> Option<ConstantDef> {
        let constants = self.parse(source);

        for def in constants.clone() {
            // Check if position is within the constant name
            // The name appears after "#define <value> "
            // We need to find where the name starts in the source

            // Find the #define statement in the source
            let search_start = self.position_to_byte_offset(source, def.range.start);
            let search_end = self.position_to_byte_offset(source, def.range.end);

            if let Some(define_text) = source.get(search_start..search_end) {
                // Find the name within the #define statement
                // Pattern: #define <value> <name>;
                if let Some(caps) = self.define_regex.captures(define_text) {
                    if let Some(name_match) = caps.get(2) {
                        let name_start_in_define = name_match.start();
                        let name_end_in_define = name_match.end();

                        // Convert to absolute position
                        let name_start_col =
                            def.range.start.character + name_start_in_define as u32;
                        let name_end_col = def.range.start.character + name_end_in_define as u32;

                        // Check if position is within the name
                        if def.range.start.line == position.line
                            && position.character >= name_start_col
                            && position.character <= name_end_col
                        {
                            return Some(def);
                        }
                    }
                }
            }
        }

        None
    }

    /// Convert position to byte offset
    fn position_to_byte_offset(&self, source: &str, position: Position) -> usize {
        let mut offset = 0;
        let mut line = 0u32;

        for ch in source.chars() {
            if line == position.line {
                // We're on the target line, count characters
                let chars_before_col: usize = source[offset..]
                    .chars()
                    .take(position.character as usize)
                    .map(|c| c.len_utf8())
                    .sum();
                return offset + chars_before_col;
            }

            offset += ch.len_utf8();
            if ch == '\n' {
                line += 1;
            }
        }

        offset
    }

    /// Find a constant by name in the source
    pub fn find_constant_by_name(&self, source: &str, name: &str) -> Option<ConstantDef> {
        let constants = self.parse(source);
        constants.into_iter().find(|def| def.name == name)
    }
}

impl Default for ConstantParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a string as i64, supporting decimal, hex (0x), and binary (0b) formats
pub fn parse_i64(s: &str) -> Result<i64, std::num::ParseIntError> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16)
    } else if let Some(bin) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        i64::from_str_radix(bin, 2)
    } else {
        s.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_define() {
        let source = "#define 2 FOO;";
        let parser = ConstantParser::new();
        let defs = parser.parse(source);

        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "FOO");
        assert_eq!(defs[0].value, "2");
        assert!(defs[0].is_simple);
    }

    #[test]
    fn test_parse_type_function_define() {
        let source = "#define TAdd#(FOO, 1) BAR;";
        let parser = ConstantParser::new();
        let defs = parser.parse(source);

        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "BAR");
        assert_eq!(defs[0].value, "TAdd#(FOO, 1)");
        assert!(!defs[0].is_simple);
    }

    #[test]
    fn test_parse_multiple_defines() {
        let source = r#"
#define 2 FOO;
#define TAdd#(FOO, 1) BAR;
#define 10 BAZ;
"#;
        let parser = ConstantParser::new();
        let defs = parser.parse(source);

        assert_eq!(defs.len(), 3);
        assert_eq!(defs[0].name, "FOO");
        assert_eq!(defs[1].name, "BAR");
        assert_eq!(defs[2].name, "BAZ");
    }

    #[test]
    fn test_parse_with_comments() {
        let source = r#"
// This is a comment
#define 4 WIDTH; // inline comment
/* multi-line
   comment */
#define TAdd#(WIDTH, 4) EXTENDED;
"#;
        let parser = ConstantParser::new();
        let defs = parser.parse(source);

        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].name, "WIDTH");
        assert_eq!(defs[1].name, "EXTENDED");
    }

    #[test]
    fn test_find_constant_at_position() {
        let source = "#define 2 FOO;";
        let parser = ConstantParser::new();

        // The constant FOO is at position 10-13 (after "#define 2 ")
        // Let's verify by checking the parsed constant's range
        let defs = parser.parse(source);
        assert_eq!(defs.len(), 1);

        // Debug: print the actual range
        println!("Constant FOO range: {:?}", defs[0].range);

        // The range should be for the entire #define statement
        // We need to find the name within that range
        // Let's use the actual range from the parsed constant
        let name_start = defs[0].range.start.character + "#define 2 ".len() as u32;

        // Position at "F" in FOO
        let pos = Position {
            line: 0,
            character: name_start,
        };
        let result = parser.find_constant_at_position(source, pos);

        assert!(
            result.is_some(),
            "Should find constant at position {}",
            name_start
        );
        assert_eq!(result.unwrap().name, "FOO");
    }
}

#[test]
fn test_hex_constant_parsing() {
    let source = "#define 0xFF VALUE;";
    let parser = ConstantParser::new();
    let defs = parser.parse(source);

    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "VALUE");
    assert_eq!(defs[0].value, "0xFF");
    assert!(defs[0].is_simple);
}

#[test]
fn test_parse_i64_hex() {
    assert_eq!(parse_i64("0xFF").unwrap(), 255);
    assert_eq!(parse_i64("0xff").unwrap(), 255);
    assert_eq!(parse_i64("0x10").unwrap(), 16);
    assert_eq!(parse_i64("0x0").unwrap(), 0);
}

#[test]
fn test_parse_i64_decimal() {
    assert_eq!(parse_i64("42").unwrap(), 42);
    assert_eq!(parse_i64("0").unwrap(), 0);
    assert_eq!(parse_i64("-1").unwrap(), -1);
}

#[test]
fn test_parse_i64_binary() {
    assert_eq!(parse_i64("0b1010").unwrap(), 10);
    assert_eq!(parse_i64("0b0").unwrap(), 0);
}

#[test]
fn test_parse_i64_invalid() {
    assert!(parse_i64("").is_err());
    assert!(parse_i64("abc").is_err());
    assert!(parse_i64("0x").is_err());
}
