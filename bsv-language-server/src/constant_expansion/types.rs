// Types for Constant Expansion

use lsp_types::Range;
use std::collections::HashMap;

/// A constant definition parsed from #define
#[derive(Debug, Clone, PartialEq)]
pub struct ConstantDef {
    /// The name of the constant (e.g., "FOO")
    pub name: String,

    /// The value or expression (e.g., "2" or "TAdd#(FOO, 1)")
    pub value: String,

    /// The range in the source file where this constant is defined
    pub range: Range,

    /// Whether this is a simple numeric constant or a type function
    pub is_simple: bool,
}

/// A single step in the expansion process
#[derive(Debug, Clone)]
pub struct ExpansionStep {
    /// The expression at this step
    pub expression: String,

    /// Optional description of what happened
    pub description: Option<String>,

    /// The result value after this step (if computable)
    pub value: Option<i64>,

    /// Whether this step is a constant reference (not a numeric literal or operation)
    pub is_constant_ref: bool,

    /// For constant references, the original definition expression (e.g., "TAdd#(BASE_WIDTH, 1)")
    pub original_definition: Option<String>,
}

/// The result of expanding a constant
#[derive(Debug, Clone)]
pub struct ExpansionResult {
    /// The original constant name
    pub name: String,

    /// The final computed value
    pub final_value: i64,

    /// The steps taken to reach the final value
    pub steps: Vec<ExpansionStep>,

    /// Whether the expansion was successful
    pub success: bool,

    /// Error message if expansion failed
    pub error: Option<String>,
}

impl ExpansionResult {
    /// Format the expansion as a trace string for display in hover
    ///
    /// Example output (using ASCII art for tree structure):
    /// ```ignore
    /// BAR = 10
    ///
    /// OFFSET = 7
    /// OFFSET = TSub#(BAR, 3)
    /// ├─ BAR = 10
    /// └─ Result: 10-3 = 7
    ///
    /// TOTAL_WIDTH = 40
    /// TOTAL_WIDTH = TAdd#(ADDR_WIDTH, DATA_WIDTH)
    /// ├─ DATA_WIDTH = 8
    /// ├─ ADDR_WIDTH = 32
    /// └─ Result: 32+8 = 40
    /// ```
    pub fn format_trace(&self) -> String {
        let mut lines = Vec::new();

        if !self.success {
            if let Some(ref error) = self.error {
                return format!("❌ Expansion failed: {}", error);
            }
            return "❌ Expansion failed".to_string();
        }

        // First line: name = final value
        lines.push(format!("{} = {}", self.name, self.final_value));

        // Check if this is a simple numeric constant (first step is just a number)
        let is_simple_numeric = if let Some(first_step) = self.steps.first() {
            first_step
                .expression
                .trim()
                .chars()
                .all(|c| c.is_numeric() || c.is_whitespace())
        } else {
            false
        };

        // For simple numeric constants, just show the value, no need for expansion trace
        if is_simple_numeric && self.steps.len() <= 2 {
            return lines.join("\n");
        }

        // Second line: name = original definition (only if it's not a simple numeric)
        if let Some(first_step) = self.steps.first() {
            let first_expr = first_step.expression.trim();
            // Only show the definition line if it's different from the final value
            // and it's not just a numeric literal
            if first_expr != self.final_value.to_string()
                && !first_expr
                    .chars()
                    .all(|c| c.is_numeric() || c.is_whitespace())
            {
                lines.push(format!("{} = {}", self.name, first_expr));
            }
        }

        // Collect constant reference steps and show with original definitions
        // Reverse to show from outer to inner (the order they were expanded)
        let mut constant_refs: Vec<&ExpansionStep> =
            self.steps.iter().filter(|s| s.is_constant_ref).collect();
        constant_refs.reverse();

        // Show constant reference chain with original definitions
        // All constant refs use ├─ since Result will be the final └─
        for step in &constant_refs {
            let prefix = "├─ ";

            // Extract the constant name from the expression (e.g., "BAR = 10" -> "BAR")
            let const_name = step.expression.split('=').next().unwrap_or("?").trim();

            // Use original definition if available, otherwise use the value from expression
            if let Some(orig_def) = &step.original_definition {
                // Only show if the original definition is different from what we'd show
                let value_str = step.value.map(|v| v.to_string()).unwrap_or_default();
                if orig_def.as_str() != value_str.as_str() {
                    lines.push(format!("{}{} = {}", prefix, const_name, orig_def));
                } else {
                    lines.push(format!("{}{} = {}", prefix, const_name, value_str));
                }
            } else if let Some(val) = step.value {
                lines.push(format!("{}{} = {}", prefix, const_name, val));
            }
        }

        // Build calculation expression from the chain
        let calc_expr = self.build_calculation_expression();

        // Only show Result line if there are actual calculations (not just simple substitutions)
        if !calc_expr.is_empty() && calc_expr != self.final_value.to_string() {
            lines.push(format!("└─ Result: {} = {}", calc_expr, self.final_value));
        } else if !calc_expr.is_empty() {
            // calc_expr equals final_value, just show the result
            lines.push(format!("└─ Result: {}", self.final_value));
        }

        lines.join("\n")
    }

