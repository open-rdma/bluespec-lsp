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
        symbols
    }
    
    fn traverse_node(&self, node: Node, source: &str, symbols: &mut Vec<crate::Symbol>) {
        // 提取模块定义
        if node.kind() == "moduleDef" {
            if let Some(name_node) = self.get_module_name_node(node) {
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
        }
        
        // 提取函数/方法定义
        if node.kind() == "functionDef" || node.kind() == "methodDef" {
            if let Some(name_node) = self.get_callable_name_node(node) {
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

    fn get_module_name_node<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        self.child_by_kind(node, "moduleProto")
            .and_then(|proto| self.child_by_kind(proto, "identifier"))
    }

    fn get_callable_name_node<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "methodDef" {
            return self.child_by_kind(node, "identifier");
        }

        if node.kind() == "functionDef" {
            return self.child_by_kind(node, "functionProto")
                .and_then(|proto| self.child_by_kind(proto, "functionType"))
                .and_then(|ft| self.child_by_kind(ft, "identifier"));
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
}

impl Default for BsvParser {
    fn default() -> Self {
        Self::new().expect("Failed to create BSV parser")
    }
}
