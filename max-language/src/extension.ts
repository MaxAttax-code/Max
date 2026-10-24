import * as vscode from 'vscode';
import { registerHover } from "./hover";
import { registerCompletion } from "./completion";
import { registerCodeActions } from "./codeActions";
import { registerLint } from "./lint";

export function activate(context: vscode.ExtensionContext) {
    registerHover(context);
    registerCompletion(context);
    registerCodeActions(context);
    registerLint(context);
}