    /// Build a concise calculation expression from the expansion chain
    fn build_calculation_expression(&self) -> String {
        // For simple cases, just show the final value
        if self.steps.len() <= 2 {
            return String::new();
        }

        // Collect all type function operations with their resolved values
        let mut operations = Vec::new();

        for step in &self.steps {
            if step.expression.contains('#') && step.value.is_some() {
                if let Some(op_info) = self.extract_operation_info(step) {
                    operations.push(op_info);
                }
            }
        }

        if operations.is_empty() {
            return String::new();
        }

        // Build the calculation expression based on the operation type
        // For TMax/TMin, show as max(a,b) or min(a,b)
        // For arithmetic ops, build a clean expression
        if operations.len() == 1 {
            // Single operation - show it cleanly
            let op = &operations[0];
            match op.op_type.as_str() {
                "TMax" if op.args.len() >= 2 => {
                    return format!("max({},{})", op.args[0], op.args[1]);
                }
                "TMin" if op.args.len() >= 2 => {
                    return format!("min({},{})", op.args[0], op.args[1]);
                }
                "TAdd" if op.args.len() >= 2 => {
                    return format!("{}+{}", op.args[0], op.args[1]);
                }
                "TSub" if op.args.len() >= 2 => {
                    return format!("{}-{}", op.args[0], op.args[1]);
                }
                "TMul" if op.args.len() >= 2 => {
                    return format!("{}*{}", op.args[0], op.args[1]);
                }
                "TDiv" if op.args.len() >= 2 => {
                    return format!("{}/{}", op.args[0], op.args[1]);
                }
                "TLog" if !op.args.is_empty() => {
                    return format!("log2({})", op.args[0]);
                }
                "TExp" if !op.args.is_empty() => {
                    return format!("2^{}", op.args[0]);
                }
                _ => {}
            }
        }

        // For multiple nested operations, build a chained expression
        // Start from the innermost value and apply operations outward
        let mut expr = operations
            .first()
            .and_then(|op| op.args.first().cloned())
            .unwrap_or_else(|| self.final_value.to_string());

        for op in &operations {
            match op.op_type.as_str() {
                "TAdd" if op.args.len() >= 2 => {
                    expr = format!("({}+{})", expr, op.args[1]);
                }
                "TSub" if op.args.len() >= 2 => {
                    expr = format!("({}-{})", expr, op.args[1]);
                }
                "TMul" if op.args.len() >= 2 => {
                    expr = format!("{}*{}", expr, op.args[1]);
                }
                "TDiv" if op.args.len() >= 2 => {
                    expr = format!("{}/{}", expr, op.args[1]);
                }
                "TMax" if op.args.len() >= 2 => {
                    expr = format!("max({},{})", op.args[0], op.args[1]);
                }
                "TMin" if op.args.len() >= 2 => {
                    expr = format!("min({},{})", op.args[0], op.args[1]);
                }
                _ => {}
            }
        }

        expr
    }

    /// Extract operation information from a step
    fn extract_operation_info(&self, step: &ExpansionStep) -> Option<OperationInfo> {
        let expr = &step.expression;

        // Parse the function name
        let func_name = expr.split('#').next()?;

        // Extract arguments
        let args = self.extract_numeric_args(expr)?;

        Some(OperationInfo {
            op_type: func_name.to_string(),
            args,
        })
    }

    /// Extract numeric arguments from a type function expression
    /// Looks up constant references to get their numeric values
    fn extract_numeric_args(&self, expr: &str) -> Option<Vec<String>> {
        let start = expr.find('(')? + 1;
        let end = expr.find(')')?;
        let args_str = &expr[start..end];

        let args: Vec<&str> = args_str.split(',').map(|s| s.trim()).collect();

        // Build a map of constant names to their values from ALL steps (not just constant_ref)
        let mut const_values: HashMap<&str, String> = HashMap::new();
        for step in &self.steps {
            // Extract constant name and value from any step
            if let Some(val) = step.value {
                // Try to get the constant name from the expression
                if step.is_constant_ref {
                    let parts: Vec<&str> = step.expression.split('=').map(|s| s.trim()).collect();
                    if parts.len() == 2 {
                        const_values.insert(parts[0], val.to_string());
                    }
                }
            }
        }

        let mut numeric_args = Vec::new();
        for arg in &args {
            // Try to parse as number directly
            if let Ok(n) = arg.parse::<i64>() {
                numeric_args.push(n.to_string());
            } else if let Some(value) = const_values.get(arg) {
                // Found in constant reference map
                numeric_args.push(value.clone());
            } else {
                // Use the arg as-is (couldn't resolve)
                numeric_args.push(arg.to_string());
            }
        }

        if numeric_args.is_empty() {
            None
        } else {
            Some(numeric_args)
        }
    }
}

