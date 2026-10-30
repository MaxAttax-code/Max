"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.registerCodeActions = registerCodeActions;
const vscode = __importStar(require("vscode"));
const lint_1 = require("./lint");
function registerCodeActions(context) {
    /**
     * A class that implements the vscode CodeActionProvider interface.
     * It enables quick fixing, refactoring and source actions.
     * This class should be created and registered into VS codes CodeActionProvider
     */
    class MaxCodeActionProvider {
        provideCodeActions(document, _range, context) {
            const fixes = [];
            for (const diag of context.diagnostics) {
                const msg = diag.message;
                if (msg.includes("Expected a value after '='")) {
                    fixes.push(this.createPlaceholderValueFix(document, diag));
                }
                if (msg.includes("in PascalCase")) {
                    fixes.push(this.createPascalCaseFix(document, diag));
                }
                if (msg.includes("Infinite for loop detected")) {
                    // Branch to what the case of infinite loop is
                    if (msg.includes("step value of 0")) {
                        fixes.push(this.createStepOneFix(document, diag));
                        fixes.push(this.createStepNegOneFix(document, diag));
                    }
                    if (msg.includes("positive step value")) {
                        fixes.push(this.createStepOneFix(document, diag));
                    }
                    if (msg.includes("negative step value")) {
                        fixes.push(this.createStepNegOneFix(document, diag));
                    }
                }
                if (msg.includes("Max-reserved keyword")) {
                    fixes.push(this.createLetteriseFix(document, diag));
                    fixes.push(this.createAddUnderscoreFix(document, diag));
                }
                // Add more cases as we build more diagnostics
            }
            return fixes;
        }
        createPlaceholderValueFix(document, diag) {
            const fix = new vscode.CodeAction("Insert placeholder nil", vscode.CodeActionKind.QuickFix);
            fix.edit = new vscode.WorkspaceEdit();
            const range = diag.range;
            fix.edit.replace(document.uri, range, "nil");
            fix.diagnostics = [diag];
            return fix;
        }
        createPascalCaseFix(document, diag) {
            const fix = new vscode.CodeAction("Refactor identifier into PascalCase", vscode.CodeActionKind.QuickFix);
            fix.edit = new vscode.WorkspaceEdit();
            const range = diag.range;
            fix.edit.replace(document.uri, range, (0, lint_1.toPascalCase)(document.getText(range)));
            fix.diagnostics = [diag];
            return fix;
        }
        createLetteriseFix(document, diag) {
            const fix = new vscode.CodeAction("Refactor identifier into letters-structure", vscode.CodeActionKind.QuickFix);
            fix.edit = new vscode.WorkspaceEdit();
            const range = diag.range;
            fix.edit.replace(document.uri, range, (0, lint_1.toLetter)(document.getText(range)));
            fix.diagnostics = [diag];
            return fix;
        }
        createStepOneFix(document, diag) {
            const fix = new vscode.CodeAction("Replace with step 1", vscode.CodeActionKind.QuickFix);
            fix.edit = new vscode.WorkspaceEdit();
            const range = diag.range;
            fix.edit.replace(document.uri, range, (1).toString());
            fix.diagnostics = [diag];
            return fix;
        }
        createStepNegOneFix(document, diag) {
            const fix = new vscode.CodeAction("Replace with step -1", vscode.CodeActionKind.QuickFix);
            fix.edit = new vscode.WorkspaceEdit();
            const range = diag.range;
            fix.edit.replace(document.uri, range, (-1).toString());
            fix.diagnostics = [diag];
            return fix;
        }
        createAddUnderscoreFix(document, diag) {
            const fix = new vscode.CodeAction("Add underscore to beginning", vscode.CodeActionKind.QuickFix);
            fix.edit = new vscode.WorkspaceEdit();
            const range = diag.range;
            fix.edit.insert(document.uri, range.start, "_");
            fix.diagnostics = [diag];
            return fix;
        }
    }
    MaxCodeActionProvider.providedCodeActionKinds = [
        vscode.CodeActionKind.QuickFix
    ];
    // Now we register the code action provider
    vscode.languages.registerCodeActionsProvider("max", new MaxCodeActionProvider(), {
        providedCodeActionKinds: MaxCodeActionProvider.providedCodeActionKinds
    });
}
