import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from "path";
import { keywords, types, taskTypes, globalTasks, operators } from "./language";

export function registerCompletion(context: vscode.ExtensionContext) {
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

    const functionRegex = /define\s+function\s+([A-Za-z_][A-Za-z0-9_]*)\s+with\s*\(([^)]*)\)/g;
    const coroutineRegex = /define\s+coroutine\s+([A-Za-z_][A-Za-z0-9_]*)\s+=>\s/g;
    const enumHeaderRegex = /\benum\b\s+([A-Za-z_][A-Za-z0-9_]*)\s+members\b/g;

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
        
    /** Blanks out comments but keeps every offset and newline intact */
    function stripComments(text: string): string {
        const blank = (m: string) => m.replace(/[^\n]/g, " ");
        return text
            .replace(/\/\/\/[\s\S]*?\\\\\\/g, blank) // Our block comments, i.e. /// ... \\\
            .replace(/\/\/.*$/gm, blank); // A normal one-liner
    }

    /** Represents a single enum node, holding only it's necessary info */
    interface EnumNode {
        name: string;
        subMembers: EnumNode[];
        reference?: string;
    }

    /**
     * Finds where the next enum block end is located, starting from a location
     * @param text The text to check through
     * @param from Where to begin as a position in the text
     * @returns The index of the next ending, else -1
     */
    function enumFindBlockEnd(text: string, from: number): number {
        const re = /\b(members|conclude)\b/g;
        re.lastIndex = from;
        let depth = 1;
        let m: RegExpExecArray | null;
        while ((m = re.exec(text)) !== null) {
            depth += m[1] === "members" ? 1 : -1;
            if (depth === 0) return m.index;
        }
        return -1;
    }

    /**
     * Splits an enum from the body, seperating sub members in the process via depth.
     * This is very similar to the splitTopLevel function in the runLint method.
     * @see `runLint()` -> `splitTopLevel()`
     * @param body The body to split
     * @returns The final array of split parts
     */
    function enumSplitTopLevel(body: string): string[] {
        const parts: string[] = [];
        const re = /\b(members|conclude)\b|,/g;
        let depth = 0, start = 0;
        let m: RegExpExecArray | null;

        while ((m = re.exec(body)) !== null) {
            // members = another enum/sub-member category
            if (m[0] === "members") depth++;
            // conclude = end of the previous block
            else if (m[0] === "conclude") depth--;
            else if (depth === 0) {
                parts.push(body.slice(start, m.index));
                start = m.index + 1;
            }
        }

        parts.push(body.slice(start));
        return parts;
    }

    /**
     * Parses an enum body into it's seperate enum members, including sub members and sub-enum referencing.
     * This is similar to the runLint() enum logic.
     * @see `runLint()` -> `enumHeaderRegex` code
     * @param body The body to parse
     * @returns The resulting list of Enum members
     */
    function parseEnumMembers(body: string): EnumNode[] {
        const members: EnumNode[] = [];
        for (const part of enumSplitTopLevel(body)) {
            const trimmed = part.trim();
            if (!trimmed) continue;

            // Yeah, just look at the runLint() code for more in-depth
            const plain = /^([A-Za-z_][A-Za-z0-9_]*)$/.exec(trimmed);
            const withSubs = /^([A-Za-z_][A-Za-z0-9_]*)\s*\$\s*members\b([\s\S]*)\bconclude$/.exec(trimmed);
            const withRef = /^([A-Za-z_][A-Za-z0-9_]*)\s*\$\s*([A-Za-z_][A-Za-z0-9_]*)$/.exec(trimmed);

            if (plain) members.push({ name: plain[1], subMembers: [] });
            else if (withSubs) members.push({ name: withSubs[1], subMembers: parseEnumMembers(withSubs[2]) });
            else if (withRef) members.push({ name: withRef[1], subMembers: [], reference: withRef[2] });
        }
        return members;
    }

    /**
     * Parses an entire by using a `enumHeaderRegex`, mapping enum members with their enum name,
     * @param rawText The string to parse enums from
     * @returns A final map of the enum members, with their corresponding enum name
     */
    function parseEnums(rawText: string): Map<string, EnumNode[]> {
        const text = stripComments(rawText);
        const enums = new Map<string, EnumNode[]>();
        const header = /\benum\b\s+([A-Za-z_][A-Za-z0-9_]*)\s+members\b/g;
        let m: RegExpExecArray | null;

        while ((m = header.exec(text)) !== null) {
            const bodyStart = m.index + m[0].length;
            // Find the block end using the corresponding block start
            const end = enumFindBlockEnd(text, bodyStart);
            if (end === -1) continue;

            enums.set(m[1], parseEnumMembers(text.slice(bodyStart, end)));
            header.lastIndex = end;
        }
        return enums;
    }

    /**
     * Resolves the children from a chain of enum member accessing
     * @param enums The enums Map to look through
     * @param chain The chain of member accessing that leads to the enum members
     * @returns The final resolved enum members at the end of the chain
     */
    function resolveEnumChildren(enums: Map<string, EnumNode[]>, chain: string[]): EnumNode[] | undefined {
        let members = enums.get(chain[0]);

        for (const segment of chain.slice(1)) {
            const node = members?.find(m => m.name === segment);
            // Nothing found in the members via the chain?
            // That means this is a dead end, so return undefined
            if (!node) return undefined;
            members = node.reference ? enums.get(node.reference) : node.subMembers;
        }
        return members;
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
                // This matches consecutive accessing too ('something.foo.bar')
                const memberAccessMatch = textBeforeCursor.match(
                    /([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)\.\s*$/
                );

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
                    const chain = memberAccessMatch[1].split(".");
                    const targetVar = chain[0]; // Of course the target var is always first

                    // Check if an enum was defined of this name
                    const enumChildren = resolveEnumChildren(parseEnums(text), chain);
                    if (enumChildren) {
                        return enumChildren.map(member => {
                            const item = new vscode.CompletionItem(member.name, vscode.CompletionItemKind.EnumMember);
                            item.detail = member.reference
                                ? `Sub enum: ${member.reference}`
                                : member.subMembers.length > 0 ? "Has sub members" : "Enum member";
                            return item;
                        });
                    }

                    // Nothing else should be requiring more than two strings in the chain
                    // We will add modules later
                    if (chain.length > 1) return [];

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
                        const varType = typeMatch[2]?.toLowerCase();
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

                // Enums too!
                while ((match = enumHeaderRegex.exec(text)) !== null) {
                    const item = new vscode.CompletionItem(match[1], vscode.CompletionItemKind.Enum);
                    item.detail = "User-defined enum";
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