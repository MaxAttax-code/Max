import { LanguageClient, ServerOptions, TransportKind } from "vscode-languageclient/node";
import * as vscode from 'vscode';
import * as path from 'path';

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
    const serverPath = path.join(
        context.extensionPath,   // ...\Max\max-language
        "..",                    // ...\Max
        "rust_f", "target", "debug", "max_lsp.exe" // Connect!
    );

    const server: ServerOptions = {
        command: serverPath,
        transport: TransportKind.stdio,
    };
    client = new LanguageClient("max", "Max Language Server", server, {
        documentSelector: [{ language: "max" }],
    });
    client.start();
}

export function deactivate() { return client?.stop(); }