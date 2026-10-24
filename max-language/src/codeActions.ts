import * as vscode from "vscode";
import { toPascalCase } from "./lint";

export function registerCodeActions(context: vscode.ExtensionContext) {
    /**
     * A class that implements the vscode CodeActionProvider interface.
     * It enables quick fixing, refactoring and source actions.
     * This class should be created and registered into VS codes CodeActionProvider
     */
    class MaxCodeActionProvider implements vscode.CodeActionProvider {
        public static readonly providedCodeActionKinds = [
            vscode.CodeActionKind.QuickFix
        ];

        provideCodeActions(
            document: vscode.TextDocument,
            _range: vscode.Range,
            context: vscode.CodeActionContext
        ): vscode.CodeAction[]
        {
            const fixes: vscode.CodeAction[] = [];

            for (const diag of context.diagnostics) {
                const msg = diag.message;

                if (msg.includes("Expected a value after '='")) {
                    fixes.push(this.createPlaceholderValueFix(document, diag));
                }

                if (msg.includes("in PascalCase")) {
                    fixes.push(this.createPascalCaseFix(document, diag))
                }

                // Add more cases as we build more diagnostics
            }

            return fixes;
        }

        private createPlaceholderValueFix(document: vscode.TextDocument, diag: vscode.Diagnostic) {
            const fix = new vscode.CodeAction("Insert placeholder nil", vscode.CodeActionKind.QuickFix);
            fix.edit = new vscode.WorkspaceEdit();

            const range = diag.range;
            fix.edit.replace(document.uri, range, "nil");

            fix.diagnostics = [diag];
            return fix;
        }

        private createPascalCaseFix(document: vscode.TextDocument, diag: vscode.Diagnostic) {
            const fix = new vscode.CodeAction("Refactor identifier into PascalCase", vscode.CodeActionKind.QuickFix);
            fix.edit = new vscode.WorkspaceEdit();

            const range = diag.range;
            fix.edit.replace(document.uri, range, toPascalCase(document.getText(range)));

            fix.diagnostics = [diag];
            return fix;
        }
    }

    // Now we register the code action provider
    vscode.languages.registerCodeActionsProvider("max", new MaxCodeActionProvider(), {
        providedCodeActionKinds: MaxCodeActionProvider.providedCodeActionKinds
    });
}