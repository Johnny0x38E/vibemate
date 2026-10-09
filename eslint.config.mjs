import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import prettier from "eslint-config-prettier";

export default defineConfig([
    {
        ignores: [
            "node_modules/**",
            "dist/**",
            "src-tauri/**",
            ".codegraph/**",
        ],
    },
    {
        files: ["src/**/*.{ts,tsx}", "vite.config.ts"],
        extends: [js.configs.recommended, tseslint.configs.strictTypeChecked],
        languageOptions: {
            parserOptions: {
                projectService: {
                    // Vite configuration has its own Node tsconfig rather than the UI project.
                    allowDefaultProject: ["vite.config.ts"],
                    defaultProject: "tsconfig.node.json",
                },
                tsconfigRootDir: import.meta.dirname,
            },
        },
        rules: {
            "@typescript-eslint/explicit-module-boundary-types": "error",
            "@typescript-eslint/consistent-type-imports": "error",
            "@typescript-eslint/no-floating-promises": "error",
            "@typescript-eslint/ban-ts-comment": [
                "error",
                {
                    "ts-ignore": true,
                    "ts-nocheck": true,
                    "ts-expect-error": "allow-with-description",
                    minimumDescriptionLength: 10,
                },
            ],
            "no-warning-comments": [
                "error",
                { terms: ["fixme"], location: "start" },
            ],
            "no-restricted-syntax": [
                "error",
                {
                    selector: "TSEnumDeclaration",
                    message: "Prefer a typed union instead of an enum.",
                },
            ],
        },
    },
    {
        files: ["src/**/*.{ts,tsx}"],
        extends: [reactHooks.configs.flat.recommended],
        languageOptions: { globals: globals.browser },
        rules: {
            "react-hooks/exhaustive-deps": "error",
            "no-console": "error",
            "no-eval": "error",
            "no-restricted-globals": [
                "error",
                "fetch",
                "localStorage",
                "sessionStorage",
            ],
            "no-restricted-properties": [
                "error",
                {
                    object: "window",
                    property: "fetch",
                    message: "Provider requests belong in Rust.",
                },
                {
                    object: "globalThis",
                    property: "fetch",
                    message: "Provider requests belong in Rust.",
                },
                {
                    property: "localStorage",
                    message:
                        "Do not persist frontend data or credentials in browser storage.",
                },
                {
                    property: "sessionStorage",
                    message: "Do not persist credentials in browser storage.",
                },
            ],
            "no-restricted-imports": [
                "error",
                {
                    patterns: [
                        {
                            group: ["node:*"],
                            message: "System operations belong in Rust.",
                        },
                    ],
                },
            ],
        },
    },
    {
        files: ["src/**/*.{ts,tsx}"],
        ignores: ["src/lib/desktop.ts", "src/lib/desktop/**"],
        rules: {
            "no-restricted-imports": [
                "error",
                {
                    patterns: [
                        {
                            group: ["@tauri-apps/*", "node:*"],
                            message:
                                "Use typed wrappers in src/lib/desktop; native operations belong in Rust.",
                        },
                    ],
                },
            ],
        },
    },
    {
        files: ["vite.config.ts", "eslint.config.mjs", "scripts/**/*.mjs"],
        languageOptions: { globals: globals.node },
    },
    {
        files: ["eslint.config.mjs", "scripts/**/*.mjs"],
        extends: [js.configs.recommended],
    },
    // Prettier owns whitespace and formatting; ESLint checks behavior and types.
    prettier,
]);
