use dashmap::DashMap;
use lsp_types::{Position, Range, Url};
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

#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub name: String,
    pub type_name: Option<String>,
}

impl ParameterInfo {
    pub fn new(name: String, type_name: Option<String>) -> Self {
        Self { name, type_name }
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
    pub parameters: Vec<ParameterInfo>,
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
            parameters: Vec::new(),
        }
    }

    pub fn contains_position(&self, position: &Position) -> bool {
        self.range.start <= *position && *position <= self.range.end
    }
}

#[derive(Debug, Clone)]
pub struct Reference {
    pub name: String,
    pub range: Range,
    pub uri: Option<Url>,
}

#[derive(Debug, Default)]
pub struct SymbolTable {
    symbols: Arc<DashMap<String, Vec<Symbol>>>,
    references: Arc<DashMap<String, Vec<Reference>>>,
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
            .or_default()
            .push(symbol_with_uri);
    }

    pub fn get_symbols(&self, uri: &Url) -> Vec<Symbol> {
        self.symbols
            .get(&uri.to_string())
            .map(|symbols| symbols.clone())
            .unwrap_or_default()
    }

    pub fn find_symbol_at_position(&self, uri: &Url, position: Position) -> Option<Symbol> {
        self.symbols.get(&uri.to_string()).and_then(|symbols| {
            symbols
                .iter()
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

    pub fn clear_file(&self, uri: &Url) {
        self.symbols.remove(&uri.to_string());
        self.references.remove(&uri.to_string());
    }

    // ── Reference methods ─────────────────────────────────────────────

    pub fn add_reference(&self, uri: &Url, reference: Reference) {
        let uri_str = uri.to_string();
        let mut ref_with_uri = reference;
        ref_with_uri.uri = Some(uri.clone());
        self.references
            .entry(uri_str)
            .or_default()
            .push(ref_with_uri);
    }

    pub fn get_references(&self, uri: &Url) -> Vec<Reference> {
        self.references
            .get(&uri.to_string())
            .map(|refs| refs.clone())
            .unwrap_or_default()
    }

    pub fn find_references_by_name(&self, name: &str) -> Vec<Reference> {
        self.references
            .iter()
            .flat_map(|entry| entry.value().clone())
            .filter(|r| r.name == name)
            .collect()
    }

    pub fn clear_references(&self, uri: &Url) {
        self.references.remove(&uri.to_string());
    }
}
