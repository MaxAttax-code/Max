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
exports.isPascalCase = isPascalCase;
exports.toPascalCase = toPascalCase;
exports.registerLint = registerLint;
const vscode = __importStar(require("vscode"));
const language_1 = require("./language");
// ---------------------------- \\
// ---- CASING HELPER FUNCS --- \\
// ---------------------------- \\
// Keep in mind these funcs are outside runLint(), as they may also be used within Hover providers and CodeActions
/**
 * Determines if a word follows PascalCase naming conventions
 */
function isPascalCase(str) {
    if (!str || str.length === 0) {
        return false;
    }
    // Check if the first character is an uppercase letter
    // Sorry, but'd it basically be requiring a dictionary, if
    // we were to check EVERY word is Uppercase
    const firstChar = str[0];
    if (firstChar !== firstChar.toUpperCase() || firstChar === firstChar.toLowerCase()) {
        return false;
    }
    // Ensure it contains only alphanumeric characters (no spaces, underscores, or hyphens)
    return /^[A-Za-z0-9]+$/.test(str);
}
/**
 * Formats an identifier into PascalCase
 * @returns The new string formatted in PascalCase
 */
function toPascalCase(str) {
    if (!str)
        return "";
    // If it already matches PascalCase, return it as-is
    if (isPascalCase(str)) {
        return str;
    }
    // Replace any non-alphanumeric character with a space,
    // then split the string into words.
    const words = str
        .replace(/[^A-Za-z0-9]/g, " ")
        .split(/\s+/)
        .filter(Boolean); // Remove empty strings. Clean!
    if (words.length === 0)
        return "";
    // Capitalize the first letter of each word and join them together
    return words
        .map(word => word.charAt(0).toUpperCase() + word.slice(1))
        .join("");
}
function registerLint(context) {
    // Create a diagnostics bucket. This is for error linting
    const diagnostics = vscode.languages.createDiagnosticCollection("max");
    let TaskType;
    (function (TaskType) {
        TaskType[TaskType["function"] = 0] = "function";
        TaskType[TaskType["coroutine"] = 1] = "coroutine";
    })(TaskType || (TaskType = {}));
    let LifecycleModifier;
    (function (LifecycleModifier) {
        LifecycleModifier[LifecycleModifier["open"] = 0] = "open";
        LifecycleModifier[LifecycleModifier["static"] = 1] = "static";
    })(LifecycleModifier || (LifecycleModifier = {}));
    let MutabilityModifier;
    (function (MutabilityModifier) {
        MutabilityModifier[MutabilityModifier["fixed"] = 0] = "fixed";
    })(MutabilityModifier || (MutabilityModifier = {}));
    /**
     * This function gives us the error linting needed for Max.
     * @param document The text document to inspect
     */
    function runLint(document) {
        // |‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾| \\
        // |     HELPER FUNCTIONS     | \\
        // |__________________________| \\
        /**
         * Checks if an offset (a given positional number) is within a block comment or comment
         * @param offset The offset to check is within a comment block
         * @param blocks The comment blocks to check within
         * @returns Whether the offset was in any given block comment or not
         */
        function isInsideBlockComment(offset, blocks) {
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
        function isInsideLineComment(offset) {
            return lineComments.some(c => offset >= c.start && offset <= c.end);
        }
        // We are lazy, so let's just do both funcs in one!
        function insideAnyComment(offset, blocks) {
            return isInsideBlockComment(offset, blocks) || isInsideLineComment(offset);
        }
        /**
         * Checks if a given name is one of the types in the types list, or a valid enum type.
         * @param name The name to check
         * @returns True if the name is a type, else false
         */
        function isTypeName(name) {
            // If a type or an enum name!
            return language_1.types.includes(name) || (enumStack.find(s => s.name === name) !== undefined);
        }
        /**
         * Checks if a given name is one of the iterable types in the list.
         * @param name The name to check
         * @returns True if the name is an iterable type, else false
         */
        function isIterableTypeName(name) {
            return language_1.iterTypes.includes(name);
        }
        /**
         * Function that helps parse typing, returning if it's valid or not.
         * @param typeSpec The type name to check
         * @returns A boolean if the type is valid or not
         */
        function parseIsValidType(typeSpec) {
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
                if (!isIterableTypeName(iterType))
                    return false;
                return parseIsValidType(inner); // RECURSION. Super cool right?
            }
            // Short-form collection using the '<>' symbols
            const shortForm = /^([A-Za-z_][A-Za-z0-9_]*)<(.+)>$/.exec(typeSpec);
            if (shortForm) {
                const iterType = shortForm[1];
                const inner = shortForm[2];
                if (!isIterableTypeName(iterType))
                    return false;
                return parseIsValidType(inner);
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
        function parseTaskParameters(paramText, paramsStartPos) {
            const startPos = document.positionAt(paramsStartPos);
            const endPos = document.positionAt(paramsStartPos + paramText.length);
            // First we extract content inside parentheses
            const innerMatch = paramText.match(/\(([^)]*)\)/);
            if (!innerMatch)
                return [];
            const innerContent = innerMatch[1].trim();
            if (!innerContent)
                return []; // Empty parameters: with ()
            // Split 'em by comma to evaluate each parameter individually
            const rawParams = innerContent.split(',');
            const parameters = [];
            // Through each param found
            for (const rawParam of rawParams) {
                const trimmed = rawParam.trim();
                if (!trimmed)
                    continue;
                // A flexible regex: requires name, 'as', and a type name optionally
                const paramMatch = trimmed.match(/^([A-Za-z_][A-Za-z0-9_]*)(?:\s+as\s+([A-Za-z_][A-Za-z0-9_]*))?$/);
                if (!paramMatch) {
                    // Report syntax error for malformed parameter structure
                    diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), // adjust position as needed
                    `Invalid parameter syntax: "${trimmed}". Expected format: 'name' or 'name as type'`, vscode.DiagnosticSeverity.Error));
                    continue;
                }
                const paramName = paramMatch[1];
                const typeName = paramMatch[2] || null; // Will be null if omitted
                // Finally, validate the type
                if (typeName !== null && !language_1.types.includes(typeName)) {
                    diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), `Unknown type "${typeName}" for parameter "${paramName}"`, vscode.DiagnosticSeverity.Error));
                }
                parameters.push({ name: paramName, type: typeName });
            }
            return parameters;
        }
        /**
         * Returns if a given string identifier is valid for naming
         * @param identifier The string identifier to check
         * @returns If the identifier is valid or not
         */
        function isValidIdentifier(identifier) {
            if (language_1.keywords.includes(identifier) || language_1.operators.includes(identifier) || language_1.taskTypes.includes(identifier)) {
                return false;
            }
            return true;
        }
        /** Index of the `conclude` that closes a block whose body starts at `from`, or -1 */
        function findBlockEnd(from) {
            const tokenRegex = /\b(members|conclude)\b/g;
            tokenRegex.lastIndex = from;
            let depth = 1;
            let m;
            while ((m = tokenRegex.exec(text)) !== null) {
                if (insideAnyComment(m.index, blockCommentStack))
                    continue;
                depth += m[1] === "members" ? 1 : -1;
                if (depth === 0)
                    return m.index;
            }
            return -1;
        }
        /** Splits a members body on commas at depth 0, keeping each piece's document offset */
        function splitTopLevel(body, baseOffset) {
            const parts = [];
            const re = /\b(members|conclude)\b|,/g;
            let depth = 0;
            let start = 0;
            let m;
            // This just finds where 'members' or 'conclude' are
            while ((m = re.exec(body)) !== null) {
                if (m[0] === "members")
                    depth++; // Members? Increase depth
                else if (m[0] === "conclude")
                    depth--; // Conclude the members, thus decrease depth
                else if (depth === 0) {
                    // Else just input the parts in when there's no depth (no member validation)
                    parts.push({ text: body.slice(start, m.index), offset: baseOffset + start });
                    start = m.index + 1;
                }
            }
            parts.push({ text: body.slice(start), offset: baseOffset + start });
            return parts;
        }
        // |‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾| \\
        // |          SETUP           | \\
        // |__________________________| \\
        const text = document.getText();
        const diags = [];
        // Regex to capture: declare <identifier> (as <type>) = <value>(, resort <value2>)
        // BUT! In order to prevent Regex hell, we simply capture all until a newline
        // Syntaxing is handled later
        const declareRegex = /\bdeclare\b([^\n]*)/g;
        // Regex to capture: define <tasktype> <name> [with (params as types...) | =>] [return type] do
        const defineRegex = /\bdefine\s+([A-Za-z_][A-Za-z0-9_]*)\s+([A-Za-z_][A-Za-z0-9_]*)\s+(?:(with\s*\([^)]*\))|(=>))(?:\s+return\s+([A-Za-z_][A-Za-z0-9_]*))?\s+\bdo\b/g;
        // Regex for a block start and end
        const blockRegex = /\b(then|do|members|conclude)\b/g;
        // Regex to capture: enum <name> members
        // // The body is found by depth counting
        const enumHeaderRegex = /\benum\b\s+([A-Za-z_][A-Za-z0-9_]*)\s+members\b/g;
        // Regex to capture: // <comment>
        const lineCommentRegex = /\/\/.*$/gm;
        // Creating an interface? Nah, just a list of line comments that are found
        const lineComments = [];
        // Defines when a block comment starts and ends, as /// and \\\
        // They need no word boundaries ('\b') since they are not characters (which would be '\w' or '\W')
        const blockCommentStartRegex = /\/\/\//g; // ///
        const blockCommentEndRegex = /\\\\\\/g; // \\\
        const blockCommentStack = [];
        // Start with a stack of only one scope. This single scope should never be popped
        const scopeStack = [{ vars: [], startRange: new vscode.Range(document.positionAt(0), document.positionAt(text.length)) }];
        const taskStack = [];
        const enumStack = [];
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
                diags.push(new vscode.Diagnostic(new vscode.Range(endPos, endPos.translate(0, match[0].length)), "Unexpected block comment ending; no starting block comment to end found.", vscode.DiagnosticSeverity.Error));
                continue;
            }
            // This mimics normal block code
            const blockComment = blockCommentStack.pop();
            if (blockComment === undefined) {
                throw new Error("Block attempted to be recieved from stack did not exist.");
            }
            blockComment.endOffset = match.index;
            const blockContent = text.substring(blockComment.startOffset, blockComment.endOffset);
            // Check if block comment is only whitespace/newlines
            if (/^\s*$/.test(blockContent)) {
                diags.push(new vscode.Diagnostic(new vscode.Range(document.positionAt(blockComment.startOffset), endPos), "Empty block comment detected — Is this on purpose? If so, add an underscore to remove this warning.", vscode.DiagnosticSeverity.Information));
            }
        }
        // |‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾| \\
        // |      SYNTAX CHECKING     | \\
        // |__________________________| \\
        // KEEP IN MIND THAT THE ORDER OF THESE REGEXES MATTER
        // ---------------------------- \\
        // ------ DEFINITION LAWS ----- \\
        // ---------------------------- \\
        while ((match = defineRegex.exec(text)) !== null) {
            const fullMatch = match[0];
            const taskTypeName = match[1];
            const identifier = match[2];
            const withClause = match[3]; // e.g. "with (x as int, y as int)" or undefined
            const arrowClause = match[4]; // e.g. "=>" or undefined
            const returnType = match[5]; // e.g. "return int" or undefined
            const startPos = document.positionAt(match.index);
            const endPos = document.positionAt(match.length + fullMatch.length);
            // ----------------------------
            // 1. Validate the Identifier
            // ----------------------------
            if (!isValidIdentifier(identifier)) {
                diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), `Invalid task definition name, cannot use ${identifier}`, vscode.DiagnosticSeverity.Error));
                continue;
            }
            // ----------------------------
            // 2. Validate the Task Type
            // ----------------------------
            let taskType;
            switch (taskTypeName) {
                case "function":
                    taskType = TaskType.function;
                    break;
                case "coroutine":
                    taskType = TaskType.coroutine;
                    break;
                default: {
                    diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), `Invalid Task Type given: ${taskTypeName}`, vscode.DiagnosticSeverity.Error));
                    continue;
                }
            }
            const clauses = `${withClause === undefined}:${arrowClause === undefined}`;
            let params = [];
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
                        diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), `Invalid Task Definition structure. A Coroutine cannot have any parameters.`, vscode.DiagnosticSeverity.Error));
                        continue;
                    }
                    params = parseTaskParameters(withClause, withClauseIndex);
                    break;
                }
                default: {
                    diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), `Invalid Task Definition structure. Must have parameters or be parameterless.`, vscode.DiagnosticSeverity.Error));
                    continue;
                }
            }
            if (returnType !== undefined) {
                if (!isTypeName(returnType)) {
                    diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), `Invalid return type for task definition.`, vscode.DiagnosticSeverity.Error));
                    continue;
                }
            }
            taskStack.push({
                name: identifier,
                type: taskType,
                parameters: params
            });
        }
        // ---------------------------- \\
        // -------- ENUM LAWS --------- \\
        // ---------------------------- \\
        while ((match = enumHeaderRegex.exec(text)) !== null) {
            const enumName = match[1];
            const bodyStart = match.index + match[0].length;
            const endIndex = findBlockEnd(bodyStart);
            // Unclosed enum: the scope check at the bottom already reports it
            if (endIndex === -1)
                continue;
            const membersCaptured = text.slice(bodyStart, endIndex);
            const enumRange = new vscode.Range(document.positionAt(match.index), document.positionAt(endIndex + "conclude".length));
            const enumNameIndex = match.index + match[0].indexOf(enumName, 4); // search after the word "enum"
            const enumNameRange = new vscode.Range(document.positionAt(enumNameIndex), document.positionAt(enumNameIndex + enumName.length));
            // ----------------------------
            // 1. Validate the Enum Name
            // ----------------------------
            if (enumStack.find(e => e.name === enumName) !== undefined) {
                // The same name was found? Not cool!
                diags.push(new vscode.Diagnostic(enumNameRange, `Already defined Enum name: '${enumName}'`, vscode.DiagnosticSeverity.Error));
            }
            if (!isValidIdentifier(enumName)) {
                diags.push(new vscode.Diagnostic(enumNameRange, `Invalid Enum name, must not be a Max-reserved word, got: '${enumName}'`, vscode.DiagnosticSeverity.Error));
            }
            // ----------------------------
            // 2. Validate the members
            // ----------------------------
            const finalMembers = [];
            /** Validates an enum member, checking if it is a duplicate or invalid */
            function validateMember(name, memberList, range) {
                if (!isValidIdentifier(name)) {
                    diags.push(new vscode.Diagnostic(range, `Enum member name must not be a Max-reserved word, got: '${name}'`, vscode.DiagnosticSeverity.Error));
                }
                if (memberList.some(m => m.memberName === name)) {
                    diags.push(new vscode.Diagnostic(range, `Duplicate Enum member name: '${name}'`, vscode.DiagnosticSeverity.Error));
                }
            }
            /**
            Validates a member list. This also has recursive ability with enum sub-members.
            The body is what the members captured from the original enum are.
            */
            function validateMembers(memberList, body, bodyOffset) {
                let intValue = 0; // Sub members restart at 0, mirrors our C# logic
                for (const part of splitTopLevel(body, bodyOffset)) {
                    const trimmed = part.text.trim();
                    if (trimmed.length === 0)
                        continue; // Trailing comma? Alr
                    // Our splitTopLevel func very nicely gives us offsets
                    const start = part.offset + part.text.indexOf(trimmed);
                    const range = new vscode.Range(document.positionAt(start), document.positionAt(start + trimmed.length));
                    // Depending on how the member is written
                    const plain = /^([A-Za-z_][A-Za-z0-9_]*)$/.exec(trimmed);
                    const withSubs = /^([A-Za-z_][A-Za-z0-9_]*)\s*\$\s*members\b[\s\S]*\bconclude$/.exec(trimmed);
                    const withRef = /^([A-Za-z_][A-Za-z0-9_]*)\s*\$\s*([A-Za-z_][A-Za-z0-9_]*)$/.exec(trimmed);
                    if (plain) {
                        validateMember(plain[1], memberList, range);
                        memberList.push({ enumName, memberName: plain[1], intValue, subMembers: [] });
                    }
                    else if (withSubs) {
                        const name = withSubs[1];
                        validateMember(name, memberList, range);
                        // Sub body = everything between this member's `members` and its final `conclude`
                        const membersKeyword = trimmed.indexOf("members", name.length);
                        const subBodyStart = membersKeyword + "members".length; // Starts after 'member' length
                        const subBody = trimmed.slice(subBodyStart, trimmed.length - "conclude".length);
                        const subMembers = [];
                        validateMembers(subMembers, subBody, start + subBodyStart);
                        memberList.push({ enumName, memberName: name, intValue, subMembers });
                    }
                    else if (withRef) {
                        const [/* Blank */ , name, referenced] = withRef;
                        validateMember(name, memberList, range);
                        const refStart = start + trimmed.lastIndexOf(referenced);
                        // How nice, we highlight specifically the referenced enum
                        const refRange = new vscode.Range(document.positionAt(refStart), document.positionAt(refStart + referenced.length));
                        // Check if the name given is a defined enum
                        if (!enumStack.some(e => e.name === referenced)) {
                            diags.push(new vscode.Diagnostic(refRange, `Referenced sub enum '${referenced}' is not defined. If it exists, it must be declared before this enum definition`, vscode.DiagnosticSeverity.Error));
                        }
                        memberList.push({ enumName, memberName: name, intValue, subMembers: [] });
                    }
                    else {
                        diags.push(new vscode.Diagnostic(range, `Invalid Enum member name format: '${trimmed}'`, vscode.DiagnosticSeverity.Error));
                    }
                    // Increase!
                    intValue++;
                }
            }
            validateMembers(finalMembers, membersCaptured, bodyStart);
            // ----------------------------
            // 3. Non-Error Touches
            // ----------------------------
            // An empty enum? That's marked as suspicious
            if (finalMembers.length === 0) {
                diags.push(new vscode.Diagnostic(enumRange, `Empty Enum found`, vscode.DiagnosticSeverity.Warning));
            }
            if (!isPascalCase(enumName)) {
                diags.push(new vscode.Diagnostic(enumNameRange, `Enum identifier '${enumName}' should ideally be in PascalCase ('${toPascalCase(enumName)}')`, vscode.DiagnosticSeverity.Information));
            }
            // ----------------------------
            // 4. Add to Stack
            // ----------------------------
            enumStack.push({
                name: enumName,
                members: finalMembers
            });
        }
        // ---------------------------- \\
        // ----- DECLARATION LAWS ----- \\
        // ---------------------------- \\
        while ((match = declareRegex.exec(text)) !== null) {
            const fullMatch = match[0];
            const remainder = match[1].trim();
            const startPos = document.positionAt(match.index);
            const endPos = document.positionAt(match.index + fullMatch.length);
            if (insideAnyComment(match.index, blockCommentStack))
                continue;
            // Now we split the comma and resort if it exists
            const parts = remainder.split(/\s*,\s*resort/);
            const mainPart = parts[0]; // Our declaration and value
            const resortPart = parts[1]; // Our resortable, or undefined
            // Let's take this in steps!
            // ----------------------------
            // 1. Parse resort value
            // ----------------------------
            let resortValue = null;
            if (resortPart !== undefined) {
                // Get if the resort match is valid
                const resortMatch = /^\s+(.+)$/.exec(resortPart);
                if (!resortMatch) {
                    diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), "Invalid resort clause. Expected: resort <value>", vscode.DiagnosticSeverity.Error));
                    continue;
                }
                resortValue = resortMatch[1];
            }
            // ----------------------------
            // 2. Parse main declaration
            // ----------------------------
            // Find the name and value, with typing in the hue!
            // name [as type] = value
            const mainMatch = /^((?:(?:fixed|open|static)\s+)+)?([A-Za-z_][A-Za-z0-9_]*)(?:\s+as\s+(.+?))?\s*=\s*(.+)$/.exec(mainPart);
            if (!mainMatch) {
                diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), "Invalid declaration statement", vscode.DiagnosticSeverity.Error));
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
            if (!isValidIdentifier(identifier)) {
                diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), `Invalid declaration name, cannot use ${identifier}`, vscode.DiagnosticSeverity.Error));
                continue;
            }
            let isDuplicate = false;
            const currentScopeIndex = scopeStack.length - 1;
            for (let i = currentScopeIndex; i >= 0; i--) {
                const existing = scopeStack[i].vars.find(v => v.name === identifier);
                if (existing === undefined)
                    continue;
                if (existing.modifiers.lifecycleModifier !== LifecycleModifier.open) {
                    isDuplicate = true;
                    break;
                }
                break;
            }
            if (isDuplicate) {
                diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), `Duplicate declaration name found: '${identifier}' already exists in this scope or an outer scope.`, vscode.DiagnosticSeverity.Error));
                continue;
            }
            // ----------------------------
            // 4. Parse modifier
            // ----------------------------
            // Get names based on splitting whitespaces
            const modifierNames = modifierText
                ? modifierText.split(/\s+/)
                : [];
            const modifiers = {
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
                if (invalidModifiers)
                    break;
            }
            if (invalidModifiers) {
                diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), "Conflicting or duplicate declaration modifiers.", vscode.DiagnosticSeverity.Error));
                continue;
            }
            // ----------------------------
            // 5. Parse type specification
            // ----------------------------
            if (typeSpect !== undefined) {
                // Check out `parseType` for more
                if (!parseIsValidType(typeSpect)) {
                    diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), `Invalid type specification '${typeSpect}'.`, vscode.DiagnosticSeverity.Error));
                    continue;
                }
            }
            // ----------------------------
            // 6. Validate value
            // ----------------------------
            if (value.length === 0) {
                diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), `Expected a value after '='`, vscode.DiagnosticSeverity.Error));
                continue;
            }
            // ----------------------------
            // 7. Validate resort value
            // ----------------------------
            if (resortValue !== null && resortValue.length === 0) {
                diags.push(new vscode.Diagnostic(new vscode.Range(startPos, endPos), `Expected a value after 'resort'`, vscode.DiagnosticSeverity.Error));
                continue;
            }
            const wanted = scopeStack[scopeStack.length - 1].vars;
            // Find if the var already exists (of course this only applies to shadowing)
            const existingIndex = wanted.findIndex(v => v.name === identifier);
            const variable = {
                name: identifier,
                modifiers: modifiers,
                type: typeSpect,
                value: value
            };
            // Shadowing in the same scope? then just replace that variable!
            if (existingIndex !== -1) {
                wanted[existingIndex] = variable;
            }
            else {
                wanted.push(variable);
            }
        }
        // ---------------------------- \\
        // ----- BLOCK REGEX LAWS ----- \\
        // ---------------------------- \\
        while ((match = blockRegex.exec(text)) !== null) {
            const keyword = match[0];
            const startPos = document.positionAt(match.index);
            const endPos = document.positionAt(match.index + keyword.length);
            if (keyword === "then" || keyword === "do" || keyword === "members") {
                // Start a new scope
                scopeStack.push({
                    startRange: new vscode.Range(startPos, endPos),
                    vars: []
                });
            }
            else if (keyword === "conclude") {
                // End the current scope
                scopeStack.pop();
            }
        }
        // |‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾| \\
        // |    FINAL CONSISTENCY     | \\
        // |__________________________| \\
        // If more than the global scope is left (or somehow the global scope was popped)
        if (scopeStack.length < 1) {
            diags.push(new vscode.Diagnostic(new vscode.Range(document.positionAt(0), document.positionAt(text.length)), "Cannot attempt to exit out of the global scope", vscode.DiagnosticSeverity.Error));
        }
        else if (scopeStack.length !== 1) {
            for (let unclosed of scopeStack) {
                if (scopeStack.indexOf(unclosed) === 0) {
                    continue; // Ignore the global scope
                }
                diags.push(new vscode.Diagnostic(unclosed.startRange, "Unclosed scope found. Close with 'conclude'.", vscode.DiagnosticSeverity.Error));
            }
        }
        diagnostics.set(document.uri, diags);
    }
    // Now we activate it for workspace changes
    vscode.workspace.onDidChangeTextDocument(event => {
        if (event.document.languageId == "max") { // For our language
            runLint(event.document);
        }
    });
}
