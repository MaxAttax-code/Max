import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from "path";

export function activate(context: vscode.ExtensionContext) {
    // Create a diagnostics bucket. This is for error linting
    const diagnostics = vscode.languages.createDiagnosticCollection("max");

    /**
     * A class that implements the vscode CodeActionProvider interface.
     * It enables quick fixing, refactoring and source actions.
     * This class should be 
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

                if (msg.includes("Expected a variable name after 'declare'")) {
                    fixes.push(this.createInsertPlaceholderVarFix(document, diag));
                }

                if (msg.includes("Unexpected 'conclude'")) {
                    fixes.push(this.createRemoveConcludeFix(document, diag))
                }

                if (msg.includes("Empty block detected")) {
                    fixes.push(this.createInsertPlaceholderInsideBlockFix(document, diag));
                }

                // Add more cases as you build more diagnostics
            }

            return fixes;
        }

        private createInsertPlaceholderVarFix(document: vscode.TextDocument, diag: vscode.Diagnostic): vscode.CodeAction {
            const fix = new vscode.CodeAction("Insert placeholder variable", vscode.CodeActionKind.QuickFix);
            fix.edit = new vscode.WorkspaceEdit();

            const insertPos = diag.range.end;
            fix.edit.insert(document.uri, insertPos, " placeholder");

            fix.diagnostics = [diag];
            return fix;
        }

        private createRemoveConcludeFix(document: vscode.TextDocument, diag: vscode.Diagnostic): vscode.CodeAction {
            const fix = new vscode.CodeAction("Remove 'conclude'", vscode.CodeActionKind.QuickFix);
            fix.edit = new vscode.WorkspaceEdit();

            // This time, we REMOVING the range!
            fix.edit.delete(document.uri, diag.range);

            fix.diagnostics = [diag];
            return fix;
        }

        private createInsertPlaceholderInsideBlockFix(document: vscode.TextDocument, diag: vscode.Diagnostic): vscode.CodeAction {
            const fix = new vscode.CodeAction("Insert empty say statement inside block", vscode.CodeActionKind.QuickFix);
            fix.edit = new vscode.WorkspaceEdit();

            // Insert into it the empty say statement on the next line!
            const insertPos = diag.range.start.translate(1, 0);
            fix.edit.insert(document.uri, insertPos, "say \"\"");

            fix.diagnostics = [diag];
            return fix;
        }
    }

    // Now we register the code action provider
    vscode.languages.registerCodeActionsProvider("max", new MaxCodeActionProvider(), {
        providedCodeActionKinds: MaxCodeActionProvider.providedCodeActionKinds
    });

    /**
     * Defines what kind of block type has begun
     */
    enum BlockType {
        do,
        then
    }

    /**
     * An interface that determines where a block code in Max starts. Marked by 'then' or 'do'.
     */
    interface BlockEntry {
        start: vscode.Position,
        startOffset: number,
        type: BlockType
    }
    
    interface BlockComment {
        start: vscode.Position,
        startOffset: number,
        endOffset?: number
    }

    /**
     * This function gives us the error linting needed for Max.
     * @param document The text document to inspect
     */
    function runLint(document: vscode.TextDocument) {


        // ---------------------------- \\
        // ----- HELPER FUNCTIONS ----- \\
        // ---------------------------- \\


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

        function isNumeric(str: string): boolean {
            return /^[0-9]+$/.test(str);
        }

        function isValidIdentifier(str: string): boolean {
            return /^[A-Za-z_][A-Za-z0-9_]*$/.test(str);
        }

        
        // ---------------------------- \\
        // ---------------------------- \\
        // ---------------------------- \\


        const text = document.getText();
        const diags: vscode.Diagnostic[] = [];

        // Regex to capture: declare <identifier>
        const declareRegex = /\bdeclare\s+([A-Za-z_][A-Za-z0-9_]*)?/g;

        // Regex to capture: define <taskType> <identifier>
        const defineRegex = /\bdefine\s+([A-Za-z_][A-Za-z0-9_]*)?\s+([A-Za-z_][A-Z-a-z0-9_]*)?/g;

        // Define when a block starts and ends
        const blockStartRegex = /\b(do|then)\b/g;
        const blockEndRegex = /\bconclude\b/g;

        // Regex to capture: if/elif <condition> then
        const ifStartRegex = /\bif\b([^]*)/g;
        const elifRegex = /\belif\b([^]*)/g;

        // Regex to capture: <loop_keyword> <condition> do
        const loopStartRegex = /\b(while|for|foreach)\b([^]*)/g;

        // Regex to capture: <identifier> = <number/identifier> to <number/identifier> [OPTIONAL>>], step <number/identifier>
        // Yo that's REALLY long!
        const forloopInitialiserRegex =
            /\bfor\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*([A-Za-z_][A-Za-z0-9_]+)\s+to\s+([A-Za-z_][A-Za-z0-9_]+)(?:\s*,\s*step\s+([A-Za-z_][A-Za-z0-9_]+))?\s+do\b/g;

        // Regex to capture: // <comment>
        const lineCommentRegex = /\/\/.*$/gm;
        // Creating an interface? Nah, just a list of line comments that are found
        const lineComments: { start: number, end: number }[] = [];

        // Defines when a block comment starts and ends
        // They need no word boundaries ('\b') since they are not characters (which would be '\w' or '\W')
        const blockCommentStartRegex = /\/\/\//g;
        const blockCommentEndRegex = /\\\\\\/g;

        const blockStack: BlockEntry[] = [];
        const blockCommentStack: BlockComment[] = [];

        let match;

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

        while ((match = declareRegex.exec(text)) !== null) {
            // The full match
            const fullMatch = match[0];
            // The identifier to be found
            const identifier = match[1];
            const startPos = document.positionAt(match.index);
            const endPos = document.positionAt(match.index + fullMatch.length);

            if (insideAnyComment(match.index, blockCommentStack)) continue;

            // If there is no identifier name, uh oh!
            if (!identifier) {
                // Inside a comment? SKIP!
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    "Expected a variable name after 'declare'.",
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            // We can not allow keywords either!
            if (keywords.includes(identifier) || taskTypes.includes(identifier)) {
                // Yeah you're gonna see this everywhere

                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    `Cannot use keyword '${identifier}' as a variable name.`,
                    vscode.DiagnosticSeverity.Error
                ));
            }
        }
        
        while ((match = defineRegex.exec(text)) !== null) {
            const fullMatch = match[0];
            const taskType = match[1];
            const identifier = match[2];
            const startPos = document.positionAt(match.index);
            const endPos = document.positionAt(match.index + fullMatch.length);
        
            if (insideAnyComment(match.index, blockCommentStack)) continue;

            if (!taskType) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    "Expected a task type after 'define'.",
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }
            
            if (!taskTypes.includes(taskType)) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    "Expected a valid task type for a task definition.",
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }
            
            if (!identifier) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    "Expected a task name for a task definition.",
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            if (keywords.includes(identifier) || taskTypes.includes(identifier)) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    `Cannot use keyword '${identifier}' as a task name.`,
                    vscode.DiagnosticSeverity.Error
                ));
            }
        }

        // When a block starts
        while ((match = blockStartRegex.exec(text)) !== null) {
            let typeOf: BlockType;
            switch (text) {
                case "then": typeOf = BlockType.then; break;
                case "do": typeOf = BlockType.do; break;
                default: throw new Error(`Invalid block started found: ${text}`);
            }

            const startPos = document.positionAt(match.index);
            blockStack.push({
                start: startPos,
                startOffset: match.index + match[0].length, // This finds the offset to reach the position AFTER the keyword!
                type: typeOf
            });
        }

        // Find where the block ends
        while ((match = blockEndRegex.exec(text)) !== null) {
            const endPos = document.positionAt(match.index);

            if (blockStack.length === 0) {
                if (insideAnyComment(match.index, blockCommentStack)) continue;
                // No block to close!
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(endPos, endPos.translate(0, match[0].length)),
                    "Unexpected 'conclude' — no matching 'do' or 'then' block.",
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            // Correct match — pop it, Candy Crush Style
            const block = blockStack.pop();
            if (block === undefined) { throw new Error("Block attempted to be recieved from stack did not exist."); }
            const startOffset = block.startOffset; const endOffset = match.index;
            
            // Extract the block's content (excluding the keywords!)
            const blockContent = text.substring(startOffset, endOffset);

            // Check if blockContent is only whitespace/newlines
            if (/^\s*$/.test(blockContent)) {
                if (insideAnyComment(match.index, blockCommentStack)) continue;
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(document.positionAt(startOffset), endPos),
                    "Empty block detected — contains only whitespace or newlines.",
                    vscode.DiagnosticSeverity.Warning
                ));
            }
        }

        // When an if statement starts
        while ((match = ifStartRegex.exec(text)) !== null) {
            const fullMatch = match[0];
            const startPos = document.positionAt(match.index);
            const endPos = document.positionAt(match.index + fullMatch.length);

            if (insideAnyComment(match.index, blockCommentStack)) continue;
            
            if (!fullMatch.includes("then")) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    "If statement must have keyword 'then' following it's condition.",
                    vscode.DiagnosticSeverity.Error
                ));
            }

            // If there is no condition after 'if'
            if (/^\bif\b\s*$/.test(fullMatch)) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    "Expected a condition after 'if'.",
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            // 'then' exists, but no condition is put
            if (/^\bif\s+then\b/.test(fullMatch)) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    "Expected a condition before 'then'.",
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }
        }

        // An elif branch
        while ((match = elifRegex.exec(text)) !== null) {
            const fullMatch = match[0];
            const startPos = document.positionAt(match.index);
            const endPos = document.positionAt(match.index + fullMatch.length);

            if (insideAnyComment(match.index, blockCommentStack)) continue;

            // Any thens found? Fill it up!
            if (blockStack[-1].type == BlockType.then) {
                blockStack.pop();
            }

            // No 'then' to follow ever found?
            if (blockStack.length === 0 || !blockStack.some((entry) => entry.type == BlockType.then)) {
                if (insideAnyComment(match.index, blockCommentStack)) continue;
                // No block to close!
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(endPos, endPos.translate(0, match[0].length)),
                    "Unexpected 'elif' — no following 'then' block.",
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            if (!fullMatch.includes("then")) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    "Elif branch must have keyword 'then' following it's condition.",
                    vscode.DiagnosticSeverity.Error
                ));
            }

            // If there is no condition after 'if'
            if (/^\belif\b\s*$/.test(fullMatch)) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    "Expected a condition after 'elif'.",
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            // 'then' exists, but no condition is put
            if (/^\belif\s+then\b/.test(fullMatch)) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    "Expected a condition before 'then'.",
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }
        }

        // When loop statement begins
        while ((match = loopStartRegex.exec(text)) !== null) {
            const fullMatch = match[0];
            // How convenient!
            const loopKeyword = match[1];

            const startPos = document.positionAt(match.index);
            const endPos = document.positionAt(match.index + fullMatch.length);

            if (insideAnyComment(match.index, blockCommentStack)) continue;

             // Missing "do"
            if (!fullMatch.includes("do")) {
                let msg = "";

                switch (loopKeyword) {
                    case "while":
                        msg = "A 'while' loop must end with 'do' to begin its block.";
                        break;
                    case "for":
                        msg = "A 'for' loop must end with 'do' after its initialisation.";
                        break;
                    case "foreach":
                        msg = "A 'foreach' loop must end with 'do' after specifying the iteration target.";
                        break;
                }

                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    msg,
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            // Missing condition/initialiser
            if (/^\b(while|for|foreach)\b\s*$/.test(fullMatch)) {
                let msg = "";

                switch (loopKeyword) {
                    case "while":
                        msg = "Expected a condition after 'while'.";
                        break;
                    case "for":
                        msg = "Expected an initialisation after 'for'.";
                        break;
                    case "foreach":
                        msg = "Expected an iteration target after 'foreach'.";
                        break;
                }

                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    msg,
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }

            // "do" exists but no condition/initialiser
            if (/^\b(while|for|foreach)\s+do\b/.test(fullMatch)) {
                let msg = "";

                switch (loopKeyword) {
                    case "while":
                        msg = "Expected a condition before 'do' in a 'while' loop.";
                        break;
                    case "for":
                        msg = "Expected an initialisation before 'do' in a 'for' loop.";
                        break;
                    case "foreach":
                        msg = "Expected an iteration target before 'do' in a 'foreach' loop.";
                        break;
                }

                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    msg,
                    vscode.DiagnosticSeverity.Error
                ));
                continue;
            }
        }

        // Detect a for loop initialisation
        while ((match = forloopInitialiserRegex.exec(text)) !== null) {
            const loopVar = match[1];
            const startVal = match[2];
            const endVal = match[3];
            const stepVal = match[4];

            const startPos = document.positionAt(match.index);
            const endPos = document.positionAt(match.index + match[0].length);

            if (insideAnyComment(match.index, blockCommentStack)) continue;

            // Detect malformed identifiers. No, not malinformed!
            if (!isValidIdentifier(loopVar)) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    `Invalid loop variable '${loopVar}'.`,
                    vscode.DiagnosticSeverity.Error
                ));
            }

            // Detect malformed start value
            if (!isValidIdentifier(startVal) && !isNumeric(startVal)) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    `Invalid start value '${startVal}' in 'for' loop.`,
                    vscode.DiagnosticSeverity.Error
                ));
            }

            // Detect malformed end value
            if (!isValidIdentifier(endVal) && !isNumeric(endVal)) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    `Invalid end value '${endVal}' in 'for' loop.`,
                    vscode.DiagnosticSeverity.Error
                ));
            }

            // If we are given number start/end/step values, we can also highlight that!
            if (isNumeric(startVal) && isNumeric(endVal) && isNumeric(stepVal)) {
                let startNum = parseInt(startVal);
                let endNum = parseInt(endVal);
                let stepNum = parseInt(stepVal);

                if (startNum > endNum && stepNum > 0) {
                    diags.push(new vscode.Diagnostic(
                        new vscode.Range(startPos, endPos),
                        "The start value is higher than the end value, whilst the step value is positive.",
                        vscode.DiagnosticSeverity.Warning
                    ))
                }

                if (startNum < endNum && stepNum < 0) {
                    diags.push(new vscode.Diagnostic(
                        new vscode.Range(startPos, endPos),
                        "The start value is lower than the end value, whilst the step value is negative.",
                        vscode.DiagnosticSeverity.Warning
                    ))
                }

                if (stepNum == 0) {
                    diags.push(new vscode.Diagnostic(
                        new vscode.Range(startPos, endPos),
                        "This will infinitely loop without a break statement.",
                        vscode.DiagnosticSeverity.Warning
                    ))
                }
            }

            // Detect malformed step value. Detect malformed everything at this rate!
            if (stepVal && !isValidIdentifier(stepVal) && !isNumeric(stepVal)) {
                diags.push(new vscode.Diagnostic(
                    new vscode.Range(startPos, endPos),
                    `Invalid step value '${stepVal}' in 'for' loop.`,
                    vscode.DiagnosticSeverity.Error
                ));
            }
        }

        // Checks through the stack, telling us which blocks are missing an end ('conclude')
        for (const unclosed of blockStack) {
            diags.push(new vscode.Diagnostic(
                new vscode.Range(unclosed.start, unclosed.start),
                "Missing 'conclude' for this block.",
                vscode.DiagnosticSeverity.Error
            ));
        }

        for (const unclosedComment of blockCommentStack) {
            diags.push(new vscode.Diagnostic(
                new vscode.Range(unclosedComment.start, unclosedComment.start),
                "Missing block comment ending.",
                vscode.DiagnosticSeverity.Error
            ))
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
        "declare",
        "resort",
        "say",
        "define",

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

    const taskTypes = [
        "function",
        "coroutine"
    ]

    const globalTasks = [
        "input",
        "when",
        "freeze",
        "error"
    ]

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
                    const declRegex = new RegExp(`\\bdeclare\\s+${targetVar}\\s+as\\s+([A-Za-z_][A-Za-z0-9_]*)`, 'g');
                    const typeMatch = declRegex.exec(text);

                    // When the typing matches
                    if (typeMatch) {
                        const varType = typeMatch[1].toLowerCase();
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
                    ...globalTasks.map(gt => new vscode.CompletionItem(gt, vscode.CompletionItemKind.Function)),
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
                { label: "getEnumerator()", detail: "Enumerates the list, returning a new dictionary with the indexes as the keys, and list items as the values. Type Specification is also provided." },
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