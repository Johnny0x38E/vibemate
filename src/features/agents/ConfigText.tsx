import type { JSX } from "react";
import styles from "./VersionTextDialog.module.css";

type TokenKind =
    "plain" | "key" | "string" | "number" | "literal" | "comment" | "section";

interface Token {
    text: string;
    kind: TokenKind;
}

/**
 * Read-only, syntax-highlighted configuration text.
 *
 * Tokens are rendered as React spans; no HTML is assembled from the document,
 * so a hostile value cannot inject markup. The text is already sanitized by
 * Rust, and line breaks/indentation are preserved by `white-space: pre`.
 */
export function ConfigText({
    text,
    format,
}: {
    text: string;
    format: "json" | "toml";
}): JSX.Element {
    const lines = text === "" ? [""] : text.split("\n");
    return (
        <pre className={styles["text"]}>
            <code>
                {lines.map((line, index) => (
                    <span className={styles["line"]} key={index}>
                        {tokenize(line, format).map((token, tokenIndex) =>
                            token.kind === "plain" ? (
                                token.text
                            ) : (
                                <span
                                    key={tokenIndex}
                                    className={
                                        styles[`token${capitalize(token.kind)}`]
                                    }
                                >
                                    {token.text}
                                </span>
                            ),
                        )}
                    </span>
                ))}
            </code>
        </pre>
    );
}

function tokenize(line: string, format: "json" | "toml"): Token[] {
    return format === "json" ? jsonLine(line) : tomlLine(line);
}

function jsonLine(line: string): Token[] {
    const pattern =
        /"(?:\\.|[^"\\])*"|\btrue\b|\bfalse\b|\bnull\b|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?/gu;
    const tokens: Token[] = [];
    let last = 0;
    for (const match of line.matchAll(pattern)) {
        const index = match.index;
        if (index > last) {
            tokens.push({ text: line.slice(last, index), kind: "plain" });
        }
        const value = match[0];
        tokens.push({ text: value, kind: jsonTokenKind(line, index, value) });
        last = index + value.length;
    }
    if (last < line.length) {
        tokens.push({ text: line.slice(last), kind: "plain" });
    }
    return tokens;
}

function jsonTokenKind(line: string, index: number, value: string): TokenKind {
    if (!value.startsWith('"')) {
        return value === "true" || value === "false" || value === "null"
            ? "literal"
            : "number";
    }
    // A string followed by a colon is an object key.
    return /^\s*:/u.test(line.slice(index + value.length)) ? "key" : "string";
}

function tomlLine(line: string): Token[] {
    const comment = commentIndex(line);
    const code = comment === -1 ? line : line.slice(0, comment);
    const tokens = tomlCode(code);
    if (comment !== -1) {
        tokens.push({ text: line.slice(comment), kind: "comment" });
    }
    return tokens;
}

function tomlCode(code: string): Token[] {
    const trimmed = code.trim();
    if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
        const start = code.indexOf(trimmed);
        const tokens: Token[] = [];
        if (start > 0)
            tokens.push({ text: code.slice(0, start), kind: "plain" });
        tokens.push({ text: trimmed, kind: "section" });
        const end = start + trimmed.length;
        if (end < code.length) {
            tokens.push({ text: code.slice(end), kind: "plain" });
        }
        return tokens;
    }
    const equals = assignmentIndex(code);
    if (equals === -1) return valueTokens(code);
    return [
        { text: code.slice(0, equals), kind: "plain" },
        { text: "=", kind: "plain" },
        ...valueTokens(code.slice(equals + 1)),
    ];
}

function valueTokens(text: string): Token[] {
    const pattern =
        /"(?:\\.|[^"\\])*"|'[^']*'|\btrue\b|\bfalse\b|-?\d+(?:\.\d+)?/gu;
    const tokens: Token[] = [];
    let last = 0;
    for (const match of text.matchAll(pattern)) {
        const index = match.index;
        if (index > last) {
            tokens.push({ text: text.slice(last, index), kind: "plain" });
        }
        const value = match[0];
        tokens.push({
            text: value,
            kind:
                value.startsWith('"') || value.startsWith("'")
                    ? "string"
                    : value === "true" || value === "false"
                      ? "literal"
                      : "number",
        });
        last = index + value.length;
    }
    if (last < text.length) {
        tokens.push({ text: text.slice(last), kind: "plain" });
    }
    return tokens;
}

/** Index of a `#` comment outside a quoted string, or -1. */
function commentIndex(line: string): number {
    let quote: string | null = null;
    let escaped = false;
    for (let index = 0; index < line.length; index += 1) {
        const character = line[index];
        if (quote !== null) {
            if (escaped) escaped = false;
            else if (character === "\\") escaped = true;
            else if (character === quote) quote = null;
            continue;
        }
        if (character === '"' || character === "'") quote = character;
        else if (character === "#") return index;
    }
    return -1;
}

/** Index of the assignment `=` outside a quoted string, or -1. */
function assignmentIndex(line: string): number {
    let quote: string | null = null;
    let escaped = false;
    for (let index = 0; index < line.length; index += 1) {
        const character = line[index];
        if (quote !== null) {
            if (escaped) escaped = false;
            else if (character === "\\") escaped = true;
            else if (character === quote) quote = null;
            continue;
        }
        if (character === '"' || character === "'") quote = character;
        else if (character === "=") return index;
    }
    return -1;
}

function capitalize(value: string): string {
    return `${value.slice(0, 1).toUpperCase()}${value.slice(1)}`;
}
