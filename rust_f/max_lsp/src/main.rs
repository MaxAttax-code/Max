mod convert;
mod completion;

use std::{collections::HashMap, path::PathBuf};

use max_runtime::natives::NativeRegistry;
use max_compiler::{
    project::{compile_project_with_overlay, normalise},
    compiler::NativeNames,
    error::{MaxDiagnostic, Severity},
    naming::Casing,
    token::TokenKind as K,
    lexer::Lexer
};

use convert::*;

use tokio::sync::RwLock;
use tower_lsp::{
    LspService as LspS,
    Client,
    Server,
    LanguageServer,
    jsonrpc::Result,
    lsp_types::*
};

/// Holds the necessary info of a text document
struct Document {
    uri: Url,
    text: String,
    version: i32
}

struct Backend {
    client: Client,
    /// Currently *open* documents
    documents: RwLock<HashMap<Url, Document>>,
    /// Root of the documents dir
    root: RwLock<Option<PathBuf>>,
    registry: NativeRegistry
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        params: InitializeParams,
    ) -> Result<InitializeResult> {
        let root = params.root_uri
            .and_then(|uri| uri.to_file_path().ok())
            .or_else(|| {
                params.workspace_folders
                    .and_then(|folders| folders.into_iter().next())
                    .and_then(|folder| folder.uri.to_file_path().ok())
            });
        *self.root.write().await = root;

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(
                    TextDocumentSyncCapability::Kind(
                        TextDocumentSyncKind::FULL
                    )
                ),
                code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions
                    {
                        code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
                        ..Default::default()
                    }
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".into()]),
                    resolve_provider: Some(false),
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "Max Language Server".into(),
                version: Some("0.1.0".into()),
            }),
        })
    }

    async fn initialized(
        &self,
        _: InitializedParams,
    ) {
        self.client
            .log_message(
                MessageType::INFO,
                "Max Language Server initialized!"
            )
            .await;
    }

    async fn did_open(
        &self,
        params: DidOpenTextDocumentParams,
    ) {
        let document = params.text_document;

        let uri = document.uri.clone();

        self.documents.write().await.insert(
            uri.clone(),
            Document {
                uri,
                text: document.text,
                version: document.version,
            },
        );

        self.analyze_document(&document.uri).await;
    }

    async fn did_change(
        &self,
        params: DidChangeTextDocumentParams,
    ) {
        let uri = params.text_document.uri.clone();

        if let Some(change) = params.content_changes.into_iter().last() {
            if let Some(document) = self.documents.write().await.get_mut(&uri) {
                document.text = change.text;
                document.version = params.text_document.version;
            }
        }

        self.analyze_document(&uri).await;
    }

    async fn did_close(
        &self,
        params: DidCloseTextDocumentParams,
    ) {
        self.documents.write().await.remove(
            &params.text_document.uri
        );

        self.client
            .publish_diagnostics(
                params.text_document.uri,
                Vec::new(),
                None,
            )
            .await;
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else { return Ok(None) };

        let mut actions= CodeActionResponse::new();
        
        // Nice little helper to speed this up!
        let mut push_quick_fix = |diag: &Diagnostic, title: String, fix_kind: FixKind| {
            let quick_fix = quick_fix(&uri, &docs, diag, title, fix_kind);
            if let Some(fixable) = quick_fix {
                actions.push(fixable);
            }
        };

        for diag in &params.context.diagnostics {
            if diag.source.as_deref() != Some("max") { continue; }
            let Some(NumberOrString::String(code)) = &diag.code else { continue };

            // ALLLLLL the codes in the word!
            match code.as_str() {
                "pascal-case-recommendation" => {
                    let old = text_in_range(&doc.text, diag.range);
                    let new = Casing::to_pascal_case(&old);
                    push_quick_fix(diag, format!("Rename to `{new}`"), FixKind::RenameEverywhere { old, new });
                }
                "for-in" => {
                    push_quick_fix(diag, "Use 'to' instead".into(), FixKind::Replace { text: "to".into(), range: diag.range });
                }
                "plus-plus-usage" => {
                    push_quick_fix(diag, "Use '+='".into(), FixKind::Replace { text: " += 1".into(), range: diag.range });
                }
                "minus-minus-usage" => {
                    push_quick_fix(diag, "Use '-='".into(), FixKind::Replace { text: " -= 1".into(), range: diag.range });
                }
                "param-coroutine" => {
                    push_quick_fix(diag, "Use '=>'".into(), FixKind::Replace { text: "=>".into(), range: diag.range });
                }
                // ---
                // COMPILER DIAGNOSTICS
                // ---
                "declaration-duplicate" => {
                    push_quick_fix(diag, "Rename with underscore".into(), FixKind::Push { text: "_".into(), position: (diag.range.start.line, diag.range.start.character) });
                }
                "duplicate-enum-name" => {
                    push_quick_fix(diag, "Rename as version".into(), FixKind::Push { text: "Version".into(), position: (diag.range.end.line, diag.range.end.character) });
                }
                _ => {}
            }
        }

        Ok((!actions.is_empty()).then_some(actions))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(&uri) else { return Ok(None); };

        let items = completion::complete(&doc.text, pos, &self.registry);
        Ok((!items.is_empty()).then(|| CompletionResponse::Array(items)))
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

/* --- ORDINARY HELPER METHODS --- */

/// Finds all the occurrences of a given name within the text content as
/// a Vector of `Range`. The [`lexer`][`Lexer`] is used to tokenise the text
/// and find all identifiers by the given name. See [`span_to_range`] to
/// understand how these found occurrences are mapped into Ranges
fn occurrences(text: &str, name: &str) -> Vec<Range> {
    let Ok(tokens) = Lexer::new(text).tokenise() else { return vec![] };

    tokens.iter().enumerate()
        .filter(|(i, t)| {
            // Each identifier (not a member accessor!) is found
            t.kind == K::Identifier
                && t.value == name
                && !(*i > 0 && tokens[i - 1].kind == K::Dot)
        })
        .map(|(_, t)| span_to_range(text, t.span))
        .collect()
}

/* -------------------------------- */

impl Backend {
    pub fn new(client: Client) -> Self {
        Backend { client, documents: RwLock::new(HashMap::new()), root: RwLock::new(None), registry: NativeRegistry::new() }
    }

    async fn analyze_document(&self, _changed: &Url) {
        let Some(root) = self.root.read().await.clone() else {
            self.client.log_message(
                MessageType::ERROR,
                "Cannot analyze Max document: initialize request had no root URI or workspace folder.",
            ).await;
            return;
        };

        let mut url_for: HashMap<PathBuf, Url> = HashMap::new();
        let mut overlay: HashMap<PathBuf, String> = HashMap::new();
        for doc in self.documents.read().await.values() {
            if let Ok(path) = doc.uri.to_file_path() {
                let key = normalise(&path);
                // Collect the relation of PathBufs and Urls
                url_for.insert(key.clone(), doc.uri.clone());
                // As well as the Text in the doc of the PathBuf
                overlay.insert(key, doc.text.clone());
            }
        }

        let native_names = NativeNames {
            module_tasks: self.registry.module_task_keys(),
            module_properties: self.registry.module_property_keys(),
            global_tasks: self.registry.global_task_names(),
        };
        
        let result = tokio::task::spawn_blocking(move || {
            compile_project_with_overlay(
                &root.join("src"),
                &root.join("library"),
                &overlay,
                native_names,
            )
        }).await;

        let open: Vec<(Url, i32, String)> = self.documents.read().await
            .values().map(|d| (d.uri.clone(), d.version, d.text.clone())).collect();
        let mut by_file: HashMap<Url, Vec<Diagnostic>> = HashMap::new();
        match result {
            Ok(Ok(analysis)) => {
                for (path, diagnostics) in analysis.diagnostics {
                    if let Some(uri) = url_for.get(&path) {
                        let Some((_, _, text)) = open.iter().find(|(open_uri, _, _)| open_uri == uri) else {
                            continue;
                        };
                        by_file.insert(
                            uri.clone(),
                            diagnostics.iter().map(|diagnostic| Self::to_diagnostic(diagnostic, text)).collect(),
                        );
                    }
                }
            }
            // A Project error? Now that's a fatal error!
            Ok(Err(err)) => {
                self.client.log_message(MessageType::ERROR, err.to_string()).await;
            }
            // This is just an error by the LSP
            Err(err) => {
                self.client.log_message(MessageType::ERROR, err.to_string()).await;
            }
        }

        // Publish for every open doc, so fixed errors get cleared
        for (uri, version, _) in open {
            // Ensure old diagnostics doesn't get published
            let current = self.documents.read().await.get(&uri).map(|d| d.version);
            if current != Some(version) { continue; }

            let diags = by_file.remove(&uri).unwrap_or_default();
            self.client.publish_diagnostics(uri, diags, Some(version)).await;
        }
    }

    /// Converts a MaxDiagnostic and it's given message into an LSP Diagnostic.
    /// 
    fn to_diagnostic(diagnostic: &MaxDiagnostic, text: &str) -> Diagnostic {
        let span = diagnostic.span;
        let severity = match diagnostic.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            Severity::Info => DiagnosticSeverity::INFORMATION,
            Severity::Hint => DiagnosticSeverity::HINT,
        };

        Diagnostic {
            range: Range::new(
                Self::to_position(span.start_line, span.start_column, text),
                Self::to_position(span.end_line, span.end_column, text),
            ),
            severity: Some(severity),
            code: Some(NumberOrString::String(diagnostic.code.to_owned())),
            source: Some("max".into()),
            message: diagnostic.message.clone(),
            ..Default::default()
        }
    }

    /// Find the `Position` at the line and column of the given text.
    /// The line index is 0-based, so the given line value is saturate-subtracted by 1
    /// to become equal.
    /// The character in a `Position` is a UTF-16 code always, not a string representation
    fn to_position(line: u32, column: u32, text: &str) -> Position {
        let line_index = line.saturating_sub(1);
        let character = text.lines()
            .nth(line_index as usize)
            .unwrap_or_default()
            .chars()
            .take(column.saturating_sub(1) as usize)
            .map(char::len_utf16)
            .sum::<usize>() as u32;
        Position::new(line_index, character)
    }
}


enum FixKind {
    /// Replace exactly the diagnostics range
    Replace { text: String, range: Range },
    /// Rename every reference to an identifier
    RenameEverywhere { old: String, new: String },
    /// Push text into the given position
    Push { text: String, position: (u32, u32) }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspS::new(Backend::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}