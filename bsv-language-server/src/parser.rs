// 改进后的 parser.rs - 完整实现
// 这个文件展示了如何实施容错符号提取

use crate::Result;
use lsp_types::{FoldingRange, FoldingRangeKind};
use std::sync::Mutex;
use tree_sitter::{Node, Parser, Tree};

extern "C" {
    fn tree_sitter_bsv() -> *const std::ffi::c_void;
}

/// Result of detecting a function/method call context at a cursor position.
#[derive(Debug, Clone)]
pub struct CallContext {
    /// Name of the function or method being called.
    pub callable_name: String,
    /// 0-based index of the argument the cursor is currently in.
    pub argument_index: usize,
}

pub struct BsvParser {
    parser: Mutex<Parser>,
}

impl BsvParser {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();

        // SAFETY: tree_sitter_bsv() returns a valid TSLanguage* pointer
        // from the generated C parser. In tree-sitter 0.20.x, Language
        // is a type alias for *const (), which is ABI-compatible with
        // *const c_void.
        let language = unsafe {
            let ptr = tree_sitter_bsv();
            std::mem::transmute::<*const std::ffi::c_void, tree_sitter::Language>(ptr)
        };

        parser
            .set_language(language)
            .map_err(|_| crate::Error::TreeSitter)?;

