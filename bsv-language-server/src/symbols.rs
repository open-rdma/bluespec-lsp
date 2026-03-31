use lsp_types::{Position, Range, Url};
use dashmap::DashMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Module,
    Function,
    Variable,
    Type,
    Interface,
    Package,
    Method,
    Rule,
    Unknown,
}

impl SymbolKind {
    pub fn from_node_kind(kind: &str) -> Self {
        match kind {
            "moduleDef" => SymbolKind::Module,
            "functionDef" | "methodDef" => SymbolKind::Function,
            "varDecl" => SymbolKind::Variable,
            "typeDef" => SymbolKind::Type,
            "interfaceDecl" => SymbolKind::Interface,
            "package" => SymbolKind::Package,
            "rule" => SymbolKind::Rule,
            _ => SymbolKind::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub range: Range,
    pub uri: Option<Url>,
    pub container: Option<String>,
    pub documentation: Option<String>,
}

impl Symbol {
    pub fn new(name: String, kind: SymbolKind, range: Range) -> Self {
        Self {
            name,
            kind,
            range,
            uri: None,
            container: None,
            documentation: None,
        }
    }
    
    pub fn contains_position(&self, position: &Position) -> bool {
        self.range.start <= *position && *position <= self.range.end
    }
}

#[derive(Debug, Clone)]
pub struct SymbolReference {
    pub uri: Url,
    pub range: Range,
}

#[derive(Debug, Default)]
pub struct SymbolTable {
    symbols: Arc<DashMap<String, Vec<Symbol>>>,
    references: Arc<DashMap<String, Vec<SymbolReference>>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            symbols: Arc::new(DashMap::new()),
            references: Arc::new(DashMap::new()),
        }
    }
    
    pub fn add_symbol(&self, uri: &Url, symbol: Symbol) {
        let uri_str = uri.to_string();
        let mut symbol_with_uri = symbol;
        symbol_with_uri.uri = Some(uri.clone());
        
        self.symbols
            .entry(uri_str)
            .or_insert_with(Vec::new)
            .push(symbol_with_uri);
    }
    
    pub fn get_symbols(&self, uri: &Url) -> Vec<Symbol> {
        self.symbols
            .get(&uri.to_string())
            .map(|symbols| symbols.clone())
            .unwrap_or_default()
    }
    
    pub fn find_symbol_at_position(&self, uri: &Url, position: Position) -> Option<Symbol> {
        self.symbols
            .get(&uri.to_string())
            .and_then(|symbols| {
                symbols.iter()
                    .find(|symbol| symbol.contains_position(&position))
                    .cloned()
            })
    }
    
    pub fn find_symbol_by_name(&self, name: &str) -> Vec<Symbol> {
        self.symbols
            .iter()
            .flat_map(|entry| entry.value().clone())
            .filter(|symbol| symbol.name == name)
            .collect()
    }
    
    pub fn get_all_symbols(&self) -> Vec<Symbol> {
        self.symbols
            .iter()
            .flat_map(|entry| entry.value().clone())
            .collect()
    }
    
    pub fn add_reference(&self, symbol_name: &str, reference: SymbolReference) {
        self.references
            .entry(symbol_name.to_string())
            .or_insert_with(Vec::new)
            .push(reference);
    }
    
    pub fn get_references(&self, symbol_name: &str) -> Vec<SymbolReference> {
        self.references
            .get(symbol_name)
            .map(|refs| refs.clone())
            .unwrap_or_default()
    }
    
    pub fn clear_file(&self, uri: &Url) {
        self.symbols.remove(&uri.to_string());
        
        // 清理引用
        let uri_str = uri.to_string();
        self.references.retain(|_, refs| {
            refs.retain(|r| r.uri.to_string() != uri_str);
            !refs.is_empty()
        });
    }
    
    pub fn clear_all(&self) {
        self.symbols.clear();
        self.references.clear();
    }
}