/// Helper struct for operation information
#[derive(Debug, Clone)]
struct OperationInfo {
    op_type: String,
    args: Vec<String>,
}

/// Supported type functions for constant evaluation
// T prefix is the BSV convention (TAdd, TSub, etc.) — keep for clarity
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypeFunction {
    /// TAdd#(a, b) = a + b
    TAdd,

    /// TSub#(a, b) = a - b
    TSub,

    /// TMul#(a, b) = a * b
    TMul,

    /// TDiv#(a, b) = a / b
    TDiv,

    /// TLog#(n) = log2(n) (ceiling)
    TLog,

    /// TExp#(n) = 2^n
    TExp,

    /// TMax#(a, b) = max(a, b)
    TMax,

    /// TMin#(a, b) = min(a, b)
    TMin,
}

impl TypeFunction {
    /// Parse a type function name
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "TAdd" => Some(Self::TAdd),
            "TSub" => Some(Self::TSub),
            "TMul" => Some(Self::TMul),
            "TDiv" => Some(Self::TDiv),
            "TLog" => Some(Self::TLog),
            "TExp" => Some(Self::TExp),
            "TMax" => Some(Self::TMax),
            "TMin" => Some(Self::TMin),
            _ => None,
        }
    }

    /// Evaluate the type function with given arguments
    pub fn evaluate(&self, args: &[i64]) -> Option<i64> {
        match self {
            Self::TAdd => {
                if args.len() == 2 {
                    Some(args[0] + args[1])
                } else {
                    None
                }
            }
            Self::TSub => {
                if args.len() == 2 {
                    Some(args[0] - args[1])
                } else {
                    None
                }
            }
            Self::TMul => {
                if args.len() == 2 {
                    Some(args[0] * args[1])
                } else {
                    None
                }
            }
            Self::TDiv => {
                if args.len() == 2 && args[1] != 0 {
                    Some(args[0] / args[1])
                } else {
                    None
                }
            }
            Self::TLog => {
                if args.len() == 1 && args[0] > 0 {
                    // Ceiling of log2
                    let n = args[0] as u64;
                    let log = (64 - n.leading_zeros()) as i64;
                    // Adjust for exact powers of 2
                    if n.is_power_of_two() {
                        Some(log - 1)
                    } else {
                        Some(log)
                    }
                } else {
                    None
                }
            }
            Self::TExp => {
                if args.len() == 1 && args[0] >= 0 {
                    2i64.checked_pow(args[0] as u32)
                } else {
                    None
                }
            }
            Self::TMax => {
                if args.len() == 2 {
                    Some(args[0].max(args[1]))
                } else {
                    None
                }
            }
            Self::TMin => {
                if args.len() == 2 {
                    Some(args[0].min(args[1]))
                } else {
                    None
                }
            }
        }
    }

    /// Get the name of this type function
    pub fn name(&self) -> &'static str {
        match self {
            Self::TAdd => "TAdd",
            Self::TSub => "TSub",
            Self::TMul => "TMul",
            Self::TDiv => "TDiv",
            Self::TLog => "TLog",
            Self::TExp => "TExp",
            Self::TMax => "TMax",
            Self::TMin => "TMin",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_function_evaluate() {
        assert_eq!(TypeFunction::TAdd.evaluate(&[2, 3]), Some(5));
        assert_eq!(TypeFunction::TSub.evaluate(&[10, 3]), Some(7));
        assert_eq!(TypeFunction::TMul.evaluate(&[4, 5]), Some(20));
        assert_eq!(TypeFunction::TDiv.evaluate(&[20, 4]), Some(5));
        assert_eq!(TypeFunction::TLog.evaluate(&[256]), Some(8));
        assert_eq!(TypeFunction::TExp.evaluate(&[3]), Some(8));
        assert_eq!(TypeFunction::TMax.evaluate(&[5, 10]), Some(10));
        assert_eq!(TypeFunction::TMin.evaluate(&[5, 10]), Some(5));
    }

    #[test]
    fn test_expansion_result_format() {
        let result = ExpansionResult {
            name: "BAR".to_string(),
            final_value: 3,
            steps: vec![
                ExpansionStep {
                    expression: "TAdd#(FOO, 1)".to_string(),
                    description: None,
                    value: None,
                    is_constant_ref: false,
                    original_definition: None,
                },
                ExpansionStep {
                    expression: "FOO".to_string(),
                    description: Some("substituted".to_string()),
                    value: Some(2),
                    is_constant_ref: true,
                    original_definition: Some("2".to_string()),
                },
                ExpansionStep {
                    expression: "TAdd#(2, 1)".to_string(),
                    description: Some("evaluated".to_string()),
                    value: Some(3),
                    is_constant_ref: false,
                    original_definition: None,
                },
            ],
            success: true,
            error: None,
        };

        let trace = result.format_trace();
        assert!(trace.contains("BAR"));
        assert!(trace.contains("3"));
    }
}
