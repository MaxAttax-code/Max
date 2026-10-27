"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
const hover_1 = require("./hover");
const completion_1 = require("./completion");
const codeActions_1 = require("./codeActions");
const lint_1 = require("./lint");
function activate(context) {
    (0, hover_1.registerHover)(context);
    (0, completion_1.registerCompletion)(context);
    (0, codeActions_1.registerCodeActions)(context);
    (0, lint_1.registerLint)(context);
}
