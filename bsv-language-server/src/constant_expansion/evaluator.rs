// Constant Evaluator - expands and evaluates constants

use crate::constant_expansion::parser::parse_i64;
use crate::constant_expansion::types::{ConstantDef, ExpansionResult, ExpansionStep, TypeFunction};
use std::collections::HashMap;

/// Evaluator for expanding and computing constant values
pub struct ConstantEvaluator {
    /// Map of constant name to definition
    constants: HashMap<String, ConstantDef>,

    /// Maximum expansion depth to prevent infinite loops
    max_depth: usize,
}

impl ConstantEvaluator {
    pub fn new(constants: Vec<ConstantDef>) -> Self {
        let constants: HashMap<String, ConstantDef> = constants
            .into_iter()
            .map(|def| (def.name.clone(), def))
            .collect();

        Self {
            constants,
            max_depth: 100,
        }
    }

    /// Try to find the outermost type function call (FuncName#(args)) in a value string
    /// by scanning with parenthesis counting to correctly handle nested calls.
    fn parse_type_function_call<'a>(&self, value: &'a str) -> Option<(&'a str, &'a str)> {
        let bytes = value.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            // Look for the start of an identifier followed by "#("
            if bytes[i] == b'#' && i + 1 < bytes.len() && bytes[i + 1] == b'(' {
                // Find the start of the function name by scanning backwards
                if i == 0 {
                    break;
                }
                let name_end = i;
                let mut name_start = i;
                while name_start > 0 {
                    let c = bytes[name_start - 1];
                    if c.is_ascii_alphanumeric() || c == b'_' {
                        name_start -= 1;
                    } else {
                        break;
                    }
                }
                if name_start == name_end {
                    i += 1;
                    continue;
                }
                let func_name = &value[name_start..name_end];
                if TypeFunction::from_name(func_name).is_none() {
                    i += 1;
                    continue;
                }

                // Now find the matching closing paren by counting depth
                let args_start = i + 2; // skip "#("
                let mut depth: u32 = 1;
                let mut j = args_start;
                while j < bytes.len() && depth > 0 {
                    if bytes[j] == b'(' {
                        depth += 1;
                    } else if bytes[j] == b')' {
                        depth -= 1;
                    }
                    j += 1;
                }
                if depth == 0 {
                    let args_str = &value[args_start..j - 1];
                    return Some((func_name, args_str));
                }
            }
            i += 1;
        }
        None
    }

    /// Split comma-separated args only at top-level commas (not inside nested parens).
    fn split_top_level_args<'a>(&self, args_str: &'a str) -> Vec<&'a str> {
        let mut args = Vec::new();
        let mut depth: u32 = 0;
        let mut start = 0;
        for (i, ch) in args_str.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => {
                    args.push(args_str[start..i].trim());
                    start = i + 1;
                }
                _ => {}
            }
        }
        args.push(args_str[start..].trim());
        args
    }

    /// Create an evaluator from a source string
    pub fn from_source(source: &str) -> Self {
        let parser = crate::constant_expansion::ConstantParser::new();
        Self::new(parser.parse(source))
    }

    /// Expand a constant and compute its value
    pub fn expand(&self, name: &str) -> Option<ExpansionResult> {
        let def = self.constants.get(name)?;

        let mut steps = Vec::new();
        let mut visited = vec![name.to_string()];

        // First step: show the original expression
        steps.push(ExpansionStep {
            expression: def.value.clone(),
            description: None,
            value: None,
            is_constant_ref: false, // Original expression, not a reference
            original_definition: None,
        });

        // Recursively expand and evaluate
        match self.expand_value(&def.value, &mut steps, &mut visited, 0) {
            Ok(final_value) => Some(ExpansionResult {
                name: name.to_string(),
                final_value,
                steps,
                success: true,
                error: None,
            }),
            Err(error) => Some(ExpansionResult {
                name: name.to_string(),
                final_value: 0,
                steps,
                success: false,
                error: Some(error),
            }),
        }
    }

    /// Expand a value expression recursively
    fn expand_value(
        &self,
        value: &str,
        steps: &mut Vec<ExpansionStep>,
        visited: &mut Vec<String>,
        depth: usize,
    ) -> Result<i64, String> {
        if depth > self.max_depth {
            return Err("Maximum expansion depth exceeded".to_string());
        }

        // Try to parse as a simple number
        if let Ok(n) = parse_i64(value) {
            // Only add a step if this is not redundant (i.e., not already shown)
            // Check if the last step already has this numeric value
            let is_redundant = steps
                .last()
                .is_some_and(|s| s.value == Some(n) && !s.is_constant_ref);

            if !is_redundant {
                steps.push(ExpansionStep {
                    expression: value.to_string(),
                    description: Some("numeric literal".to_string()),
                    value: Some(n),
                    is_constant_ref: false, // Numeric literal, not a constant reference
                    original_definition: None,
                });
            }
            return Ok(n);
        }

        // Try to parse as a type function call (outermost match with balanced parens)
        if let Some((func_name, args_str)) = self.parse_type_function_call(value) {
            if let Some(func) = TypeFunction::from_name(func_name) {
                return self.expand_type_function(func, args_str, steps, visited, depth);
            }
        }

        // Try to look up as a constant name
        if let Some(def) = self.constants.get(value) {
            if visited.contains(&value.to_string()) {
                return Err(format!("Circular reference detected: {}", value));
            }

            visited.push(value.to_string());

            // Add step showing the substitution
            let sub_value = self.expand_value(&def.value, steps, visited, depth + 1)?;

            steps.push(ExpansionStep {
                expression: format!("{} = {}", value, sub_value),
                description: Some(format!("substituted from {}", value)),
                value: Some(sub_value),
                is_constant_ref: true, // This is a constant reference
                original_definition: Some(def.value.clone()), // Store original definition
            });

            return Ok(sub_value);
        }

        // Try to evaluate as a complex expression with nested constants
        self.expand_complex_expression(value, steps, visited, depth)
    }

    /// Expand a type function call
    fn expand_type_function(
        &self,
        func: TypeFunction,
        args_str: &str,
        steps: &mut Vec<ExpansionStep>,
        visited: &mut Vec<String>,
        depth: usize,
    ) -> Result<i64, String> {
        // Parse arguments (comma-separated, respecting balanced parens)
        let args: Vec<&str> = self.split_top_level_args(args_str);

        // Expand each argument
        let mut expanded_args = Vec::new();
        for arg in &args {
            let expanded = self.expand_value(arg, steps, visited, depth + 1)?;
            expanded_args.push(expanded);
        }

        // Evaluate the function
        let result = func.evaluate(&expanded_args).ok_or_else(|| {
            format!(
                "Failed to evaluate {} with args {:?}",
                func.name(),
                expanded_args
            )
        })?;

        // Add step showing the evaluation (only if not redundant)
        let args_display: Vec<String> = expanded_args.iter().map(|n| n.to_string()).collect();
        let expr = format!("{}#({})", func.name(), args_display.join(", "));

        // Check if this step is redundant (same expression and value already exists)
        let is_redundant = steps
            .last()
            .is_some_and(|s| s.expression == expr && s.value == Some(result));

        if !is_redundant {
            steps.push(ExpansionStep {
                expression: expr,
                description: Some(format!("evaluated {}#({})", func.name(), args_str)),
                value: Some(result),
                is_constant_ref: false, // Type function evaluation, not a constant reference
                original_definition: None,
            });
        }

        Ok(result)
    }

    /// Expand a complex expression that may contain nested constants
    fn expand_complex_expression(
        &self,
        expr: &str,
        steps: &mut Vec<ExpansionStep>,
        visited: &mut Vec<String>,
        depth: usize,
    ) -> Result<i64, String> {
        // Check if this is a type function with nested constants
        if let Some((func_name, args_str)) = self.parse_type_function_call(expr) {
            if let Some(func) = TypeFunction::from_name(func_name) {
                // Parse and expand arguments (respecting balanced parens)
                let args: Vec<&str> = self.split_top_level_args(args_str);

                let mut expanded_args = Vec::new();
                let mut substitution_made = false;

                for arg in &args {
                    // Try to expand the argument
                    if let Ok(n) = parse_i64(arg) {
                        expanded_args.push(n);
                    } else if let Some(def) = self.constants.get(*arg) {
                        if visited.contains(&arg.to_string()) {
                            return Err(format!("Circular reference: {}", arg));
                        }
                        visited.push(arg.to_string());
                        let expanded = self.expand_value(&def.value, steps, visited, depth + 1)?;
                        expanded_args.push(expanded);
                        substitution_made = true;
                    } else {
                        // Try recursive expansion
                        let expanded = self.expand_value(arg, steps, visited, depth + 1)?;
                        expanded_args.push(expanded);
                        substitution_made = true;
                    }
                }

                // Evaluate the function
                let result = func
                    .evaluate(&expanded_args)
                    .ok_or_else(|| format!("Failed to evaluate {}", func.name()))?;

                if substitution_made {
                    let args_display: Vec<String> =
                        expanded_args.iter().map(|n| n.to_string()).collect();
                    let step_expr = format!("{}#({})", func.name(), args_display.join(", "));

                    // Check if this step is redundant
                    let is_redundant = steps
                        .last()
                        .is_some_and(|s| s.expression == step_expr && s.value == Some(result));

                    if !is_redundant {
                        steps.push(ExpansionStep {
                            expression: step_expr,
                            description: Some("evaluated".to_string()),
                            value: Some(result),
                            is_constant_ref: false, // Type function evaluation
                            original_definition: None,
                        });
                    }
                }

                return Ok(result);
            }
        }

        // If nothing else worked, try to parse as number
        expr.parse()
            .map_err(|_| format!("Cannot evaluate: {}", expr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_simple() {
        let source = "#define 42 ANSWER;";
        let evaluator = ConstantEvaluator::from_source(source);

        let result = evaluator.expand("ANSWER").unwrap();
        assert_eq!(result.final_value, 42);
        assert!(result.success);
    }

    #[test]
    fn test_evaluate_tadd() {
        let source = r#"
#define 2 A;
#define 3 B;
#define TAdd#(A, B) SUM;
"#;
        let evaluator = ConstantEvaluator::from_source(source);

        let result = evaluator.expand("SUM").unwrap();
        assert_eq!(result.final_value, 5);
    }

    #[test]
    fn test_evaluate_nested() {
        let source = r#"
#define 2 BASE;
#define TAdd#(BASE, 1) NEXT;
#define TMul#(NEXT, 2) DOUBLE;
"#;
        let evaluator = ConstantEvaluator::from_source(source);

        // NEXT = TAdd#(2, 1) = 3
        assert_eq!(evaluator.expand("NEXT").unwrap().final_value, 3);

        // DOUBLE = TMul#(3, 2) = 6
        assert_eq!(evaluator.expand("DOUBLE").unwrap().final_value, 6);
    }

    #[test]
    fn test_circular_reference() {
        let source = r#"
#define A B;
#define B A;
"#;
        let evaluator = ConstantEvaluator::from_source(source);

        let result = evaluator.expand("A").unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Circular"));
    }

    #[test]
    fn test_undefined_constant() {
        let source = "#define 2 FOO;";
        let evaluator = ConstantEvaluator::from_source(source);

        let result = evaluator.expand("UNDEFINED");
        assert!(result.is_none());
    }

    #[test]
    fn test_hex_value_expansion() {
        let source = "#define 0xFF MAX_VAL;";
        let evaluator = ConstantEvaluator::from_source(source);
        let result = evaluator.expand("MAX_VAL").unwrap();
        assert!(result.success);
        assert_eq!(result.final_value, 255);
    }

    #[test]
    fn test_mixed_hex_and_decimal() {
        let source = r#"
#define 0xFF MAX_VAL;
#define TAdd#(MAX_VAL, 1) OVERFLOW;
"#;
        let evaluator = ConstantEvaluator::from_source(source);
        let result = evaluator.expand("OVERFLOW").unwrap();
        assert!(result.success);
        assert_eq!(result.final_value, 256);
    }

    #[test]
    fn test_tlog_edge_cases() {
        // TLog#(1) = 0 (log2 of 1 is 0)
        assert_eq!(TypeFunction::TLog.evaluate(&[1]), Some(0));
        // TLog#(2) = 1 (log2 of 2 is 1)
        assert_eq!(TypeFunction::TLog.evaluate(&[2]), Some(1));
        // TLog#(256) = 8
        assert_eq!(TypeFunction::TLog.evaluate(&[256]), Some(8));
        // TLog#(255) = 8 (ceiling of log2)
        assert_eq!(TypeFunction::TLog.evaluate(&[255]), Some(8));
        // TLog#(0) should return None
        assert_eq!(TypeFunction::TLog.evaluate(&[0]), None);
    }

    #[test]
    fn test_division_by_zero_handling() {
        // TDiv#(10, 0) should evaluate to None, not panic
        assert_eq!(TypeFunction::TDiv.evaluate(&[10, 0]), None);
    }

    #[test]
    fn test_nested_parens_type_function() {
        let source = r#"
#define 2 A;
#define 3 B;
#define TAdd#(TAdd#(1, A), B) NESTED;
"#;
        let evaluator = ConstantEvaluator::from_source(source);
        let result = evaluator.expand("NESTED").unwrap();
        // TAdd#(TAdd#(1, 2), 3) = TAdd#(3, 3) = 6
        assert_eq!(result.final_value, 6);
        assert!(result.success);
    }
}