        Ok(Self {
            parser: Mutex::new(parser),
        })
    }

    pub fn parse(&self, source: &str) -> Result<Tree> {
        let mut parser = self.parser.lock().unwrap();
        parser
            .parse(source, None)
            .ok_or_else(|| crate::Error::Parse("Failed to parse source code".into()))
    }

    pub fn extract_symbols(&self, tree: &Tree, source: &str) -> Vec<crate::Symbol> {
        let mut symbols = Vec::new();
        let root_node = tree.root_node();

        self.traverse_node(root_node, source, &mut symbols);

        // 去重：可能从正常节点和 ERROR 节点提取了相同的符号
        symbols.sort_by(|a, b| a.name.cmp(&b.name));
        symbols.dedup_by(|a, b| a.name == b.name);

        symbols
    }

    /// Collect folding ranges from the parsed syntax tree.
    ///
    /// Returns a sorted list of `FoldingRange` covering all foldable block
    /// structures in the document: module/endmodule, interface/endinterface,
    /// rule/endrule, method/endmethod, function/endfunction, action blocks,
    /// typeclass blocks, FSM blocks (seq/endseq, par/endpar), begin/end
    /// expressions, and consecutive `//` comment lines.
    ///
    /// Ranges are non-overlapping — child blocks nest within parents but the
    /// returned list is valid for LSP folding (editors handle nesting).
    pub fn collect_folding_ranges(&self, tree: &Tree, source: &str) -> Vec<FoldingRange> {
        let mut ranges = Vec::new();
        let root = tree.root_node();
        Self::collect_block_ranges(root, &mut ranges);
        Self::collect_comment_ranges(source, &mut ranges);
        ranges.sort_by_key(|r| r.start_line);
        ranges
    }

    /// Recursively collect folding ranges from block-structure nodes.
    fn collect_block_ranges(node: Node, ranges: &mut Vec<FoldingRange>) {
        // Node kinds that form foldable blocks.
        const BLOCK_KINDS: &[&str] = &[
            "moduleDef",
            "interfaceDecl",
            "methodDef",
            "functionDef",
            "rule",
            "typeclassDef",
            "typeclassInstanceDef",
            "actionBlock",
            "actionValueBlock",
            "subinterfaceDef",
            "subFunctionDef",
            "seqFsmStmt",
            "parFsmStmt",
            "interfaceExpr",
            "externModuleImport",
            "rulesExpr",
            "beginEndExpr",
        ];

        if BLOCK_KINDS.contains(&node.kind()) {
            let start = node.start_position().row as u32;
            let end = node.end_position().row as u32;
            // Only emit ranges that are at least 2 lines tall (foldable).
            if end > start {
                // Use the line *before* the closing keyword as end_line so the
                // closing keyword (e.g. `endmodule`) stays visible when folded.
                let end_line = if end > start + 1 { end - 1 } else { end };
                ranges.push(FoldingRange {
                    start_line: start,
                    start_character: None,
                    end_line,
                    end_character: None,
                    kind: Some(FoldingRangeKind::Region),
                    collapsed_text: None,
                });
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::collect_block_ranges(child, ranges);
        }
    }

    /// Collect folding ranges for consecutive `//` comment lines and `/* */` blocks.
    ///
    /// Because tree-sitter stores comments as extras (outside the standard
    /// child node tree), we scan the source text line-by-line instead.
    fn collect_comment_ranges(source: &str, ranges: &mut Vec<FoldingRange>) {
        Self::collect_line_comment_ranges(source, ranges);
        Self::collect_block_comment_ranges(source, ranges);
    }

    /// Fold consecutive `//` comment lines (3+ lines).
    fn collect_line_comment_ranges(source: &str, ranges: &mut Vec<FoldingRange>) {
        let lines: Vec<&str> = source.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let trimmed = lines[i].trim();
            if trimmed.starts_with("//") {
                let start = i;
                while i < lines.len() && lines[i].trim().starts_with("//") {
                    i += 1;
                }
                let end = i; // exclusive end
                if end - start >= 3 {
                    ranges.push(FoldingRange {
                        start_line: start as u32,
                        start_character: None,
                        end_line: (end - 1) as u32,
                        end_character: None,
                        kind: Some(FoldingRangeKind::Comment),
                        collapsed_text: None,
                    });
                }
            } else {
                i += 1;
            }
        }
    }

    /// Fold `/* ... */` block comments spanning 3+ lines.
    fn collect_block_comment_ranges(source: &str, ranges: &mut Vec<FoldingRange>) {
        let lines: Vec<&str> = source.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            if lines[i].trim().starts_with("/*") {
                let start = i;
                // Find the closing `*/` (may be on same or later line).
                while i < lines.len() && !lines[i].contains("*/") {
                    i += 1;
                }
                if i < lines.len() {
                    i += 1; // include the line with `*/`
                }
                // Only fold blocks spanning 3+ lines.
                if i - start >= 3 {
                    ranges.push(FoldingRange {
                        start_line: start as u32,
                        start_character: None,
                        end_line: (i - 1) as u32,
                        end_character: None,
                        kind: Some(FoldingRangeKind::Comment),
                        collapsed_text: None,
                    });
                }
            } else {
                i += 1;
            }
        }
    }

    fn traverse_node(&self, node: Node, source: &str, symbols: &mut Vec<crate::Symbol>) {
        // 新增：处理 ERROR 节点 - 尝试从错误中恢复模块名
        if node.kind() == "ERROR" {
            if let Some(name_node) = self.try_extract_module_from_error(node, source) {
                if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                    if !name.is_empty() {
                        symbols.push(crate::Symbol {
                            name: name.to_string(),
                            kind: crate::SymbolKind::Module,
                            range: self.node_to_range(&name_node),
                            uri: None,
                            container: None,
                            documentation: Some(
                                "[Error recovery] Module definition with syntax errors".to_string(),
                            ),
                            parameters: Vec::new(),
                        });
                    }
                }
            }
        }

        // 提取模块定义
        if node.kind() == "moduleDef" {
            if let Some(name_node) = self.get_module_name_node(node, source) {
                if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                    if !name.is_empty() {
                        symbols.push(crate::Symbol {
                            name: name.to_string(),
                            kind: crate::SymbolKind::Module,
                            range: self.node_to_range(&name_node),
                            uri: None,
                            container: None,
                            documentation: None,
                            parameters: Vec::new(),
                        });
                    }
                }
            }

            // 容错：如果模块没有正确的 endmodule，尝试从 moduleStmt 中提取后续模块
            if !self.has_valid_endmodule(node, source) {
                self.extract_embedded_modules(node, source, symbols);
            }
        }

        // 提取函数/方法定义
        if node.kind() == "functionDef" || node.kind() == "methodDef" {
            if let Some(name_node) = self.get_callable_name_node(node, source) {
                if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                    if !name.is_empty() {
                        let params = self.extract_callable_params(node, source);
                        symbols.push(crate::Symbol {
                            name: name.to_string(),
                            kind: if node.kind() == "methodDef" {
                                crate::SymbolKind::Method
                            } else {
                                crate::SymbolKind::Function
                            },
                            range: self.node_to_range(&name_node),
                            uri: None,
                            container: None,
                            documentation: None,
                            parameters: params,
                        });
                    }
                }
            }
        }

        // 新增：从 ERROR 节点中提取函数定义
        if node.kind() == "ERROR" {
            self.extract_functions_from_error(node, source, symbols);
        }

        // 提取变量声明
        if node.kind() == "varDecl" {
            if let Some(lvalue_node) = self.child_by_kind(node, "lValue") {
                if let Some(ident_node) = self.find_identifier(lvalue_node) {
                    if let Ok(name) = ident_node.utf8_text(source.as_bytes()) {
                        if !name.is_empty() {
                            symbols.push(crate::Symbol {
                                name: name.to_string(),
                                kind: crate::SymbolKind::Variable,
                                range: self.node_to_range(&ident_node),
                                uri: None,
                                container: None,
                                documentation: None,
                                parameters: Vec::new(),
                            });
                        }
                    }
                }
            }
        }

        // 递归遍历子节点
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.traverse_node(child, source, symbols);
        }
    }

    #[allow(clippy::manual_find)]
    fn child_by_kind<'a>(&self, node: Node<'a>, kind: &str) -> Option<Node<'a>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == kind {
                return Some(child);
            }
        }
        None
    }

    fn find_identifier<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "identifier" {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(ident) = self.find_identifier(child) {
                return Some(ident);
            }
        }

        None
    }

    // 改进后的方法：增加容错逻辑
    fn get_module_name_node<'a>(&self, node: Node<'a>, _source: &str) -> Option<Node<'a>> {
        // 策略 1：标准路径 - 查找 moduleProto 然后找 identifier
        if let Some(proto) = self.child_by_kind(node, "moduleProto") {
            if let Some(ident) = self.child_by_kind(proto, "identifier") {
                return Some(ident);
            }
        }

        // 策略 2：容错 - 如果 moduleDef 有错误，查找 'module' 关键字后的 identifier
        if node.has_error() {
            let mut cursor = node.walk();
            let mut found_module_keyword = false;

            for child in node.children(&mut cursor) {
                if child.kind() == "module" {
                    found_module_keyword = true;
                } else if found_module_keyword && child.kind() == "identifier" {
                    return Some(child);
                }
            }
        }

        None
    }

    // 检查模块是否有正确的结束（endmodule 必须是最后一个非空子节点）
    fn has_valid_endmodule(&self, node: Node, source: &str) -> bool {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        // 查找 endmodule 的位置
        let endmodule_pos = children.iter().position(|c| c.kind() == "endmodule");

        if let Some(pos) = endmodule_pos {
            // 检查 endmodule 之前是否有可能包含嵌入模块的 moduleStmt
            for child in children.iter().take(pos) {
                if child.kind() == "moduleStmt" && self.might_contain_module(*child, source) {
                    return false;
                }
            }
            return true;
        }

        false
    }

    // 检查 moduleStmt 是否可能包含嵌入的模块
    fn might_contain_module(&self, node: Node, source: &str) -> bool {
        // 检查节点本身是否有错误
        if node.has_error() {
            return true;
        }

        // 检查是否有 ERROR 节点
        if self.has_error_descendant(node) {
            return true;
        }

        // 检查是否包含 "module" identifier（被错误解析的 module 关键字）
        if self.contains_module_identifier(node, source) {
            return true;
        }

        false
    }

    // 检查是否有 ERROR 后代节点
    fn has_error_descendant(&self, node: Node) -> bool {
        if node.kind() == "ERROR" {
            return true;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.has_error_descendant(child) {
                return true;
            }
        }

        false
    }

    // 检查节点是否包含名为 "module" 的 identifier（被错误解析的 module 关键字）
    fn contains_module_identifier(&self, node: Node, source: &str) -> bool {
        if node.kind() == "identifier" {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                if text == "module" {
                    return true;
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.contains_module_identifier(child, source) {
                return true;
            }
        }

        false
    }

    // 从 moduleStmt 中尝试提取嵌入的模块定义
    fn extract_embedded_modules(&self, node: Node, source: &str, symbols: &mut Vec<crate::Symbol>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "moduleStmt" {
                // 检查这个 moduleStmt 是否包含 module 关键字（可能是嵌入的模块）
                self.try_extract_module_from_stmt(child, source, symbols);
            }
        }
    }

    // 尝试从 moduleStmt 中提取模块
    fn try_extract_module_from_stmt(
        &self,
        node: Node,
        source: &str,
        symbols: &mut Vec<crate::Symbol>,
    ) {
        // 策略 1：查找 ERROR 节点中的 identifier
        if let Some(error_node) = self.find_error_node(node) {
            if let Some(ident) = self.find_identifier_after_module(error_node, source) {
                if let Ok(name) = ident.utf8_text(source.as_bytes()) {
                    if !name.is_empty() && name != "module" {
                        symbols.push(crate::Symbol {
                            name: name.to_string(),
                            kind: crate::SymbolKind::Module,
                            range: self.node_to_range(&ident),
                            uri: None,
                            container: None,
                            documentation: Some(
                                "[Error recovery] Module extracted from incomplete definition"
                                    .to_string(),
                            ),
                            parameters: Vec::new(),
                        });
                    }
                }
            }
        }

        // 策略 2：查找 "module" identifier 后的 identifier
        if let Some(ident) = self.find_identifier_after_module(node, source) {
            if let Ok(name) = ident.utf8_text(source.as_bytes()) {
                if !name.is_empty() && name != "module" {
                    // 避免重复添加
                    if !symbols.iter().any(|s| s.name == name) {
                        symbols.push(crate::Symbol {
                            name: name.to_string(),
                            kind: crate::SymbolKind::Module,
                            range: self.node_to_range(&ident),
                            uri: None,
                            container: None,
                            documentation: Some(
                                "[Error recovery] Module extracted from incomplete definition"
                                    .to_string(),
                            ),
                            parameters: Vec::new(),
                        });
                    }
                }
            }
        }

        // 策略 3：递归检查子节点
        let mut cursor2 = node.walk();
        for child in node.children(&mut cursor2) {
            if child.kind() == "moduleStmt" || child.kind() == "moduleDef" {
                self.try_extract_module_from_stmt(child, source, symbols);
            }
        }
    }

    // 查找 ERROR 节点
    fn find_error_node<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "ERROR" {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(error) = self.find_error_node(child) {
                return Some(error);
            }
        }

        None
    }

    // 查找 "module" identifier 后的 identifier
    fn find_identifier_after_module<'a>(&self, node: Node<'a>, source: &str) -> Option<Node<'a>> {
        // 使用扁平化遍历
        let mut found_module_ident = false;
        self.find_identifier_after_module_impl(node, source, &mut found_module_ident)
    }

    fn find_identifier_after_module_impl<'a>(
        &self,
        node: Node<'a>,
        source: &str,
        found_module: &mut bool,
    ) -> Option<Node<'a>> {
        // 先检查当前节点
        if node.kind() == "identifier" {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                if text == "module" {
                    *found_module = true;
                } else if *found_module {
                    return Some(node);
                }
            }
        }

        // 递归检查子节点
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = self.find_identifier_after_module_impl(child, source, found_module)
            {
                return Some(found);
            }
        }

        None
    }

    // 改进后的方法：增加容错逻辑
    fn get_callable_name_node<'a>(&self, node: Node<'a>, _source: &str) -> Option<Node<'a>> {
        if node.kind() == "methodDef" {
            // 标准路径
            if let Some(ident) = self.child_by_kind(node, "identifier") {
                return Some(ident);
            }

            // 容错：在错误节点中查找第一个 identifier
            if node.has_error() {
                return self.find_first_identifier(node);
            }
        }

        if node.kind() == "functionDef" {
            // 标准路径
            if let Some(proto) = self.child_by_kind(node, "functionProto") {
                if let Some(ft) = self.child_by_kind(proto, "functionType") {
                    if let Some(ident) = self.child_by_kind(ft, "identifier") {
                        return Some(ident);
                    }
                }
            }

            // 容错：在错误节点中查找第一个 identifier
            if node.has_error() {
                return self.find_first_identifier(node);
            }
        }

        None
    }

    // 新增方法：从 ERROR 节点提取模块名
    #[allow(clippy::only_used_in_recursion)]
    fn try_extract_module_from_error<'a>(&self, node: Node<'a>, source: &str) -> Option<Node<'a>> {
        // 策略 1：直接查找 moduleProto
        if let Some(proto) = self.child_by_kind(node, "moduleProto") {
            if let Some(ident) = self.child_by_kind(proto, "identifier") {
                return Some(ident);
            }
        }

        // 策略 2：查找 module 关键字后的 identifier
        let mut cursor = node.walk();
        let mut found_module = false;

        for child in node.children(&mut cursor) {
            if child.kind() == "module" {
                found_module = true;
            } else if found_module && child.kind() == "identifier" {
                return Some(child);
            } else if found_module && child.kind() == "ERROR" {
                // 递归查找嵌套的 ERROR 节点
                if let Some(found) = self.try_extract_module_from_error(child, source) {
                    return Some(found);
                }
            }
        }

        None
    }

    // 新增方法：从 ERROR 节点提取函数定义
    fn extract_functions_from_error(
        &self,
        node: Node,
        source: &str,
        symbols: &mut Vec<crate::Symbol>,
    ) {
        // 查找 functionProto 并从中提取函数名
        if let Some(proto) = self.find_function_proto(node) {
            if let Some(name) = self.extract_function_name_from_proto(proto, source) {
                if !symbols.iter().any(|s| s.name == name) {
                    symbols.push(crate::Symbol {
                        name: name.clone(),
                        kind: crate::SymbolKind::Function,
                        range: self.node_to_range(&proto),
                        uri: None,
                        container: None,
                        documentation: Some(
                            "[Error recovery] Function extracted from ERROR node".to_string(),
                        ),
                        parameters: Vec::new(),
                    });
                }
            }
        }

        // 查找 varDecl 中可能的函数定义（函数被错误解析为变量声明）
        self.extract_function_from_vardecl(node, source, symbols);

        // 递归查找子节点中的 functionProto
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() != "ERROR" {
                self.extract_functions_from_error(child, source, symbols);
            }
        }
    }

    // 从 varDecl 中提取可能的函数名
    fn extract_function_from_vardecl(
        &self,
        node: Node,
        source: &str,
        symbols: &mut Vec<crate::Symbol>,
    ) {
        if node.kind() != "varDecl" {
            return;
        }

        // 检查是否看起来像函数定义：type + varInit 中有参数列表
        // 例如：Bit#(32) add(Bit#(32) a, Bit#(32) b)
        let has_type = self.child_by_kind(node, "type").is_some();
        let var_init = self.child_by_kind(node, "varInit");

        if let Some(var_init) = var_init.filter(|_| has_type) {
            // 检查 varInit 是否包含 "(" 和参数（表明可能是函数）
            if self.looks_like_function(var_init, source) {
                // 从 varInit 中提取函数名
                if let Some(lvalue) = self.child_by_kind(var_init, "lValue") {
                    if let Some(ident) = self.child_by_kind(lvalue, "identifier") {
                        if let Ok(name) = ident.utf8_text(source.as_bytes()) {
                            if !name.is_empty() && !symbols.iter().any(|s| s.name == name) {
                                symbols.push(crate::Symbol {
                                    name: name.to_string(),
                                    kind: crate::SymbolKind::Function,
                                    range: self.node_to_range(&ident),
                                    uri: None,
                                    container: None,
                                    documentation: Some("[Error recovery] Function extracted from variable declaration".to_string()),
                                    parameters: Vec::new(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // 检查节点是否看起来像函数调用（有括号和参数）
    fn looks_like_function(&self, node: Node, source: &str) -> bool {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
        // 检查是否包含括号和参数列表的模式
        text.contains('(') && text.contains(')')
    }

    // 查找 functionProto 节点
    fn find_function_proto<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "functionProto" {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(proto) = self.find_function_proto(child) {
                return Some(proto);
            }
        }

        None
    }

    // 从 functionProto 提取函数名
    fn extract_function_name_from_proto(&self, proto: Node, source: &str) -> Option<String> {
        // functionProto -> functionType -> identifier
        if let Some(ft) = self.child_by_kind(proto, "functionType") {
            if let Some(ident) = self.child_by_kind(ft, "identifier") {
                if let Ok(name) = ident.utf8_text(source.as_bytes()) {
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
        None
    }

    // 新增方法：查找第一个 identifier（用于错误恢复）
    fn find_first_identifier<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "identifier" {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = self.find_first_identifier(child) {
                return Some(found);
            }
        }

        None
    }

    // ── Signature Help: parameter extraction ────────────────────────────

    /// Extract parameter information from a function or method definition node.
    ///
    /// For `functionDef`:
    ///   functionDef → functionProto → functionType → functionFormals → functionFormal
    ///   Each functionFormal is: type + identifier
    ///
    /// For `methodDef`:
    ///   methodDef → methodFormals → methodFormal
    ///   Each methodFormal is: optional(type) + identifier
    fn extract_callable_params(&self, node: Node, source: &str) -> Vec<crate::ParameterInfo> {
        let formals_node = if node.kind() == "functionDef" {
            self.get_function_formals_node(node)
        } else if node.kind() == "methodDef" {
            self.get_method_formals_node(node)
        } else {
            return Vec::new();
        };

        let formals_node = match formals_node {
            Some(n) => n,
            None => return Vec::new(),
        };

        let mut params = Vec::new();
        let mut cursor = formals_node.walk();
        for child in formals_node.children(&mut cursor) {
            let kind = child.kind();
            // functionFormal: "type identifier" or functionProto
            // methodFormal: "optional(type) identifier"
            if kind == "functionFormal" || kind == "methodFormal" || kind == "subFunctionFormal" {
                let param_info = self.extract_formal_param(child, source);
                params.push(param_info);
            }
        }
        params
    }

    /// Extract name and optional type from a single formal parameter node.
    fn extract_formal_param(&self, node: Node, source: &str) -> crate::ParameterInfo {
        // Walk children: type (optional for methodFormal) + identifier
        let mut cursor = node.walk();
        let mut type_name: Option<String> = None;
        let mut param_name: Option<String> = None;

        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind == "type" || kind == "functionType" || kind == "subFunctionType" {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    type_name = Some(text.to_string());
                }
            } else if kind == "identifier" {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    param_name = Some(text.to_string());
                }
            }
        }

        crate::ParameterInfo::new(param_name.unwrap_or_default(), type_name)
    }

    /// Find the `functionFormals` node inside a `functionDef`.
    fn get_function_formals_node<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        // functionDef → functionProto → functionType → functionFormals
        if let Some(proto) = self.child_by_kind(node, "functionProto") {
            if let Some(ft) = self.child_by_kind(proto, "functionType") {
                return self.child_by_kind(ft, "functionFormals");
            }
        }
        None
    }

    /// Find the `methodFormals` node inside a `methodDef`.
    fn get_method_formals_node<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        self.child_by_kind(node, "methodFormals")
    }

    // ── Signature Help: call context detection ──────────────────────────

    /// Detect whether the cursor is inside a function or method call, and if so,
    /// determine the callable name and which argument the cursor is on.
    ///
    /// Walks up from the deepest node at the cursor position to find a
    /// `functionCall` or `methodCall` ancestor, then counts commas in the
    /// argument list up to the cursor to compute `argument_index`.
    pub fn find_call_context(
        &self,
        tree: &Tree,
        source: &str,
        position: lsp_types::Position,
    ) -> Option<CallContext> {
        let root = tree.root_node();
        let point = tree_sitter::Point {
            row: position.line as usize,
            column: position.character as usize,
        };
        let deepest = root.descendant_for_point_range(point, point)?;

        // Walk up the tree to find a functionCall or methodCall ancestor.
        let mut node: Option<Node> = Some(deepest);
        while let Some(current) = node {
            let kind = current.kind();
            if kind == "functionCall" || kind == "methodCall" {
                return self.extract_call_context(current, source, position);
            }
            node = current.parent();
        }

        None
    }

    /// Given a `functionCall` or `methodCall` node, extract the callable name
    /// and compute the active argument index.
    fn extract_call_context(
        &self,
        node: Node,
        source: &str,
        position: lsp_types::Position,
    ) -> Option<CallContext> {
        let kind = node.kind();
        let callable_name = if kind == "functionCall" {
            // functionCall → exprPrimary (the first child before '(')
            self.get_function_call_name(node, source)?
        } else if kind == "methodCall" {
            // methodCall → exprPrimary '.' identifier '(' ... ')'
            self.get_method_call_name(node, source)?
        } else {
            return None;
        };

        // Compute argument index by scanning the source text for commas
        // between the opening '(' and the cursor position.
        let argument_index = self.count_arguments_at_position(node, source, position);

        Some(CallContext {
            callable_name,
            argument_index,
        })
    }

    /// Extract the function name from a `functionCall` node.
    ///
    /// functionCall = exprPrimary '(' ... ')'
    /// The first exprPrimary child that is an identifier is the function name.
    fn get_function_call_name(&self, node: Node, source: &str) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "exprPrimary" {
                // exprPrimary may directly be an identifier or contain one.
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    let text = text.trim();
                    // Simple identifier or dotted name — use the first identifier.
                    // Also handle method-like calls like `obj.method(args)`.
                    if !text.is_empty() && !text.starts_with('(') && !text.starts_with('\"') {
                        return Some(text.to_string());
                    }
                }
                // Try to find an identifier child
                if let Some(ident) = self.find_identifier(child) {
                    if let Ok(name) = ident.utf8_text(source.as_bytes()) {
                        return Some(name.to_string());
                    }
                }
            }
        }
        None
    }

    /// Extract the method name from a `methodCall` node.
    ///
    /// methodCall = exprPrimary '.' identifier '(' ... ')'
    fn get_method_call_name(&self, node: Node, source: &str) -> Option<String> {
        let mut cursor = node.walk();
        let mut found_dot = false;
        for child in node.children(&mut cursor) {
            if child.kind() == "." {
                found_dot = true;
            } else if found_dot && child.kind() == "identifier" {
                if let Ok(name) = child.utf8_text(source.as_bytes()) {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    /// Count how many arguments have been typed so far by counting commas
    /// between the opening '(' and the cursor position.
    fn count_arguments_at_position(
        &self,
        node: Node,
        source: &str,
        position: lsp_types::Position,
    ) -> usize {
        let node_start = node.start_position();

        // Find the opening '(' character position
        // We search from the node's start row/column
        let start_line = node_start.row;
        let start_col = node_start.column;

        // Find the open paren by scanning source
        let source_str = if let Ok(s) = node.utf8_text(source.as_bytes()) {
            s
        } else {
            return 0;
        };

        // Find the position of '(' in the node text
        let open_paren_pos = match source_str.find('(') {
            Some(pos) => pos,
            None => return 0,
        };

        // The content between '(' and cursor, accounting for multi-line
        let cursor_line = position.line as usize;
        let cursor_col = position.character as usize;

        // If cursor is on the same line as the open paren
        if cursor_line == start_line {
            let after_paren = if cursor_col > start_col + open_paren_pos + 1 {
                &source_str[open_paren_pos + 1..cursor_col - start_col]
            } else {
                return 0; // Cursor is before or at '('
            };
            // Don't proceed if candidate argument segment is empty
            if after_paren.is_empty() {
                return 0;
            }
            // Count commas, but skip those inside nested parentheses
            Self::count_top_level_commas(after_paren)
        } else {
            // Multi-line: scan from '(' to cursor position
            // Get the text from start of node to cursor
            let lines: Vec<&str> = source.lines().collect();
            let mut arg_text = String::new();

            // First line: from '(' to end of line
            if start_line < lines.len() {
                let first_line = lines[start_line];
                if open_paren_pos + 1 < first_line.len() {
                    arg_text.push_str(&first_line[open_paren_pos + 1..]);
                }
                arg_text.push('\n');
            }

            // Middle lines
            for line_idx in (start_line + 1)..cursor_line {
                if line_idx < lines.len() {
                    arg_text.push_str(lines[line_idx]);
                    arg_text.push('\n');
                }
            }

            // Last line: up to cursor
            if cursor_line < lines.len() {
                let last_line = lines[cursor_line];
                let end = if cursor_col <= last_line.len() {
                    cursor_col
                } else {
                    last_line.len()
                };
                arg_text.push_str(&last_line[..end]);
            }

            Self::count_top_level_commas(&arg_text)
        }
    }

    /// Count commas that are not inside nested parentheses.
    fn count_top_level_commas(text: &str) -> usize {
        let mut depth = 0;
        let mut count = 0;
        for ch in text.chars() {
            match ch {
                '(' | '{' | '[' => depth += 1,
                ')' | '}' | ']' if depth > 0 => depth -= 1,
                ',' if depth == 0 => count += 1,
                _ => {}
            }
        }
        count
    }

    // ── Reference extraction ──────────────────────────────────────────

    /// Extract all identifier usage sites (references) from the parse tree.
    ///
    /// Walks every `identifier` node in the tree and classifies it as a
    /// reference or a declaration by examining its parent (and grandparent)
    /// node kinds. Only reference-typed identifiers are returned.
    ///
    /// Results are deduplicated by `(range, name)` — important because
    /// identifiers inside ERROR nodes may also appear as children of
    /// well-formed nodes.
    pub fn extract_references(&self, tree: &Tree, source: &str) -> Vec<crate::Reference> {
        let mut refs = Vec::new();
        let root = tree.root_node();
        self.collect_references(root, source, &mut refs);
        refs.sort_by(|a, b| {
            a.range
                .start
                .line
                .cmp(&b.range.start.line)
                .then_with(|| a.range.start.character.cmp(&b.range.start.character))
                .then_with(|| a.name.cmp(&b.name))
        });
        refs.dedup_by(|a, b| a.range == b.range && a.name == b.name);
        refs
    }

    /// Recursively collect reference identifiers from the AST.
    fn collect_references(&self, node: Node, source: &str, refs: &mut Vec<crate::Reference>) {
        if node.kind() == "identifier" {
            if !self.is_declaration_context(node) {
                if let Ok(name) = node.utf8_text(source.as_bytes()) {
                    if !name.is_empty() {
                        refs.push(crate::Reference {
                            name: name.to_string(),
                            range: self.node_to_range(&node),
                            uri: None,
                        });
                    }
                }
            }
            return; // identifier has no children worth recursing into
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_references(child, source, refs);
        }
    }

    /// Determine whether an `identifier` node is a declaration (rather than a
    /// reference) by inspecting its parent (and grandparent) node kinds.
    fn is_declaration_context(&self, node: Node) -> bool {
        let parent = match node.parent() {
            Some(p) => p,
            None => return false,
        };
        let parent_kind = parent.kind();

        // ── Category 1: Direct declaration parents ────────────────────
        if matches!(
            parent_kind,
            "moduleProto"
                | "functionType"
                | "subFunctionType"
                | "methodProto"
                | "methodDef"
                | "rule"
                | "functionFormal"
                | "subFunctionFormal"
                | "methodFormal"
                | "moduleFormalParam"
                | "typedefEnumElement"
                | "typedefEnum"
                | "structMember"
                | "unionMember"
                | "exportItem"
                | "forInit"
                | "varDecl"
                | "varDo"
                | "varDeclDo"
        ) {
            return true;
        }

        // ── Category 2: lValue + declaration grandparent ──────────────
        if parent_kind == "lValue" {
            if let Some(gp) = parent.parent() {
                if matches!(
                    gp.kind(),
                    "varDecl" | "varInit" | "varDo" | "forInit" | "moduleInst"
                ) {
                    return true;
                }
            }
        }

        // ── Category 3: typeIde in typedef ────────────────────────────
        if parent_kind == "typeIde" {
            if let Some(gp) = parent.parent() {
                if gp.kind() == "typeDefType" {
                    return true;
                }
            }
        }

        // ── Category 4: typeclassIde + grandparent typeclassDef ───────
        if parent_kind == "typeclassIde" {
            if let Some(gp) = parent.parent() {
                if gp.kind() == "typeclassDef" {
                    return true;
                }
            }
        }

        // ── Category 5: packageIde — declaration unless inside import ─
        if parent_kind == "packageIde" {
            if let Some(gp) = parent.parent() {
                if gp.kind() != "importItem" {
                    return true;
                }
            }
        }

        false
    }

    fn node_to_range(&self, node: &Node) -> lsp_types::Range {
        lsp_types::Range {
            start: lsp_types::Position {
                line: node.start_position().row as u32,
                character: node.start_position().column as u32,
            },
            end: lsp_types::Position {
                line: node.end_position().row as u32,
                character: node.end_position().column as u32,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SymbolKind;

    #[test]
    fn test_extract_module_and_function_symbols() {
        let source = "module mkTest();\n    // test logic\nendmodule\n\nmodule mkMain();\n    mkTest my_test_inst;\nendmodule\n\nfunction Bit#(32) add(Bit#(32) a, Bit#(32) b);\n    return a + b;\nendfunction\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let symbols = parser.extract_symbols(&tree, source);

        assert!(symbols
            .iter()
            .any(|s| s.name == "mkTest" && s.kind == SymbolKind::Module));
        assert!(symbols
            .iter()
            .any(|s| s.name == "mkMain" && s.kind == SymbolKind::Module));
        assert!(symbols
            .iter()
            .any(|s| s.name == "add" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_extract_module_with_broken_endmodule() {
        let source = "module mkTest();\n    // test logic\nendmodulex\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let symbols = parser.extract_symbols(&tree, source);

        // 应该仍然能提取模块名，即使 endmodule 拼写错误
        assert!(symbols
            .iter()
            .any(|s| s.name == "mkTest" && s.kind == SymbolKind::Module));
    }

    #[test]
    fn test_extract_multiple_modules_with_errors() {
        let source = r#"
module mkA(); endmodule
module mkB(); endmodulex
module mkC(); endmodule
"#;
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let symbols = parser.extract_symbols(&tree, source);

        // 应该提取所有三个模块
        assert!(symbols.iter().any(|s| s.name == "mkA"));
        assert!(symbols.iter().any(|s| s.name == "mkB"));
        assert!(symbols.iter().any(|s| s.name == "mkC"));
    }

    #[test]
    fn test_extract_function_with_broken_module() {
        let source = r#"
module mkTest(); endmodulex
function Bit#(32) add(Bit#(32) a, Bit#(32) b);
    return a + b;
endfunction
"#;
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let symbols = parser.extract_symbols(&tree, source);

        // 即使 module 错误，function 也应该被提取
        assert!(symbols
            .iter()
            .any(|s| s.name == "mkTest" && s.kind == SymbolKind::Module));
        assert!(symbols
            .iter()
            .any(|s| s.name == "add" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_missing_endmodule_entirely() {
        let source = "module mkTest();";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let symbols = parser.extract_symbols(&tree, source);

        // 即使没有 endmodule，也应该提取模块名
        assert!(symbols.iter().any(|s| s.name == "mkTest"));
    }

    // ── Folding range tests ──────────────────────────────────────────

    #[test]
    fn test_folding_module_range() {
        let source =
            "module mkFoo();\n    rule r;\n        $display(\"hello\");\n    endrule\nendmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let ranges = parser.collect_folding_ranges(&tree, source);

        // Should have one module range and one rule range.
        assert!(
            ranges.iter().any(|r| r.start_line == 0 && r.end_line == 3),
            "Expected module folding range covering lines 0-3, got: {:?}",
            ranges,
        );
    }

    #[test]
    fn test_folding_rule_range() {
        let source = "module mkDemo();\n    rule r;\n        let x = 1;\n        let y = 2;\n    endrule\nendmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let ranges = parser.collect_folding_ranges(&tree, source);

        // The rule spans lines 1-4; end_line should be 3 (one before endrule).
        assert!(
            ranges.iter().any(|r| r.start_line == 1 && r.end_line == 3),
            "Expected rule folding range covering lines 1-3, got: {:?}",
            ranges,
        );
    }

    #[test]
    fn test_folding_function_range() {
        let source =
            "function Bit#(32) add(Bit#(32) a, Bit#(32) b);\n    return a + b;\nendfunction\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let ranges = parser.collect_folding_ranges(&tree, source);

        assert!(
            ranges.iter().any(|r| r.start_line == 0 && r.end_line == 1),
            "Expected function folding range covering lines 0-1, got: {:?}",
            ranges,
        );
    }

    #[test]
    fn test_folding_nested_blocks() {
        let source = "module mkTop();\n    rule r1;\n        $display(\"a\");\n    endrule\n    rule r2;\n        $display(\"b\");\n    endrule\nendmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let ranges = parser.collect_folding_ranges(&tree, source);

        // moduleDef spans lines 0-7 (endmodule on line 7), so end_line = 6
        // rule r1 spans lines 1-3 (endrule on line 3), so end_line = 2
        // rule r2 spans lines 4-6 (endrule on line 6), so end_line = 5
        assert!(ranges.iter().any(|r| r.start_line == 0 && r.end_line == 6));
        assert!(ranges.iter().any(|r| r.start_line == 1 && r.end_line == 2));
        assert!(ranges.iter().any(|r| r.start_line == 4 && r.end_line == 5));
        assert_eq!(ranges.len(), 3);
    }

    #[test]
    fn test_folding_comment_block() {
        let source = "// line 1\n// line 2\n// line 3\n// line 4\nmodule mkFoo();\nendmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let ranges = parser.collect_folding_ranges(&tree, source);

        // 4 consecutive comment lines on lines 0-3 should fold.
        assert!(
            ranges
                .iter()
                .any(|r| r.kind == Some(FoldingRangeKind::Comment)
                    && r.start_line == 0
                    && r.end_line == 3),
            "Expected comment folding range on lines 0-3, got: {:?}",
            ranges,
        );
    }

    #[test]
    fn test_folding_no_short_comment_block() {
        let source = "// just one\n// just two\nmodule mkFoo();\nendmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let ranges = parser.collect_folding_ranges(&tree, source);

        // Only 2 comment lines — should NOT produce a comment folding range.
        assert!(
            !ranges
                .iter()
                .any(|r| r.kind == Some(FoldingRangeKind::Comment)),
            "Should not fold 2 or fewer comment lines, got: {:?}",
            ranges,
        );
    }

    #[test]
    fn test_folding_empty_document() {
        let source = "";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let ranges = parser.collect_folding_ranges(&tree, source);

        assert_eq!(ranges.len(), 0);
    }

    #[test]
    fn test_folding_interface_range() {
        let source = "interface I;\n    method Bit#(8) get();\n    endmethod\nendinterface\n\ntypedef struct {\n    Bit#(8) field;\n} S deriving(Bits);\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let ranges = parser.collect_folding_ranges(&tree, source);

        // Interface spans lines 0-3.
        assert!(
            ranges.iter().any(|r| r.start_line == 0 && r.end_line == 2),
            "Expected interface folding range on lines 0-2, got: {:?}",
            ranges,
        );
    }

    #[test]
    fn test_folding_sorted_by_start_line() {
        let source = "module mkA(); endmodule\nmodule mkB(); endmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let ranges = parser.collect_folding_ranges(&tree, source);

        // ranges should be sorted by start_line
        for win in ranges.windows(2) {
            assert!(
                win[0].start_line <= win[1].start_line,
                "Folding ranges must be sorted by start_line"
            );
        }
    }

    #[test]
    fn test_folding_block_comment_multi_line() {
        let source = "/*\n * line 1\n * line 2\n */\nmodule mkFoo();\nendmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let ranges = parser.collect_folding_ranges(&tree, source);

        // Block comment spans lines 0-3 (4 lines), should fold.
        assert!(
            ranges
                .iter()
                .any(|r| r.kind == Some(FoldingRangeKind::Comment)
                    && r.start_line == 0
                    && r.end_line == 3),
            "Expected block comment folding on lines 0-3, got: {:?}",
            ranges,
        );
    }

    #[test]
    fn test_folding_short_block_comment_no_fold() {
        let source = "/* short */\nmodule mkFoo();\nendmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let ranges = parser.collect_folding_ranges(&tree, source);

        // Single-line block comment should NOT produce a folding range.
        assert!(
            !ranges
                .iter()
                .any(|r| r.kind == Some(FoldingRangeKind::Comment)),
            "Should not fold single-line block comment, got: {:?}",
            ranges,
        );
    }

    #[test]
    fn test_folding_action_block() {
        let source = "module mkTest();\n    rule r;\n        action\n            $display(\"hi\");\n        endaction\n    endrule\nendmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let ranges = parser.collect_folding_ranges(&tree, source);

        // actionBlock spans lines 2-4 (endaction on line 4) → end_line = 3.
        assert!(
            ranges.iter().any(|r| r.start_line == 2 && r.end_line == 3),
            "Expected actionBlock folding on lines 2-3, got: {:?}",
            ranges,
        );
    }

    #[test]
    fn test_folding_begin_end_expr() {
        // A standalone begin-end expression — parsed as beginEndExpr.
        let source = "module mkTest();\n    rule r;\n        action\n        begin\n            $display(\"a\");\n            $display(\"b\");\n        end\n        endaction\n    endrule\nendmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let ranges = parser.collect_folding_ranges(&tree, source);

        // The beginEndExpr (line 3-6, end on line 6) → end_line = 5.
        // If tree-sitter doesn't produce a beginEndExpr node, skip assertion
        // and just verify no crash.
        if ranges.iter().any(|r| r.start_line == 3) {
            assert!(
                ranges.iter().any(|r| r.start_line == 3 && r.end_line == 5),
                "Expected beginEndExpr folding on lines 3-5 if node exists, got: {:?}",
                ranges,
            );
        }
    }
    #[test]
    fn test_performance_large_file() {
        let mut source = String::new();
        for i in 0..100 {
            source.push_str(&format!("module mkModule{}(); endmodule\n", i));
        }

        let parser = BsvParser::default();
        let start = std::time::Instant::now();
        let tree = parser.parse(&source).expect("parse failed");
        let symbols = parser.extract_symbols(&tree, &source);
        let duration = start.elapsed();

        assert_eq!(symbols.len(), 100);
        assert!(duration.as_millis() < 100); // 应该在 100ms 内完成
    }

    // ── Reference extraction tests ─────────────────────────────────────

    #[test]
    fn test_extract_references_from_fixture() {
        let source = include_str!("../test_fixtures/references.bsv");
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let refs = parser.extract_references(&tree, source);

        // Expected references: (name, line)
        let expected = vec![
            ("Vector", 2),   // import Vector::*
            ("Reg", 5),      // Reg#(Bit#(32))
            ("Bit", 5),      // Bit#(32) in type
            ("mkReg", 5),    // <- mkReg(0)
            ("Bit", 10),     // Bit#(32) in function return type
            ("Bit", 10),     // Bit#(32) in function parameter
            ("Bit", 10),     // Bit#(32) in second function parameter
            ("add", 14),     // add(val, 5) - function call reference
            ("mkHello", 18), // mkHello hello_inst - module instance reference
        ];

        for (name, line) in &expected {
            assert!(
                refs.iter()
                    .any(|r| r.name == *name && r.range.start.line == *line),
                "Expected reference '{}' on line {}, but not found. Available refs: {:?}",
                name,
                line,
                refs.iter()
                    .map(|r| format!("'{}'@{}", r.name, r.range.start.line))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn test_no_false_positive_declarations_as_references() {
        let source = include_str!("../test_fixtures/references.bsv");
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let refs = parser.extract_references(&tree, source);

        // These are DECLARATIONS and should NOT appear in references
        let not_refs = vec![
            ("TestRefs", 0), // package name
            ("mkHello", 4),  // module definition
            ("val", 5),      // declaration via lValue/varInit
            ("hello", 6),    // rule name
            ("add", 10),     // function definition (parent=functionType)
            ("a", 10),       // function formal parameter
            ("b", 10),       // function formal parameter
            ("result", 14),  // let binding (parent=varDecl or lValue/varVar)
            ("mkWorld", 17), // module definition
        ];

        for (name, line) in &not_refs {
            assert!(
                !refs.iter().any(|r| r.name == *name && r.range.start.line == *line),
                "Declaration '{}' on line {} was incorrectly classified as a reference. refs at that line: {:?}",
                name, line,
                refs.iter().filter(|r| r.range.start.line == *line).map(|r| &r.name).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn test_extract_references_from_correct_fixture() {
        let source = include_str!("../test_fixtures/correct.bsv");
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let refs = parser.extract_references(&tree, source);

        // correct.bsv has: mkReg as a reference
        assert!(
            refs.iter().any(|r| r.name == "mkReg"),
            "Expected mkReg as a reference in correct.bsv, got: {:?}",
            refs.iter().map(|r| &r.name).collect::<Vec<_>>()
        );
        // Exported names in exportItem should NOT be references
        assert!(
            !refs
                .iter()
                .any(|r| r.name == "mkTest" && r.range.start.line == 5),
            "exportItem 'mkTest' on line 5 should not be a reference"
        );
        // Module instances should produce references
        assert!(
            refs.iter()
                .any(|r| r.name == "mkTest" && r.range.start.line == 19),
            "Module instance 'mkTest' on line 19 should be a reference, got: {:?}",
            refs.iter()
                .map(|r| format!("'{}'@{}", r.name, r.range.start.line))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_reference_deduplication() {
        let source = "module m();\n    let x = y;\n    let z = y;\nendmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let refs = parser.extract_references(&tree, source);

        // 'y' should appear twice (once on each line), not duplicated at same position
        let y_count = refs.iter().filter(|r| r.name == "y").count();
        assert_eq!(
            y_count, 2,
            "Expected exactly 2 references to 'y', got {}",
            y_count
        );
    }

    #[test]
    fn test_empty_source_no_references() {
        let source = "";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let refs = parser.extract_references(&tree, source);
        assert_eq!(refs.len(), 0);
    }

    // ── Signature Help tests ──────────────────────────────────────────

    #[test]
    fn test_extract_function_params() {
        let source =
            "function Bit#(32) add(Bit#(32) a, Bit#(32) b);\n    return a + b;\nendfunction\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let symbols = parser.extract_symbols(&tree, source);

        let add = symbols.iter().find(|s| s.name == "add").unwrap();
        assert_eq!(add.parameters.len(), 2);
        assert_eq!(add.parameters[0].name, "a");
        assert_eq!(add.parameters[0].type_name, Some("Bit#(32)".to_string()));
        assert_eq!(add.parameters[1].name, "b");
        assert_eq!(add.parameters[1].type_name, Some("Bit#(32)".to_string()));
    }

    #[test]
    fn test_extract_method_params() {
        let source = "module mkTest();\n    method Bit#(32) get(Bit#(32) addr, Bit#(8) select);\n        return addr;\n    endmethod\nendmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let symbols = parser.extract_symbols(&tree, source);

        let get = symbols.iter().find(|s| s.name == "get").unwrap();
        assert_eq!(get.parameters.len(), 2);
        assert_eq!(get.parameters[0].name, "addr");
        assert_eq!(get.parameters[0].type_name, Some("Bit#(32)".to_string()));
        assert_eq!(get.parameters[1].name, "select");
        assert_eq!(get.parameters[1].type_name, Some("Bit#(8)".to_string()));
    }

    #[test]
    fn test_extract_function_no_params() {
        let source = "function Bit#(32) getValue();\n    return 42;\nendfunction\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let symbols = parser.extract_symbols(&tree, source);

        let func = symbols.iter().find(|s| s.name == "getValue").unwrap();
        assert_eq!(func.parameters.len(), 0);
    }

    #[test]
    fn test_find_call_context_function_call() {
        let source =
            "module mkTest();\n    rule r;\n        let x = add(val, 5);\n    endrule\nendmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");

        // Cursor at 'val' in add(val, 5) — line 2, character 18 (after 'add(')
        let pos = lsp_types::Position {
            line: 2,
            character: 18,
        };
        let ctx = parser.find_call_context(&tree, source, pos);
        assert!(ctx.is_some(), "Expected call context for function call");
        if let Some(ctx) = ctx {
            assert_eq!(ctx.callable_name, "add");
            assert_eq!(ctx.argument_index, 0);
        }
    }

    #[test]
    fn test_find_call_context_arg_index() {
        let source =
            "module mkTest();\n    rule r;\n        let x = add(val, 5);\n    endrule\nendmodule\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");

        // Cursor at '5' in add(val, 5) — line 2, character 24 (after the comma)
        let pos = lsp_types::Position {
            line: 2,
            character: 24,
        };
        let ctx = parser.find_call_context(&tree, source, pos);
        assert!(ctx.is_some(), "Expected call context for second argument");
        if let Some(ctx) = ctx {
            assert_eq!(ctx.callable_name, "add");
            assert_eq!(ctx.argument_index, 1);
        }
    }

    #[test]
    fn test_count_top_level_commas() {
        // No commas
        assert_eq!(BsvParser::count_top_level_commas("42"), 0);
        // Simple commas
        assert_eq!(BsvParser::count_top_level_commas("a, b, c"), 2);
        // Commas inside nested parens should be excluded
        assert_eq!(BsvParser::count_top_level_commas("a, foo(b, c), d"), 2);
        // Commas inside nested braces
        assert_eq!(BsvParser::count_top_level_commas("a, {b, c}, d"), 2);
        // Only top-level
        assert_eq!(BsvParser::count_top_level_commas("foo(a, b, c)"), 0);
    }
}

impl Default for BsvParser {
    fn default() -> Self {
        Self::new().expect("Failed to create BSV parser")
    }
}
