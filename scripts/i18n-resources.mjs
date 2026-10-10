/** Validate bundled translation resources before UI tests or a build run. */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

/**
 * Bundled resource files by locale. English is listed first because it is the
 * fallback language and the source of the TypeScript key shape.
 */
export const RESOURCE_FILES = {
    en: "src/locales/en.json",
    "zh-CN": "src/locales/zh-CN.json",
};

// i18next appends these CLDR plural categories to a key, for example
// `itemCount_one`. Keys must therefore not use these suffixes for other purposes.
const PLURAL_SUFFIX = /^(.+)_(zero|one|two|few|many|other)$/;
// Ordinal plurals (`place_ordinal_one`) follow different rules; not supported yet.
const ORDINAL_SUFFIX = /_ordinal_(zero|one|two|few|many|other)$/;
// i18next selects `key_zero` whenever count is 0, in every language, so it is an
// optional form even though Intl.PluralRules never reports "zero" for en/zh.
const OPTIONAL_PLURAL_FORMS = ["zero"];
// The first token inside {{ }} is the parameter name. An optional leading `-`
// is i18next's unescaped form (`{{- name}}`); a format such as
// `{{count, number}}` follows a comma. Neither changes the parameter name.
const PLACEHOLDER = /{{-?\s*([^{},\s-][^{},\s]*)[^{}]*}}/g;
// `{{}}`, `{{ }}` or `{{-}}` name no parameter at all.
const EMPTY_PLACEHOLDER = /{{-?\s*}}/g;

/**
 * Read every bundled resource as parsed JSON. Throws if a file is missing or
 * contains invalid JSON. No files are changed.
 */
export function readTranslations(projectRoot) {
    return Object.fromEntries(
        Object.entries(RESOURCE_FILES).map(([locale, file]) => [
            locale,
            JSON.parse(readFileSync(resolve(projectRoot, file), "utf8")),
        ]),
    );
}

/**
 * Compare translation resources and return human-readable problems, or an
 * empty list when they are consistent. `resources` maps a locale tag to its
 * parsed JSON object; the first entry is the reference locale.
 *
 * Checks: identical message keys, string-only non-empty values, matching
 * interpolation parameter names (including unescaped `{{- name}}`), well-formed
 * and named `{{ }}` markers, and plural forms that cover every category
 * `Intl.PluralRules` requires for that locale. `_zero` is always accepted;
 * ordinal plural keys are reported as unsupported.
 */
export function validateTranslations(resources) {
    const issues = [];
    const locales = Object.keys(resources);
    // Messages per locale: plain keys map to one string, plural keys are
    // grouped by their base key so each language can use its own forms.
    const catalogs = new Map(
        locales.map((locale) => [
            locale,
            collectMessages(locale, resources[locale], issues),
        ]),
    );
    const allowedForms = new Set([
        ...OPTIONAL_PLURAL_FORMS,
        ...locales.flatMap(pluralCategories),
    ]);

    const reference = locales[0];
    const referenceCatalog = catalogs.get(reference);
    for (const locale of locales.slice(1)) {
        const catalog = catalogs.get(locale);
        compareKeys(reference, referenceCatalog, locale, catalog, issues);
    }

    for (const locale of locales) {
        const catalog = catalogs.get(locale);
        const required = pluralCategories(locale);
        for (const [base, forms] of catalog.plural) {
            for (const form of required) {
                if (!forms.has(form))
                    issues.push(
                        `${locale} ${base}: missing plural form "${base}_${form}".`,
                    );
            }
            for (const form of forms.keys()) {
                // Another bundled locale may need a form this one never selects;
                // a form no bundled locale uses is a misspelled or stale key.
                if (!allowedForms.has(form))
                    issues.push(
                        `${locale} ${base}_${form}: plural form "${form}" is not used by any bundled locale.`,
                    );
            }
        }
    }

    // Parameters are compared against the reference so a translator cannot drop
    // or rename a value the UI passes. Plural forms are compared as one message.
    for (const locale of locales.slice(1)) {
        const catalog = catalogs.get(locale);
        for (const [key, text] of referenceCatalog.plain) {
            const other = catalog.plain.get(key);
            if (other !== undefined)
                compareParameters(locale, key, [text], [other], issues);
        }
        for (const [base, forms] of referenceCatalog.plural) {
            const other = catalog.plural.get(base);
            if (other !== undefined)
                compareParameters(
                    locale,
                    base,
                    [...forms.values()],
                    [...other.values()],
                    issues,
                );
        }
    }
    return issues;
}

