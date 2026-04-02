// 改进后的 parser.rs - 完整实现
// 这个文件展示了如何实施容错符号提取

use tree_sitter::{Parser, Tree, Node};
use crate::Result;
use std::sync::Mutex;

extern "C" {
    fn tree_sitter_bsv() -> *const std::ffi::c_void;
}

pub struct BsvParser {
    parser: Mutex<Parser>,
}

impl BsvParser {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        
        let language = unsafe {
            let ptr = tree_sitter_bsv();
            std::mem::transmute(ptr)
        };
        
        parser.set_language(language)
            .map_err(|_| crate::Error::TreeSitter)?;
        
        Ok(Self {
            parser: Mutex::new(parser),
        })
    }
    
    pub fn parse(&self, source: &str) -> Result<Tree> {
        let mut parser = self.parser.lock().unwrap();
        parser.parse(source, None)
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
                            documentation: Some("[Error recovery] Module definition with syntax errors".to_string()),
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
                        symbols.push(crate::Symbol {
                            name: name.to_string(),
                            kind: if node.kind() == "methodDef" { crate::SymbolKind::Method } else { crate::SymbolKind::Function },
                            range: self.node_to_range(&name_node),
                            uri: None,
                            container: None,
                            documentation: None,
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
    
    #[allow(dead_code)]
    fn child_by_field_name<'a>(&self, node: Node<'a>, field_name: &str) -> Option<Node<'a>> {
        let mut cursor = node.walk();
        let mut result = None;
        for child in node.children_by_field_name(field_name, &mut cursor) {
            result = Some(child);
            break;
        }
        result
    }
    
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
    fn try_extract_module_from_stmt(&self, node: Node, source: &str, symbols: &mut Vec<crate::Symbol>) {
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
                            documentation: Some("[Error recovery] Module extracted from incomplete definition".to_string()),
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
                            documentation: Some("[Error recovery] Module extracted from incomplete definition".to_string()),
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
    
    fn find_identifier_after_module_impl<'a>(&self, node: Node<'a>, source: &str, found_module: &mut bool) -> Option<Node<'a>> {
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
            if let Some(found) = self.find_identifier_after_module_impl(child, source, found_module) {
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
    fn extract_functions_from_error(&self, node: Node, source: &str, symbols: &mut Vec<crate::Symbol>) {
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
                        documentation: Some("[Error recovery] Function extracted from ERROR node".to_string()),
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
    fn extract_function_from_vardecl(&self, node: Node, source: &str, symbols: &mut Vec<crate::Symbol>) {
        if node.kind() != "varDecl" {
            return;
        }
        
        // 检查是否看起来像函数定义：type + varInit 中有参数列表
        // 例如：Bit#(32) add(Bit#(32) a, Bit#(32) b)
        let has_type = self.child_by_kind(node, "type").is_some();
        let var_init = self.child_by_kind(node, "varInit");
        
        if has_type && var_init.is_some() {
            let var_init = var_init.unwrap();
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

        assert!(symbols.iter().any(|s| s.name == "mkTest" && s.kind == SymbolKind::Module));
        assert!(symbols.iter().any(|s| s.name == "mkMain" && s.kind == SymbolKind::Module));
        assert!(symbols.iter().any(|s| s.name == "add" && s.kind == SymbolKind::Function));
    }
    
    #[test]
    fn test_extract_module_with_broken_endmodule() {
        let source = "module mkTest();\n    // test logic\nendmodulex\n";
        let parser = BsvParser::default();
        let tree = parser.parse(source).expect("parse failed");
        let symbols = parser.extract_symbols(&tree, source);
        
        // 应该仍然能提取模块名，即使 endmodule 拼写错误
        assert!(symbols.iter().any(|s| s.name == "mkTest" && s.kind == SymbolKind::Module));
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
        assert!(symbols.iter().any(|s| s.name == "mkTest" && s.kind == SymbolKind::Module));
        assert!(symbols.iter().any(|s| s.name == "add" && s.kind == SymbolKind::Function));
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
}

impl Default for BsvParser {
    fn default() -> Self {
        Self::new().expect("Failed to create BSV parser")
    }
}
