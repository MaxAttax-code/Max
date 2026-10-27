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
exports.registerHover = registerHover;
const vscode = __importStar(require("vscode"));
const language_1 = require("./language");
function registerHover(context) {
    // Provides us the hover-info we want
    const hoverProvider = vscode.languages.registerHoverProvider("max", {
        provideHover(document, position, _token) {
            const range = document.getWordRangeAtPosition(position);
            if (!range)
                return;
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
            const declRegex = new RegExp(`\\bdeclare\\s+((?:(?:fixed|open|static)\\s+)*)${escapedWord}\\b(?:\\s+as\\s+([A-Za-z_][A-Za-z0-9_]*))?`, "g");
            const declMatch = declRegex.exec(text);
            if (declMatch) {
                const modifierNames = declMatch[1].trim() || "none";
                const varType = declMatch[2] ?? "inferred/dynamic";
                return new vscode.Hover(`**Variable**: \`${word}\`\n\n` +
                    `**Type**: \`${varType}\`\n\n` +
                    `**Modifiers**: \`${modifierNames}\``);
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
            if (language_1.keywords.includes(word)) {
                return new vscode.Hover(`**Keyword**: \`${word}\`\n\n*Max language control or declaration keyword.*`);
            }
            // 6. Check if the word is a built-in type
            if (language_1.types.includes(word)) {
                return new vscode.Hover(`**Type Definition**: \`${word}\``);
            }
            // 7. Check if the word is a built-in taskType
            if (language_1.taskTypes.includes(word)) {
                return new vscode.Hover(`**Task Type Definition**: \`${word}\``);
            }
            let globalTaskNames = Object.keys(language_1.globalTasks);
            // 7. Check if the word is a global task, thus displaying it's name and description
            if (globalTaskNames.includes(word)) {
                return new vscode.Hover(`Global task **${word}()**\n\n${language_1.globalTasks[word]}`);
            }
            return null; // No hover info found for this word
        }
    });
    // Don't forget to push it to subscriptions so it cleans up properly
    context.subscriptions.push(hoverProvider);
}
