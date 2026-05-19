use crate::constant_expansion::{ConstantEvaluator, ConstantParser};
use crate::{diagnostics, utils, BsvParser, SymbolTable};
use async_trait::async_trait;
use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService};

type LspResult<T> = std::result::Result<T, tower_lsp::jsonrpc::Error>;

pub struct Backend {
    client: Client,
    parser: BsvParser,
    symbol_table: Arc<RwLock<SymbolTable>>,
    documents: Arc<RwLock<HashMap<Url, String>>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            parser: BsvParser::default(),
            symbol_table: Arc::new(RwLock::new(SymbolTable::new())),
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn update_document(&self, uri: &Url, text: &str) -> crate::Result<()> {
        // Save document content, then release the lock before any .await calls.
        {
            let mut documents = self.documents.write().await;
            documents.insert(uri.clone(), text.to_string());
        }

        // Parse the document, update symbol table, and publish diagnostics.
        match self.parser.parse(text) {
            Ok(tree) => {
                // Publish syntax diagnostics from the parse tree.
                let diags = diagnostics::DiagnosticCollector::collect(&tree, text);
                self.client
                    .publish_diagnostics(uri.clone(), diags, None)
                    .await;

                let symbols = self.parser.extract_symbols(&tree, text);
                let symbols_len = symbols.len();

                let symbol_table = self.symbol_table.write().await;
                symbol_table.clear_file(uri);

                for symbol in symbols {
                    symbol_table.add_symbol(uri, symbol);
                }

                debug!("Updated symbols for {}: {} symbols found", uri, symbols_len);
                Ok(())
            }
            Err(e) => {
                // Publish a fallback diagnostic when parsing fails entirely.
                let diag = Diagnostic {
                    range: Range {
                        start: Position {
                            line: 0,
                            character: 0,
                        },
                        end: Position {
                            line: 0,
                            character: 0,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("bsv".to_string()),
                    message: format!("Failed to parse file: {}", e),
                    ..Default::default()
                };
                self.client
                    .publish_diagnostics(uri.clone(), vec![diag], None)
                    .await;

                warn!("Failed to parse {}: {}", uri, e);
                Err(e)
            }
        }
    }

    async fn get_document_symbols(&self, uri: &Url) -> Vec<SymbolInformation> {
        let symbol_table = self.symbol_table.read().await;
        let symbols = symbol_table.get_symbols(uri);

        symbols
            .into_iter()
            .map(|symbol| SymbolInformation {
                name: symbol.name,
                kind: match symbol.kind {
                    crate::SymbolKind::Module => SymbolKind::MODULE,
                    crate::SymbolKind::Function => SymbolKind::FUNCTION,
                    crate::SymbolKind::Variable => SymbolKind::VARIABLE,
                    crate::SymbolKind::Type => SymbolKind::CLASS,
                    crate::SymbolKind::Interface => SymbolKind::INTERFACE,
                    crate::SymbolKind::Package => SymbolKind::PACKAGE,
                    crate::SymbolKind::Method => SymbolKind::METHOD,
                    crate::SymbolKind::Rule => SymbolKind::EVENT,
                    crate::SymbolKind::Unknown => SymbolKind::NULL,
                },
                tags: None,
                #[allow(deprecated)]
                deprecated: None,
                location: Location {
                    uri: symbol.uri.unwrap_or_else(|| uri.clone()),
                    range: symbol.range,
                },
                container_name: symbol.container,
            })
            .collect()
    }

    async fn goto_definition(&self, uri: &Url, position: Position) -> Option<Location> {
        let symbol_table = self.symbol_table.read().await;

        // 首先在当前文档中查找符号
        if let Some(symbol) = symbol_table.find_symbol_at_position(uri, position) {
            return Some(Location {
                uri: symbol.uri.unwrap_or_else(|| uri.clone()),
                range: symbol.range,
            });
        }

        // 如果没有找到，尝试在其他文档中查找
        let documents = self.documents.read().await;
        let current_text = documents.get(uri)?;

        // 提取光标位置的单词
        if let Some(line) = utils::get_line_content(current_text, position.line as usize) {
            if let Some(word) = self.extract_word_at_position(line, position.character as usize) {
                let symbols = symbol_table.find_symbol_by_name(&word);
                if let Some(symbol) = symbols.first() {
                    if let Some(symbol_uri) = &symbol.uri {
                        return Some(Location {
                            uri: symbol_uri.clone(),
                            range: symbol.range,
                        });
                    }
                }
            }
        }

        None
    }

    fn extract_word_at_position(&self, line: &str, character: usize) -> Option<String> {
        if character >= line.len() {
            return None;
        }

        let mut start = character;
        let mut end = character;

        // 向左扩展
        while start > 0
            && (line
                .chars()
                .nth(start - 1)
                .is_some_and(|c| c.is_alphanumeric() || c == '_'))
        {
            start -= 1;
        }

        // 向右扩展
        while end < line.len()
            && (line
                .chars()
                .nth(end)
                .is_some_and(|c| c.is_alphanumeric() || c == '_'))
        {
            end += 1;
        }

        if start < end {
            Some(line[start..end].to_string())
        } else {
            None
        }
    }

    fn find_word_start(&self, line: &str, character: usize) -> usize {
        let mut start = character;

        // 向左扩展找到单词开始
        while start > 0
            && (line
                .chars()
                .nth(start - 1)
                .is_some_and(|c| c.is_alphanumeric() || c == '_'))
        {
            start -= 1;
        }

        start
    }
}

#[async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        info!("Initializing BSV Language Server");

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // Use full document sync because hover and constant expansion
                // operate on the complete text. Incremental sync would require
                // applying edits manually, which is not currently implemented.
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        info!("BSV Language Server initialized");
        let _ = self
            .client
            .log_message(MessageType::INFO, "BSV Language Server initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        info!("Shutting down BSV Language Server");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        debug!("Document opened: {}", uri);
        if let Err(e) = self.update_document(&uri, &text).await {
            warn!("Error updating document {}: {}", uri, e);
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let changes = params.content_changes;

        debug!("Document changed: {}", uri);

        // With full sync, the change payload contains the full updated document text.
        if let Some(change) = changes.last() {
            if let Err(e) = self.update_document(&uri, &change.text).await {
                warn!("Error updating document {}: {}", uri, e);
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        debug!("Document closed: {}", uri);

        // 清理文档内容
        let mut documents = self.documents.write().await;
        documents.remove(&uri);

        // 清理符号表
        let symbol_table = self.symbol_table.write().await;
        symbol_table.clear_file(&uri);
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        debug!("Goto definition request: {} at {:?}", uri, position);

        match self.goto_definition(&uri, position).await {
            Some(location) => Ok(Some(GotoDefinitionResponse::Scalar(location))),
            None => Ok(None),
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;

        debug!("Document symbols request: {}", uri);

        let symbols = self.get_document_symbols(&uri).await;

        if symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(DocumentSymbolResponse::Flat(symbols)))
        }
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> LspResult<Option<Vec<SymbolInformation>>> {
        let query = params.query.to_lowercase();

        debug!("Workspace symbols request: {}", query);

        let symbol_table = self.symbol_table.read().await;
        let all_symbols = symbol_table.get_all_symbols();
        let mut result = Vec::new();

        for symbol in all_symbols {
            if symbol.name.to_lowercase().contains(&query) {
                if let Some(symbol_uri) = &symbol.uri {
                    result.push(SymbolInformation {
                        name: symbol.name.clone(),
                        kind: match symbol.kind {
                            crate::SymbolKind::Module => SymbolKind::MODULE,
                            crate::SymbolKind::Function => SymbolKind::FUNCTION,
                            crate::SymbolKind::Variable => SymbolKind::VARIABLE,
                            crate::SymbolKind::Type => SymbolKind::CLASS,
                            crate::SymbolKind::Interface => SymbolKind::INTERFACE,
                            crate::SymbolKind::Package => SymbolKind::PACKAGE,
                            crate::SymbolKind::Method => SymbolKind::METHOD,
                            crate::SymbolKind::Rule => SymbolKind::EVENT,
                            crate::SymbolKind::Unknown => SymbolKind::NULL,
                        },
                        tags: None,
                        #[allow(deprecated)]
                        deprecated: None,
                        location: Location {
                            uri: symbol_uri.clone(),
                            range: symbol.range,
                        },
                        container_name: symbol.container,
                    });
                }
            }
        }

        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Some(result))
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        info!(
            "Hover request: {} at line={}, char={}",
            uri, position.line, position.character
        );

        // First, try to find and expand a constant at this position
        let documents = self.documents.read().await;
        if let Some(text) = documents.get(&uri) {
            info!("Document found, length={}", text.len());
            let const_parser = ConstantParser::new();
            let all_constant_defs: Vec<_> = documents
                .values()
                .flat_map(|doc_text| const_parser.parse(doc_text))
                .collect();
            let all_evaluator = ConstantEvaluator::new(all_constant_defs.clone());

            // Method 1: Check if we're at a constant definition position
            if let Some(const_def) = const_parser.find_constant_at_position(text, position) {
                info!("Found constant definition: {}", const_def.name);

                if let Some(result) = all_evaluator.expand(&const_def.name) {
                    let hover_text = if result.success {
                        format!(
                            "**{}** = `{}`\n\n```\n{}\n```",
                            const_def.name,
                            result.final_value,
                            result.format_trace()
                        )
                    } else {
                        format!(
                            "**{}** = `{}`\n\n⚠️ Could not fully expand",
                            const_def.name, const_def.value
                        )
                    };

                    let contents = HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: hover_text,
                    });

                    return Ok(Some(Hover {
                        contents,
                        range: Some(const_def.range),
                    }));
                }
            }

            // Method 2: Check if the word at cursor is a constant name (usage position)
            if let Some(line) = utils::get_line_content(text, position.line as usize) {
                info!("Line content: '{}'", line);
                if let Some(word) = self.extract_word_at_position(line, position.character as usize)
                {
                    info!("Extracted word: '{}'", word);
                    // Check if this word is a defined constant
                    let const_def = const_parser
                        .find_constant_by_name(text, &word)
                        .or_else(|| all_constant_defs.iter().find(|d| d.name == word).cloned());
                    if let Some(const_def) = const_def {
                        info!("Found constant by name: {}", const_def.name);

                        if let Some(result) = all_evaluator.expand(&word) {
                            let hover_text = if result.success {
                                format!(
                                    "**{}** = `{}`\n\n```\n{}\n```",
                                    word,
                                    result.final_value,
                                    result.format_trace()
                                )
                            } else {
                                format!(
                                    "**{}** = `{}`\n\n⚠️ Could not fully expand",
                                    word, const_def.value
                                )
                            };

                            let contents = HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: hover_text,
                            });

                            // Find word boundaries for range
                            let word_start =
                                self.find_word_start(line, position.character as usize);
                            let word_range = Range {
                                start: Position {
                                    line: position.line,
                                    character: word_start as u32,
                                },
                                end: Position {
                                    line: position.line,
                                    character: (word_start + word.len()) as u32,
                                },
                            };

                            return Ok(Some(Hover {
                                contents,
                                range: Some(word_range),
                            }));
                        }
                    } else {
                        info!(
                            "Constant '{}' not found in document or open documents",
                            word
                        );
                    }
                } else {
                    info!("No word extracted at position {}", position.character);
                }
            } else {
                info!("No line content found at line {}", position.line);
            }
        } else {
            info!("Document not found: {}", uri);
        }
        drop(documents);

