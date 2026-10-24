import * as vscode from "vscode";
import { keywords, types, taskTypes, globalTasks } from "./language";

export function registerHover(context: vscode.ExtensionContext) {
    // Provides us the hover-info we want
    const hoverProvider = vscode.languages.registerHoverProvider("max", {
        provideHover(document: vscode.TextDocument, position: vscode.Position, _token: vscode.CancellationToken) {
            const range = document.getWordRangeAtPosition(position);
            if (!range) return;

            const word = document.getText(range);
            const text = document.getText();

            // 1. Check if the word is preceded by a member access dot, that way we know it's incorrect to provide basic hover for
            const line = document.lineAt(position.line).text;
            const textBeforeWord = line.substring(0, range.start.character);
            if (/\.\s*$/.test(textBeforeWord)) {
                return null; // Suppress hover for member properties/methods
            }

            // 2. Check if the word is a declared variable (new RegExp so we can include the word within)
            // Check whether the word is a declared variable.
            const escapedWord = word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

            const declRegex = new RegExp(
                `\\bdeclare\\s+((?:(?:fixed|open|static)\\s+)*)${escapedWord}\\b(?:\\s+as\\s+([A-Za-z_][A-Za-z0-9_]*))?`, "g"
            );

            const declMatch = declRegex.exec(text);

            if (declMatch) {
                const modifierNames = declMatch[1].trim() || "none";
                const varType = declMatch[2] ?? "inferred/dynamic";

                return new vscode.Hover(
                    `**Variable**: \`${word}\`\n\n` +
                    `**Type**: \`${varType}\`\n\n` +
                    `**Modifiers**: \`${modifierNames}\``
                );
            }

            // 3. Check if the word is a user-defined function or coroutine
            const defRegex = new RegExp(`\\bdefine\\s+(function|coroutine)\\s+${word}\\b`, 'g');
            const defMatch = defRegex.exec(text);
            if (defMatch) {
                const taskKind = defMatch[1];
                return new vscode.Hover(`**User-Defined ${taskKind}**: \`${word}\``);
            }

            // 4. Check if the word is a user-defined enum
            const enumRegex = new RegExp(`\\benum\\s+${word}\\s+members`);
            const enumMatch = enumRegex.exec(text);
            if (enumMatch) {
                return new vscode.Hover(`**User-defined Enum**: \`${word}\``);
            }

            // 5. Check if the word is a keyword
            if (keywords.includes(word)) {
                return new vscode.Hover(`**Keyword**: \`${word}\`\n\n*Max language control or declaration keyword.*`);
            }

            // 6. Check if the word is a built-in type
            if (types.includes(word)) {
                return new vscode.Hover(`**Type Definition**: \`${word}\``);
            }

            // 7. Check if the word is a built-in taskType
            if (taskTypes.includes(word)) {
                return new vscode.Hover(`**Task Type Definition**: \`${word}\``);
            }

            let globalTaskNames = Object.keys(globalTasks);

            // 7. Check if the word is a global task, thus displaying it's name and description
            if (globalTaskNames.includes(word)) {
                return new vscode.Hover(`Global task **${word}()**\n\n${globalTasks[word]}`);
            }

            return null; // No hover info found for this word
        }
    });

    // Don't forget to push it to subscriptions so it cleans up properly
    context.subscriptions.push(hoverProvider);
}