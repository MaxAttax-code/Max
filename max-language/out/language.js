"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.globalTasks = exports.taskTypes = exports.iterTypes = exports.types = exports.operators = exports.keywords = void 0;
exports.keywords = [
    "using",
    "declare",
    "resort",
    "say",
    "define",
    "enum",
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
    "yield",
    "members"
];
exports.operators = [
    "not",
    "and",
    "or",
    "step"
];
exports.types = [
    "int",
    "float",
    "string",
    "boolean",
    "list",
    "dictionary"
];
exports.iterTypes = [
    "list",
    "dictionary"
];
exports.taskTypes = [
    "function",
    "coroutine"
];
exports.globalTasks = {
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
};
