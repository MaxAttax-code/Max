use std::collections::HashMap;

use max_compiler::ast::Span;
use tower_lsp::lsp_types::*;
use crate::{Document, FixKind, occurrences};

/// Creates a quick-fix code action, using the given arguments
/// The `range` will be where the code action takes place, and the
/// `new_text` will replace
pub fn quick_fix(uri: &Url, docs: &HashMap<Url, Document>, diag: &Diagnostic, title: String, fix_kind: FixKind) -> Option<CodeActionOrCommand> {
    // We use '?' so that a non-existent fix means no action to be taken
    let edit = fix_to_edit(uri, docs, fix_kind)?;
    
    Some(CodeActionOrCommand::CodeAction(CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),     // links the fix to the squiggle
        edit: Some(edit),
        is_preferred: Some(true),
        ..Default::default()
    }))
}

/// Convert a given `FixKind` into a fitted `WorkspaceEdit`
pub fn fix_to_edit(uri: &Url, docs: &HashMap<Url, Document>, fix_kind: FixKind) -> Option<WorkspaceEdit> {
    let fix = match fix_kind {
        FixKind::Push { text, position: (line, column) } => {
            let at = Position::new(line, column);
            WorkspaceEdit {
                changes: Some(HashMap::from([
                    (
                        uri.clone(),
                        vec![TextEdit {
                            // Essentially just pushes the new text to the given position
                            range: Range::new(at, at),
                            new_text: text
                        }]
                    )
                ])),
                ..Default::default()
            }
        },

        FixKind::Replace { text, range } => WorkspaceEdit {
            changes: Some(HashMap::from([
                (
                    uri.clone(),
                    vec![TextEdit {
                        range,
                        new_text: text
                    }]
                )
            ])),
            ..Default::default()
        },

        FixKind::RenameEverywhere { old, new } => {
            let mut changes = HashMap::new();
            // Read by awaiting. This is still safe

            for doc_uri in docs.keys() {
                // Get the URL document from the hashmap
                let doc_text = &docs.get(doc_uri)
                    .expect(&format!("Expected a corresponding document to doc URL '{}'", doc_uri.as_str()))
                    .text;
                let edits: Vec<TextEdit> = occurrences(doc_text, &old).into_iter()
                    .map(|range| TextEdit { range, new_text: new.clone() }).collect();
                if !edits.is_empty() { changes.insert(doc_uri.clone(), edits); }
            }
            return Some(WorkspaceEdit { changes: Some(changes), ..Default::default() });
        },
    };
    let fix = Some(fix);
    fix
}

/// Converts a given `Span` into a `Range`, in context of the text content given.
/// The line and column are found by the chars within the span, thus any kind
/// of character within the span is accepted
pub fn span_to_range(text: &str, span: Span) -> Range {
    let col = |line: u32, column: u32| -> u32 {
        let line_text = text.lines().nth(line.saturating_sub(1) as usize).unwrap_or("");
        // Convert it into the length of the chars
        line_text.chars()
            .take(column.saturating_sub(1) as usize)
            .map(char::len_utf16).sum::<usize>() as u32
    };
    Range::new(
        Position::new(span.start_line.saturating_sub(1), col(span.start_line, span.start_column)),
        Position::new(span.end_line.saturating_sub(1), col(span.end_line, span.end_column)),
    )
}

/// Text of a single-line range. LSP columns are UTF-16 units, not chars or bytes
pub fn text_in_range(text: &str, range: Range) -> String {
    let line = text.lines().nth(range.start.line as usize).unwrap_or("");
    let units: Vec<u16> = line.encode_utf16().collect();
    let (s, e) = (range.start.character as usize, range.end.character as usize);
    String::from_utf16_lossy(units.get(s..e.min(units.len())).unwrap_or(&[]))
}