        // Fall back to symbol hover
        let symbol_table = self.symbol_table.read().await;

        if let Some(symbol) = symbol_table.find_symbol_at_position(&uri, position) {
            let contents = HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    "**{}**\n\n*Kind: {}*",
                    symbol.name,
                    match symbol.kind {
                        crate::SymbolKind::Module => "Module",
                        crate::SymbolKind::Function => "Function/Method",
                        crate::SymbolKind::Variable => "Variable",
                        crate::SymbolKind::Type => "Type",
                        crate::SymbolKind::Interface => "Interface",
                        crate::SymbolKind::Package => "Package",
                        crate::SymbolKind::Method => "Method",
                        crate::SymbolKind::Rule => "Rule",
                        crate::SymbolKind::Unknown => "Unknown",
                    }
                ),
            });

            return Ok(Some(Hover {
                contents,
                range: Some(symbol.range),
            }));
        }

        Ok(None)
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> LspResult<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;

        debug!("Folding range request: {}", uri);

        let documents = self.documents.read().await;
        if let Some(text) = documents.get(&uri) {
            match self.parser.parse(text) {
                Ok(tree) => {
                    let ranges = self.parser.collect_folding_ranges(&tree, text);
                    return Ok(Some(ranges));
                }
                Err(e) => {
                    warn!("Failed to parse for folding range {}: {}", uri, e);
                }
            }
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        debug!("Completion request: {} at {:?}", uri, position);

        let symbol_table = self.symbol_table.read().await;
        let symbols = symbol_table.get_symbols(&uri);

        let mut items = Vec::new();

        for symbol in symbols {
            let kind = match symbol.kind {
                crate::SymbolKind::Module => CompletionItemKind::MODULE,
                crate::SymbolKind::Function => CompletionItemKind::FUNCTION,
                crate::SymbolKind::Variable => CompletionItemKind::VARIABLE,
                crate::SymbolKind::Type => CompletionItemKind::CLASS,
                crate::SymbolKind::Interface => CompletionItemKind::INTERFACE,
                crate::SymbolKind::Package => CompletionItemKind::MODULE, // 使用 MODULE 替代 PACKAGE
                crate::SymbolKind::Method => CompletionItemKind::METHOD,
                crate::SymbolKind::Rule => CompletionItemKind::EVENT,
                crate::SymbolKind::Unknown => CompletionItemKind::TEXT,
            };

            items.push(CompletionItem {
                label: symbol.name,
                kind: Some(kind),
                detail: Some(format!("{:?}", symbol.kind)),
                ..Default::default()
            });
        }

        Ok(Some(CompletionResponse::Array(items)))
    }
}

pub async fn run(
    stdin: impl tokio::io::AsyncRead + Unpin,
    stdout: impl tokio::io::AsyncWrite + Unpin,
) -> crate::Result<()> {
    let (service, socket) = LspService::build(Backend::new).finish();

    let server = tower_lsp::Server::new(stdin, stdout, socket);
    server.serve(service).await;

    Ok(())
}