/** The plural categories a locale's cardinal rules can select. */
function pluralCategories(locale) {
    return new Intl.PluralRules(locale).resolvedOptions().pluralCategories;
}

/** Flatten nested JSON into plain and plural messages, reporting bad values. */
function collectMessages(locale, resource, issues) {
    const plain = new Map();
    const plural = new Map();
    function visit(value, path) {
        const label = path || "(root)";
        if (typeof value === "string") {
            if (value.trim() === "")
                issues.push(`${locale} ${label}: translation is empty.`);
            // String.match with a global regex ignores lastIndex, unlike test().
            if (value.match(EMPTY_PLACEHOLDER) !== null)
                issues.push(
                    `${locale} ${label}: interpolation marker has no parameter name.`,
                );
            if (hasMalformedMarker(value))
                issues.push(
                    `${locale} ${label}: interpolation marker is not closed as {{name}}.`,
                );
            if (ORDINAL_SUFFIX.test(path)) {
                issues.push(
                    `${locale} ${label}: ordinal plurals are not supported yet.`,
                );
                return;
            }
            const match = PLURAL_SUFFIX.exec(path);
            if (match) {
                const [, base, form] = match;
                if (!plural.has(base)) plural.set(base, new Map());
                plural.get(base).set(form, value);
            } else {
                plain.set(path, value);
            }
            return;
        }
        if (
            typeof value !== "object" ||
            value === null ||
            Array.isArray(value)
        ) {
            issues.push(
                `${locale} ${label}: value must be a string or a group.`,
            );
            return;
        }
        const entries = Object.entries(value);
        if (entries.length === 0)
            issues.push(`${locale} ${label}: group has no translations.`);
        for (const [key, child] of entries)
            visit(child, path ? `${path}.${key}` : key);
    }
    visit(resource, "");
    return { plain, plural };
}

/** Report keys present in one locale but not the other, in both directions. */
function compareKeys(reference, expected, locale, actual, issues) {
    for (const [kind, label] of [
        ["plain", "key"],
        ["plural", "plural key"],
    ]) {
        for (const key of expected[kind].keys()) {
            if (!actual[kind].has(key))
                issues.push(
                    `${locale} ${key}: missing ${label} from ${reference}.`,
                );
        }
        for (const key of actual[kind].keys()) {
            if (!expected[kind].has(key))
                issues.push(
                    `${locale} ${key}: ${label} is not in ${reference}.`,
                );
        }
    }
}

/** Compare the set of parameter names used by two versions of one message. */
function compareParameters(locale, key, expectedTexts, actualTexts, issues) {
    const expected = parameterNames(expectedTexts);
    const actual = parameterNames(actualTexts);
    if (expected.join() !== actual.join())
        issues.push(
            `${locale} ${key}: parameters [${actual.join(", ")}] do not match [${expected.join(", ")}].`,
        );
}

function parameterNames(texts) {
    const names = new Set();
    for (const text of texts)
        for (const match of text.matchAll(PLACEHOLDER)) names.add(match[1]);
    return [...names].sort();
}

/** Braces left after removing valid and empty markers indicate a typo such as {{name}. */
function hasMalformedMarker(text) {
    const rest = text.replace(PLACEHOLDER, "").replace(EMPTY_PLACEHOLDER, "");
    return rest.includes("{{") || rest.includes("}}");
}

// Importing this module in tests must not run the command-line check.
if (
    process.argv[1] &&
    import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
    try {
        const issues = validateTranslations(readTranslations(process.cwd()));
        if (issues.length > 0) {
            for (const issue of issues) console.error(issue);
            console.error(`${issues.length} translation problem(s) found.`);
            process.exitCode = 1;
        } else {
            console.log(
                `Translations are consistent for ${Object.keys(RESOURCE_FILES).join(", ")}.`,
            );
        }
    } catch (error) {
        console.error(error instanceof Error ? error.message : String(error));
        process.exitCode = 1;
    }
}
