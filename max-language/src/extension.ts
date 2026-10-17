import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from "path";

export function activate(context: vscode.ExtensionContext) {
    // Create a diagnostics bucket. This is for error linting
    const diagnostics = vscode.languages.createDiagnosticCollection("max");

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

            // 4. Check if the word is a keyword
            if (keywords.includes(word)) {
                return new vscode.Hover(`**Keyword**: \`${word}\`\n\n*Max language control or declaration keyword.*`);
            }

            // 5. Check if the word is a built-in type
            if (types.includes(word)) {
                return new vscode.Hover(`**Type Definition**: \`${word}\``);
            }

            // 6. Check if the word is a built-in taskType
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
    }

    // Now we register the code action provider
    vscode.languages.registerCodeActionsProvider("max", new MaxCodeActionProvider(), {
        providedCodeActionKinds: MaxCodeActionProvider.providedCodeActionKinds
    });

    enum TaskType {
        function,
        coroutine
    }

    enum LifecycleModifier {
        open,
        static
    }

    enum MutabilityModifier {
        fixed
    }

    interface ModifierSet {
        lifecycleModifier: LifecycleModifier | undefined;
        mutabilityModifier: MutabilityModifier | undefined;
    }

    interface TaskParam {
        name: string,
        type: string | null
    }
    
    interface BlockComment {
        start: vscode.Position,
        startOffset: number,
        endOffset?: number
    }

    interface Scope {
        startRange: vscode.Range,
        vars: Variable[]
    }

    interface Variable {
        name: string,
        modifiers: ModifierSet,
        type: string | null,
        value: string
    }

    interface Task {
        name: string,
        type: TaskType,
        parameters: TaskParam[]
    }

    /**
     * This function gives us the error linting needed for Max.
     * @param document The text document to inspect
     */
    function runLint(document: vscode.TextDocument) {


        // |‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾| \\
        // |     HELPER FUNCTIONS     | \\
        // |__________________________| \\


        /**
         * Checks if an offset (a given positional number) is within a block comment or comment
         * @param offset The offset to check is within a comment block
         * @param blocks The comment blocks to check within
         * @returns Whether the offset was in any given block comment or not
         */
        function isInsideBlockComment(offset: number, blocks: BlockComment[]): boolean {
            for (const block of blocks) {
                if (block.endOffset !== undefined &&
                    offset >= block.startOffset &&
                    offset <= block.endOffset) {
                    return true;
                }
            }
            return false;
        }

        /**
         * Checks if an offset is within any line comment
         * @param offset The offset to check, a positional number
         * @returns Whether the offset was inside any given block comment or not
         */
        function isInsideLineComment(offset: number): boolean {
            return lineComments.some(c => offset >= c.start && offset <= c.end);
        }

        // We are lazy, so let's just do both funcs in one!
        function insideAnyComment(offset: number, blocks: BlockComment[]): boolean {
            return isInsideBlockComment(offset, blocks) || isInsideLineComment(offset)
        }

        /**
         * Checks if a given name is one of the types in the types list.
         * @param name The name to check
         * @returns True if the name is a type, else false
         */
        function isTypeName(name: string): boolean {
            return types.includes(name);
        }

        /**
         * Checks if a given name is one of the iterable types in the list.
         * @param name The name to check
         * @returns True if the name is an iterable type, else false
         */
        function isIterableTypeName(name: string): boolean {
            return iterTypes.includes(name);
        }

        /**
         * Function that helps parse typing, returning if it's valid or not.
         * @param typeSpec The type name to check
         * @returns A boolean if the type is valid or not
         */
        function parseType(typeSpec: string): boolean {
            typeSpec = typeSpec.trim();

            // A simple type
            if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(typeSpec)) {
                return isTypeName(typeSpec);
            }

            // The Long-form collection using 'of'
            const longForm = /^([A-Za-z_][A-Za-z0-9_]*)\s+of\s+(.+)$/.exec(typeSpec);
            if (longForm) {
                const iterType = longForm[1];
                const inner = longForm[2];

                if (!isIterableTypeName(iterType)) return false;
                return parseType(inner); // RECURSION. Super cool right?
            }

            // Short-form collection using the '<>' symbols
            const shortForm = /^([A-Za-z_][A-Za-z0-9_]*)<(.+)>$/.exec(typeSpec);
            if (shortForm) {
                const iterType = shortForm[1];
                const inner = shortForm[2];

                if (!isIterableTypeName(iterType)) return false;
                return parseType(inner);
            }

            // 4. Anything else is invalid!
            return false;
        }

        /**
         * Parses Task parameters, also validating their typing and format. This means the 'with ()'
         * @param paramText The param text to parse. Must not be '=>'
         * @param paramsStartPos The position the parameters start from
         * @returns The parameters parsed as an array of TaskParams
         */
        function parseTaskParameters(paramText: string, paramsStartPos: number): TaskParam[] {
            const startPos = document.positionAt(paramsStartPos);
            const endPos = document.positionAt(paramsStartPos + paramText.length);

            // First we extract content inside parentheses
            const innerMatch = paramText.match(/\(([^)]*)\)/);
            if (!innerMatch) return [];

            const innerContent = innerMatch[1].trim();
            if (!innerContent) return []; // Empty parameters: with ()

            // Split 'em by comma to evaluate each parameter individually
            const rawParams = innerContent.split(',');
            const parameters: TaskParam[] = [];
            
            // Through each param found
            for (const rawParam of rawParams) {
                const trimmed = rawParam.trim();
                if (!trimmed) continue;

                // A flexible regex: requires name, 'as', and a type name optionally
                const paramMatch = trimmed.match(/^([A-Za-z_][A-Za-z0-9_]*)(?:\s+as\s+([A-Za-z_][A-Za-z0-9_]*))?$/);

                if (!paramMatch) {
                    // Report syntax error for malformed parameter structure
                    diags.push(new vscode.Diagnostic(
                        new vscode.Range(startPos, endPos), // adjust position as needed
                        `Invalid parameter syntax: "${trimmed}". Expected format: 'name' or 'name as type'`,
                        vscode.DiagnosticSeverity.Error
                    ));
                    continue;
                }

                const paramName = paramMatch[1];
                const typeName = paramMatch[2] || null; // Will be null if omitted

                // Finally, validate the type
                if (typeName !== null && !types.includes(typeName)) {
                    diags.push(new vscode.Diagnostic(
                        new vscode.Range(startPos, endPos),
                        `Unknown type "${typeName}" for parameter "${paramName}"`,
                        vscode.DiagnosticSeverity.Error
                    ));
                }

                parameters.push({ name: paramName, type: typeName });
            }

            return parameters;
        }

        
        // |‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾| \\
        // |          SETUP           | \\
        // |__________________________| \\


        const text = document.getText();
        const diags: vscode.Diagnostic[] = [];

        // Regex to capture: declare <identifier> (as <type>) = <value>(, resort <value2>)
        // BUT! In order to prevent Regex hell, we simply capture all until a newline
        // Syntaxing is handled later
        const declareRegex = /\bdeclare\b([^\n]*)/g;

        // Regex to capture: define <tasktype> <name> [with (params as types...) | =>] [return type] do
        const defineRegex = /\bdefine\s+([A-Za-z_][A-Za-z0-9_]*)\s+([A-Za-z_][A-Za-z0-9_]*)\s+(?:(with\s*\([^)]*\))|(=>))(?:\s+return\s+([A-Za-z_][A-Za-z0-9_]*))?\s+\bdo\b/g;

        // Regex for a block start and end
        const blockRegex = /\b(then|do|conclude)\b/g;

        // Regex to capture: // <comment>
        const lineCommentRegex = /\/\/.*$/gm;
        // Creating an interface? Nah, just a list of line comments that are found
        const lineComments: { start: number, end: number }[] = [];

        // Defines when a block comment starts and ends, as /// and \\\
        // They need no word boundaries ('\b') since they are not characters (which would be '\w' or '\W')
        const blockCommentStartRegex = /\/\/\//g;  // ///
        const blockCommentEndRegex = /\\\\\\/g;  // \\\

        const blockCommentStack: BlockComment[] = [];
        // Start with a stack of only one scope. This single scope should never be popped
        const scopeStack: Scope[] = [{ vars: [], startRange: new vscode.Range(document.positionAt(0), document.positionAt(text.length)) }];
        const taskStack: Task[] = [];

        let match;


        // |‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾| \\
        // |    COMMENT DETECTION     | \\
        // |__________________________| \\

        /*
        NOTE: Comments logic MUST be placed above all else, so that they are detected and found first, 
        allowing other diagnostics to be ignored if within a comment!
        */

        // For when a line comment starts and ends
        while ((match = lineCommentRegex.exec(text)) !== null) {
            lineComments.push({
                start: match.index,
                end: match.index + match[0].length // As long as the line comment is, it ends at a newline
            });
        }
        
        // For when a block comment starts and ends
        while ((match = blockCommentStartRegex.exec(text)) !== null) {
            const startPos = document.positionAt(match.index);
            blockCommentStack.push({
                start: startPos,
                startOffset: match.index + match[0].length
            });
        }

        while ((match = blockCommentEndRegex.exec(text)) !== null) {
            const endPos = document.positionAt(match.index);

            if (blockCommentStack.length === 0) {
                // No block comment to close!
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(endPos, endPos.translate(0, match[0].length)),
                    "Unexpected block comment ending; no starting block comment to end found.",
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            // This mimics normal block code
            const blockComment = blockCommentStack.pop();
            if (blockComment === undefined) { throw new Error("Block attempted to be recieved from stack did not exist."); }
            blockComment.endOffset = match.index;
            
            const blockContent = text.substring(blockComment.startOffset, blockComment.endOffset);

            // Check if block comment is only whitespace/newlines
            if (/^\s*$/.test(blockContent)) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(document.positionAt(blockComment.startOffset), endPos),
                    "Empty block comment detected — Is this on purpose? If so, add an underscore or period to remove this warning.",
                    vscode.DiagnosticSeverity.Information
                ));
            }
        }


        // |‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾| \\
        // |      SYNTAX CHECKING     | \\
        // |__________________________| \\


        // ---------------------------- \\
        // ----- DECLARATION LAWS ----- \\
        // ---------------------------- \\


        while((match = declareRegex.exec(text)) !== null) {
            const fullMatch = match[0];
            const remainder = match[1].trim();

            const startPos = document.positionAt(match.index);
            const endPos = document.positionAt(match.index + fullMatch.length);

            if (insideAnyComment(match.index, blockCommentStack)) continue;

            // Now we split the comma and resort if it exists
            const parts = remainder.split(/\s*,\s*resort/);

            const mainPart = parts[0]; // Our declaration and value
            const resortPart = parts[1]; // Our resortable, or undefined

            // Let's take this in steps!

            // ----------------------------
            // 1. Parse resort value
            // ----------------------------

            let resortValue: string | null = null;

            if (resortPart !== undefined) {
                // Get if the resort match is valid
                const resortMatch = /^\s+(.+)$/.exec(resortPart);
                if (!resortMatch) {
                    diags.push(new vscode.Diagnostic(
                        new vscode.Range(startPos, endPos),
                        "Invalid resort clause. Expected: resort <value>",
                        vscode.DiagnosticSeverity.Error
                    ));
                    continue;
                }
                resortValue = resortMatch[1];
            }

            // ----------------------------
            // 2. Parse main declaration
            // ----------------------------

            // Find the name and value, with typing in the hue!

            // name [as type] = value
            const mainMatch =
                /^((?:(?:fixed|open|static)\s+)+)?([A-Za-z_][A-Za-z0-9_]*)(?:\s+as\s+(.+?))?\s*=\s*(.+)$/.exec(mainPart);
            if (!mainMatch) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    "Invalid declaration statement",
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            const modifierText = mainMatch[1]?.trim() ?? ""; // Something or an empty string
            const identifier = mainMatch[2];
            const typeSpect = mainMatch[3]; // May be undefined
            const value = mainMatch[4];

            // ----------------------------
            // 3. Validate identifier
            // ----------------------------

            // No special stuff in there
            if (keywords.includes(identifier) || taskTypes.includes(identifier)) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    `Invalid declaration name, cannot use ${identifier}`,
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            let isDuplicate = false;

            const currentScopeIndex = scopeStack.length - 1;

            for (let i = currentScopeIndex; i >= 0; i--) {
                const existing = scopeStack[i].vars.find(
                    v => v.name === identifier
                );

                if (existing === undefined) continue;

                if (existing.modifiers.lifecycleModifier !== LifecycleModifier.open) {
                    isDuplicate = true;
                    break;
                }

                break;
            }

            if (isDuplicate) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    `Duplicate declaration name found: '${identifier}' already exists in this scope or an outer scope.`,
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            // ----------------------------
            // 4. Parse modifier
            // ----------------------------

            // Get names based on splitting whitespaces
            const modifierNames = modifierText
                ? modifierText.split(/\s+/)
                : [];

            const modifiers: ModifierSet = {
                lifecycleModifier: undefined,
                mutabilityModifier: undefined
            };

            let invalidModifiers = false;

            // Check through all the modifiers
            for (const name of modifierNames) {
                switch (name) {
                    // The lifecycle modifiers
                    case "open":
                    case "static": {
                        // Already defined? Not good!
                        if (modifiers.lifecycleModifier !== undefined) {
                            invalidModifiers = true;
                            break;
                        }

                        modifiers.lifecycleModifier =
                            name === "open"
                                ? LifecycleModifier.open
                                : LifecycleModifier.static;

                        break;
                    }
                    
                    // The mutability modifier (one for now)
                    case "fixed": {
                        if (modifiers.mutabilityModifier !== undefined) {
                            invalidModifiers = true;
                            break;
                        }

                        modifiers.mutabilityModifier =
                            MutabilityModifier.fixed;

                        break;
                    }
                }

                if (invalidModifiers) break;
            }

            if (invalidModifiers) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    "Conflicting or duplicate declaration modifiers.",
                    vscode.DiagnosticSeverity.Error
                ));

                continue;
            }

            // ----------------------------
            // 5. Parse type specification
            // ----------------------------

            if (typeSpect !== undefined) {
                // Check out `parseType` for more
                if (!parseType(typeSpect)) {
                    diags.push(new vscode.Diagnostic(
                        new vscode.Range(startPos, endPos),
                        `Invalid type specification '${typeSpect}'.`,
                        vscode.DiagnosticSeverity.Error
                    ));
                    continue;
                }
            }

            // ----------------------------
            // 6. Validate value
            // ----------------------------

            if (value.length === 0) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    `Expected a value after '='`,
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            // ----------------------------
            // 7. Validate resort value
            // ----------------------------

            if (resortValue !== null && resortValue.length === 0) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    `Expected a value after 'resort'`,
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            const wanted = scopeStack[scopeStack.length - 1].vars;
            // Find if the var already exists (of course this only applies to shadowing)
            const existingIndex = wanted.findIndex(
                v => v.name === identifier
            );

            const variable: Variable = {
                name: identifier,
                modifiers: modifiers,
                type: typeSpect,
                value: value
            };

            // Shadowing in the same scope? then just replace that variable!
            if (existingIndex !== -1) {
                wanted[existingIndex] = variable;
            } else {
                wanted.push(variable);
            }
        }


        // ---------------------------- \\
        // ------ DEFINITION LAWS ----- \\
        // ---------------------------- \\


        while ((match = defineRegex.exec(text)) !== null) {
            const fullMatch = match[0];
            const taskTypeName = match[1];
            const identifier = match[2];
            const withClause = match[3]; // e.g. "with (x as int, y as int)" or undefined
            const arrowClause = match[4] // e.g. "=>" or undefined
            const returnType = match[5]; // e.g. "return int" or undefined

            const startPos = document.positionAt(match.index);
            const endPos = document.positionAt(match.length + fullMatch.length);

            // ----------------------------
            // 1. Validate the Identifier
            // ----------------------------

            if (keywords.includes(identifier) || taskTypes.includes(identifier)) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    `Invalid task definition name, cannot use ${identifier}`,
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            // ----------------------------
            // 2. Validate the Task Type
            // ----------------------------

            let taskType: TaskType;

            switch (taskTypeName) {
                case "function": taskType = TaskType.function; break;
                case "coroutine": taskType = TaskType.coroutine; break;

                default: {
                    diags.push(new vscode.Diagnostic(
                        new vscode.Range(startPos, endPos),
                        `Invalid Task Type given: ${taskTypeName}`,
                        vscode.DiagnosticSeverity.Error
                    ));
                    continue;
                }
            }

            const clauses: string = `${withClause === undefined}:${arrowClause === undefined}`;

            let params: TaskParam[] = [];

            switch (clauses) {
                // There is an arrow clause? Just break, there's no params!
                case "true:false": {
                    break;
                }

                // There is a with clause, so check params
                case "false:true": {
                    // Very nice positioning
                    const withClauseIndex = match.index + match[0].indexOf(withClause);
                    
                    if (taskType == TaskType.coroutine) {
                        diags.push(new vscode.Diagnostic(
                            new vscode.Range(startPos, endPos),
                            `Invalid Task Definition structure. A Coroutine cannot have any parameters.`,
                            vscode.DiagnosticSeverity.Error
                        ));
                        continue;
                    }

                    params = parseTaskParameters(withClause, withClauseIndex);
                    break
                }

                default: {
                    diags.push(new vscode.Diagnostic(
                        new vscode.Range(startPos, endPos),
                        `Invalid Task Definition structure. Must have parameters or be parameterless.`,
                        vscode.DiagnosticSeverity.Error
                    ));
                    continue;
                }
            }

            if (returnType !== undefined) {
                if (!isTypeName(returnType)) {
                    diags.push(new vscode.Diagnostic(
                        new vscode.Range(startPos, endPos),
                        `Invalid return type for task definition.`,
                        vscode.DiagnosticSeverity.Error
                    ));
                    continue;
                }
            }

            taskStack.push({
                name: identifier,
                type: taskType,
                parameters: params
            })
        }


        // ---------------------------- \\
        // ----- BLOCK REGEX LAWS ----- \\
        // ---------------------------- \\


        while ((match = blockRegex.exec(text)) !== null) {
            const keyword = match[0];
            const startPos = document.positionAt(match.index);
            const endPos = document.positionAt(match.index + keyword.length);

            if (keyword === "then" || keyword === "do") {
                // Start a new scope
                scopeStack.push({
                    startRange: new vscode.Range(startPos, endPos),
                    vars: []
                });
            } else if (keyword === "conclude") {
                // End the current scope
                scopeStack.pop();
            }
        }


        // |‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾| \\
        // |    FINAL CONSISTENCY     | \\
        // |__________________________| \\


        // If more than the global scope is left (or somehow the global scope was popped)
        if (scopeStack.length < 1) {
            diags.push(new vscode.Diagnostic(
                new vscode.Range(document.positionAt(0), document.positionAt(text.length)),
                "Cannot attempt to exit out of the global scope",
                vscode.DiagnosticSeverity.Error
            ));
        }
        else if (scopeStack.length !== 1) {
            for (let unclosed of scopeStack) {
                if (scopeStack.indexOf(unclosed) === 0) {
                    continue; // Ignore the global scope
                }

                diags.push(new vscode.Diagnostic(
                    unclosed.startRange,
                    "Unclosed scope found. Close with 'conclude'.",
                    vscode.DiagnosticSeverity.Error
                ));
            }
        }

        diagnostics.set(document.uri, diags);
    }

    // Now we activate it for workspace changes
    vscode.workspace.onDidChangeTextDocument(event => {
        if (event.document.languageId == "max") { // For our language
            runLint(event.document);
        }
    })
    
    // Get the path to the native api json files
    const jsonPath = path.join(context.extensionPath, 'assets', 'native_api.json');

    let staticLibraries: Record<string, { label: string; detail: string; kind?: vscode.CompletionItemKind }[]> = {};

    // Read and parse JSON to get the data
    if (fs.existsSync(jsonPath)) {
        try {
            const rawData = fs.readFileSync(jsonPath, 'utf8');
            staticLibraries = JSON.parse(rawData);
        } catch (e) {
            console.error("Failed to parse native_api.json", e);
        }
    }

    const staticModules = Object.keys(staticLibraries);

    const keywords = [
        "using",

        "declare",
        "resort",
        "say",
        "define",

        "open",
        "static",
        "fixed",

        "if",
        "then",
        "elif",
        "else",
        "maybe",
        "conclude",

        "while",
        "for",
        "foreach",
        "do",
        
        "true",
        "false",
        "possible",

        "nil",
        "as",
        "to",
        "of",
        "in",
        "with",
        "return",
        "yield"
    ]

    const operators = [
        "not",
        "and",
        "or",
        "step"
    ]

    const types = [
        "int",
        "float",
        "string",
        "boolean",
        "list",
        "dictionary"
    ]

    const iterTypes = [
        "list",
        "dictionary"
    ]

    const taskTypes = [
        "function",
        "coroutine"
    ]

    const globalTasks: Record<string, string> = {
        "input": `Accepts an input from the user using stdin.
                  This only accepts valid UTF-8 characters, and whitespaces at the beginning and end are trimmed.
                  An optional string output message can be given, which will be printed before accepting an input.`,
        "when": `Intakes a possible and a lambda condition, applying the condition to the possible.
                 Should the lambda condition ever change, the possible value will be reevaluated.
                 To stop condition-evaluation for the given possible, use freeze().\n
                 Using multiple when() on a possible performs boolean comparisons between the boolean.\n`,
        "freeze": `Freezes a possible so long as it has a condition attached and has been resolved.
                   Attempting to freeze a possible that has not been evaluated will error.`,
        "error": `Calls an error on the machine, printing out a message if given, and exitting the terminal.`
    }

    const functionRegex = /define\s+function\s+([A-Za-z_][A-Za-z0-9_]*)\s+with\s*\(([^)]*)\)/g;
    const coroutineRegex = /define\s+coroutine\s+([A-Za-z_][A-Za-z0-9_]*)\s+=>\s/g;

    // Function to parse the params of a task, returning it as a dictionary
    function parseParams(paramString: string) {
        const params: { name: string; type: string }[] = [];
        const parts = paramString.split(",").map(p => p.trim());

        for (const part of parts) {
            const match = part.match(/([A-Za-z_][A-Za-z0-9_]*)\s+as\s+([A-Za-z_][A-Za-z0-9_]*)/);
            if (match) {
                params.push({ name: match[1], type: match[2].toLowerCase() });
            }
        }
        return params;
    }

    // Just checks if we are inside the function params, and thus get the param name and type
    function isInsideFunction(document: vscode.TextDocument, position: vscode.Position) {
        const text = document.getText();
        let match;

        while ((match = functionRegex.exec(text)) !== null) {
            const start = document.positionAt(match.index);
            const params = parseParams(match[2]);

            // Find the end of the function block
            const endIndex = text.indexOf("conclude", match.index);
            const end = document.positionAt(endIndex);

            if (position.isAfter(start) && position.isBefore(end)) {
                return { params };
            }
        }

        return null;
    }

    const provider = vscode.languages.registerCompletionItemProvider(
        { language: "max" },

        {
            provideCompletionItems(document: vscode.TextDocument, position: vscode.Position) {
                // Some pre-written stuff...
                let globalTaskNames: string[] = Object.keys(globalTasks);
                // --- \\

                const line = document.lineAt(position).text;
                const textBeforeCursor = line.substring(0, position.character);
                const text = document.getText();

                // 1. Detect member access
                const memberAccessMatch = textBeforeCursor.match(/([A-Za-z_][A-Za-z0-9_]*)\.\s*$/);

                // 2. Standard completions (Keywords, Variables, Functions)
                const completions: vscode.CompletionItem[] = [];

                const funcScope = isInsideFunction(document, position);

                // When inside the function params
                if (funcScope) {
                    for (const p of funcScope.params) {
                        completions.push(new vscode.CompletionItem(
                            p.name,
                            vscode.CompletionItemKind.Variable
                        ));
                    }
                }
                
                if (memberAccessMatch) {
                    const targetVar = memberAccessMatch[1];

                    // For static modules
                    if (staticLibraries[targetVar]) {
                        return staticLibraries[targetVar].map(item => {
                            const completion = new vscode.CompletionItem(
                                item.label, 
                                item.kind ?? vscode.CompletionItemKind.Method
                            );
                            completion.detail = item.detail;
                            return completion;
                        });
                    }

                    // Check function/coroutine parameters
                    if (funcScope) {
                        const param = funcScope.params.find(p => p.name === targetVar);
                        if (param) {
                            return getNativeMethodsForType(param.type);
                        }
                    }
                    
                    // Check if the target variable was declared with an explicit type (using 'as')
                    const declRegex = new RegExp(`\\bdeclare\\s+((?:(?:fixed|open|static)\\s+)*)${targetVar}\\b(?:\\s+as\\s+([A-Za-z_][A-Za-z0-9_]*))?`, 'g');
                    const typeMatch = declRegex.exec(text);

                    // When the typing matches
                    if (typeMatch) {
                        const varType = typeMatch[2].toLowerCase();
                        return getNativeMethodsForType(varType);
                    }

                    // Check if the target variable is a defined coroutine
                    const coroCheckRegex = new RegExp(`\\bdefine\\s+coroutine\\s+${targetVar}\\b`, 'g');
                    if (coroCheckRegex.test(text)) {
                        // Specifically for coroutine instances
                        return getNativeMethodsForType("coroutineInstance");
                    }

                    // Just return the general stuff (to be added)
                    return [];
                }

                // Match variables by detecting 'declare'
                const declarationRegex = /\bdeclare\s+([A-Za-z_][A-Za-z0-9_]*)/g;
                let match;
                while ((match = declarationRegex.exec(text)) !== null) {
                    completions.push(new vscode.CompletionItem(match[1], vscode.CompletionItemKind.Variable));
                }

                // Match functions by detecting 'define' followed by 'function'
                while ((match = functionRegex.exec(text)) !== null) {
                    const item = new vscode.CompletionItem(match[1], vscode.CompletionItemKind.Function);
                    item.detail = "User-defined function";
                    completions.push(item);
                }
                
                // Same for coroutine
                while ((match = coroutineRegex.exec(text)) !== null) {
                    const item = new vscode.CompletionItem(match[1], vscode.CompletionItemKind.Variable);
                    item.detail = "User-defined coroutine";
                    completions.push(item);
                }

                // Detect if the user typed 'define ' to auto-suggest task types
                if (/\bdefine\s+$/.test(textBeforeCursor)) {
                    return taskTypes.map(tt => {
                        const item = new vscode.CompletionItem(tt, vscode.CompletionItemKind.Class);
                        item.detail = "Task type definition";
                        return item;
                    });
                }

                // Return a map of all these key things with to vscode Completion
                return [
                    ...keywords.map(k => new vscode.CompletionItem(k, vscode.CompletionItemKind.Keyword)),
                    ...operators.map(o => new vscode.CompletionItem(o, vscode.CompletionItemKind.Operator)),
                    ...types.map(t => new vscode.CompletionItem(t, vscode.CompletionItemKind.TypeParameter)),
                    ...taskTypes.map(tt => new vscode.CompletionItem(tt, vscode.CompletionItemKind.Class)),
                    ...globalTaskNames.map(gt => new vscode.CompletionItem(gt, vscode.CompletionItemKind.Function)),
                    ...staticModules.map(m => new vscode.CompletionItem(m, vscode.CompletionItemKind.Module)),
                    ...completions
                ];
            }
        },
        " ", "." // Trigger with a space or dot character
    );

    /*--
        A function that gives specific native functions or properties to specific types in Max
        Does so by keeping a record of types and their corresponding native tasks (properties are attained through getter tasks)
    --*/
    function getNativeMethodsForType(type: string): vscode.CompletionItem[] {
        const methods: Record<string, { label: string; detail: string }[]> = {
            list: [
                { label: "add(item)", detail: "Appends an item to the list" },
                { label: "addRange(items...)", detail: "Appends all the items inputted as arguments into the list" },
                { label: "concatenate(item)", detail: "Concatenates two lists together, pushing all the second lists elements to the first list" },
                { label: "combine(item)", detail: "Concatenates two lists together, but prevents duplicate overlapping" },
                { label: "insert(index, item)", detail: "Appends an item to the list at a specified index" },
                { label: "removeAt(index)", detail: "Removes item at specified index" },
                { label: "trimExcess(count)", detail: "Individually removes the elements at the end of the list until the list reaches the given count. If the list is smaller than the given count, it does nothing" },
                { label: "copy()", detail: "Returns the entire list values for another instance to copy from. Does not set type specifications" },
                { label: "contains(item)", detail: "Checks if item exists" },
                { label: "length", detail: "Returns total number of elements" },
                { label: "clear()", detail: "Removes all elements" },
                { label: "reverse()", detail: "Reverses all the elements by their index in-place" },
                { label: "join(seperator)", detail: "Join all elements in the list with the seperator" },
                { label: "map(predicate)", detail: "Performs a predicate lambda upon each element within the list" },
                { label: "filter(predicate)", detail: "Performs a predicate lambda upon each element in the list, removing those that do not pass" },
                { label: "getEnumerator()", detail: "Enumerates the list, returning a new dictionary with the indexes as the keys, and list items as the values. Type Specification is also provided" },
                { label: "findIndex()", detail: "Gets the index of the first instance of the given value found. This returns -1 if nothing was found" },
                { label: "isEmpty", detail: "Returns if the list is empty" },
                { label: "first", detail: "Returns the first element" },
                { label: "last", detail: "Returns the last element" }
            ],
            dictionary: [
                { label: "add(key, value)", detail: "Appends a key-value pair to the list. This checks for duplicates if needed" },
                { label: "containsKey(key)", detail: "Checks if key exists" },
                { label: "remove(key)", detail: "Removes key-value pair" },
                { label: "copy()", detail: "Returns the entire dictionary values for another instance to copy from. Sets both type specifications and allows duplicates if needed" },
                { label: "softCopy()", detail: "Returns the entire dictionary values for another instance to copy from. Does not set type specifications, but allows duplicates if needed" },
                { label: "keys", detail: "Returns a list of keys" },
                { label: "values", detail: "Returns a list of values" },
                { label: "clear()", detail: "Removes all entries" },
                { label: "allowDuplicates(boolean)", detail: "Declare if dictionary can have duplicate values. Keep in mind that reassigning a variable a new dictionary resets this to false" },
                { label: "length", detail: "Returns the total number of entries" },
                { label: "isEmpty", detail: "Returns if the dictionary is empty" }
            ],
            string: [
                { label: "toUpper()", detail: "Converts string to uppercase" },
                { label: "toLower()", detail: "Converts string to lowercase" },
                { label: "trim()", detail: "Trims whitespace" },
                { label: "contains(sub)", detail: "Returns if the substring is present inside the string" },
                { label: "replace(search, replacer)", detail: "Replaces any found instance of the searchable string with the replacer" },
                { label: "format(formatters)", detail: "Replaces each instance of '||' within the string with the given formatters in argument order" },
                { label: "length", detail: "Returns length of string" },
                { label: "isEmpty", detail: "Returns if the string is empty" },
                { label: "split(delimiter)", detail: "Splits the string by a delimiter and returns a List of substrings" },
                { label: "toCharacters()", detail: "Returns a list of all the characters in the string" }
            ],
            int: [
                { label: "abs()", detail: "Returns absolute value" },
                { label: "toString()", detail: "Converts integer to string" },
                { label: "clamp(min, max)", detail: "Clamps the integer between a minimum and maximum threshold" },
                { label: "isEven", detail: "Returns if the number is an even integer" },
                { label: "isOdd", detail: "Returns if the number is an odd integer" }
            ],
            float: [
                { label: "round(decimalPlaces)", detail: "Rounds double to nearest decimal place provided" },
                { label: "abs()", detail: "Returns absolute value" },
                { label: "clamp(min, max)", detail: "Clamps the float between a minimum and maximum threshold" },
                { label: "decimalPlaces", detail: "Returns the number of decimal places in the float" }
            ],
            boolean: [
                { label: "then(lambda)", detail: "If the boolean is true, proceed to run the lambda" },
                { label: "else(lambda)", detail: "If the boolean is false, proceed to run the lambda" },
                { label: "match(trueLambda, falseLambda)", detail: "Creates two branch lambdas for whether the boolean is true or false respectively" },
                { label: "expect(optMsg)", detail: "If the boolean is false, it errs, with an optional given message" },
            ],
            coroutineInstance: [
                { label: "run()", detail: "Runs the coroutine, if idle or paused" },
                { label: "pause()", detail: "Pauses the coroutine where it currently is" },
                { label: "cancel()", detail: "Cancels the coroutine, restarting it's placement to the beginning" },
                { label: "terminate()", detail: "Terminates the coroutine, killing it and thus leaving an uninvokable instance" },
            ]
        };

        const typeMethods = methods[type] || [];
        return typeMethods.map(m => {
            const item = new vscode.CompletionItem(m.label, vscode.CompletionItemKind.Method);
            item.detail = m.detail;
            return item;
        });
    }


    context.subscriptions.push(provider);
}