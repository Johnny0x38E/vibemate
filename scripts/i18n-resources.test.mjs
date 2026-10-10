import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
    readTranslations,
    RESOURCE_FILES,
    validateTranslations,
} from "./i18n-resources.mjs";

/** A minimal consistent pair; each test changes one thing to isolate a rule. */
function resources() {
    return {
        en: {
            nav: { home: "Home" },
            greeting: "Hello, {{name}}.",
            itemCount_one: "{{count, number}} item",
            itemCount_other: "{{count, number}} items",
        },
        "zh-CN": {
            nav: { home: "首页" },
            greeting: "你好，{{name}}。",
            itemCount_one: "{{count, number}} 项",
            itemCount_other: "{{count, number}} 项",
        },
    };
}

test("accepts the bundled resources", () => {
    assert.deepEqual(Object.keys(RESOURCE_FILES), ["en", "zh-CN"]);
    assert.deepEqual(validateTranslations(readTranslations(process.cwd())), []);
});

test("accepts consistent fixtures, including a form only another locale needs", () => {
    assert.deepEqual(validateTranslations(resources()), []);
    // Chinese selects only "other"; omitting the English-only form is valid too.
    const data = resources();
    delete data["zh-CN"].itemCount_one;
    assert.deepEqual(validateTranslations(data), []);
});

test("reports missing and extra keys in both directions", () => {
    const data = resources();
    delete data["zh-CN"].nav.home;
    data["zh-CN"].nav.settings = "设置";
    data["zh-CN"].nav.home2 = "首页";
    assert.deepEqual(validateTranslations(data), [
        "zh-CN nav.home: missing key from en.",
        "zh-CN nav.settings: key is not in en.",
        "zh-CN nav.home2: key is not in en.",
    ]);
});

test("reports empty, whitespace-only, non-string values and empty groups", () => {
    const data = resources();
    data["zh-CN"].greeting = " \n";
    data.en.nav.home = "";
    data.en.extra = 3;
    data["zh-CN"].extra = ["three"];
    data.en.empty = {};
    data["zh-CN"].empty = null;
    assert.deepEqual(validateTranslations(data), [
        "en nav.home: translation is empty.",
        "en extra: value must be a string or a group.",
        "en empty: group has no translations.",
        "zh-CN greeting: translation is empty.",
        "zh-CN extra: value must be a string or a group.",
        "zh-CN empty: value must be a string or a group.",
        "zh-CN greeting: parameters [] do not match [name].",
    ]);
});

test("reports renamed, missing and malformed interpolation parameters", () => {
    const data = resources();
    data["zh-CN"].greeting = "你好，{{user}}。";
    data["zh-CN"].nav.home = "{{name}} 首页";
    data.en.nav.home = "Home {{name}";
    assert.deepEqual(validateTranslations(data), [
        "en nav.home: interpolation marker is not closed as {{name}}.",
        "zh-CN nav.home: parameters [name] do not match [].",
        "zh-CN greeting: parameters [user] do not match [name].",
    ]);
});

test("reads parameter names from unescaped {{- name}} markers", () => {
    const data = resources();
    // Escaped and unescaped forms pass the same value, so they must match.
    data.en.greeting = "Hello, {{- name}}.";
    assert.deepEqual(validateTranslations(data), []);
    data["zh-CN"].greeting = "你好，{{- user}}。";
    assert.deepEqual(validateTranslations(data), [
        "zh-CN greeting: parameters [user] do not match [name].",
    ]);
});

test("reports markers that name no parameter without calling them unclosed", () => {
    const data = resources();
    data.en.nav.home = "Home {{}}";
    data["zh-CN"].nav.home = "首页 {{ }} {{-}}";
    assert.deepEqual(validateTranslations(data), [
        "en nav.home: interpolation marker has no parameter name.",
        "zh-CN nav.home: interpolation marker has no parameter name.",
    ]);
});

test("compares parameters of plural forms as one message", () => {
    const data = resources();
    // A singular wording may omit the count, but the message as a whole must not.
    data.en.itemCount_one = "One item";
    assert.deepEqual(validateTranslations(data), []);
    data["zh-CN"].itemCount_one = "{{total}} 项";
    data["zh-CN"].itemCount_other = "{{total}} 项";
    assert.deepEqual(validateTranslations(data), [
        "zh-CN itemCount: parameters [total] do not match [count].",
    ]);
});

test("requires every plural form selected by each locale's Intl rules", () => {
    const data = resources();
    delete data.en.itemCount_one;
    delete data["zh-CN"].itemCount_other;
    assert.deepEqual(validateTranslations(data), [
        'en itemCount: missing plural form "itemCount_one".',
        'zh-CN itemCount: missing plural form "itemCount_other".',
    ]);
});

test("reports plural keys missing from a locale and forms no locale uses", () => {
    const data = resources();
    delete data["zh-CN"].itemCount_one;
    delete data["zh-CN"].itemCount_other;
    data.en.itemCount_few = "{{count, number}} items";
    data["zh-CN"].fileCount_other = "{{count, number}} 个文件";
    assert.deepEqual(validateTranslations(data), [
        "zh-CN itemCount: missing plural key from en.",
        "zh-CN fileCount: plural key is not in en.",
        'en itemCount_few: plural form "few" is not used by any bundled locale.',
    ]);
});

test("accepts an optional _zero form in any locale", () => {
    // i18next selects key_zero for a count of 0 regardless of language rules.
    const data = resources();
    data.en.itemCount_zero = "No items";
    data["zh-CN"].itemCount_zero = "没有项目";
    assert.deepEqual(validateTranslations(data), []);
    delete data["zh-CN"].itemCount_zero;
    assert.deepEqual(validateTranslations(data), []);
});

test("reports ordinal plural keys as not supported yet", () => {
    const data = resources();
    data.en.place_ordinal_one = "{{count}}st place";
    data.en.place_ordinal_other = "{{count}}th place";
    assert.deepEqual(validateTranslations(data), [
        "en place_ordinal_one: ordinal plurals are not supported yet.",
        "en place_ordinal_other: ordinal plurals are not supported yet.",
    ]);
});

test("derives plural requirements from each locale rather than a fixed list", () => {
    // Russian needs one/few/many/other, so a later locale cannot reuse English forms.
    const ru = {
        nav: { home: "Главная" },
        greeting: "Привет, {{name}}.",
        itemCount_one: "{{count, number}} элемент",
        itemCount_other: "{{count, number}} элемента",
    };
    assert.deepEqual(validateTranslations({ en: resources().en, ru }), [
        'ru itemCount: missing plural form "itemCount_few".',
        'ru itemCount: missing plural form "itemCount_many".',
    ]);
    ru.itemCount_few = "{{count, number}} элемента";
    ru.itemCount_many = "{{count, number}} элементов";
    assert.deepEqual(validateTranslations({ en: resources().en, ru }), []);
});

test("the command exits with an error and lists problems for invalid files", (t) => {
    // Run the real command in an isolated checkout so the exit status that
    // check:frontend relies on is verified, not only the exported function.
    const root = mkdtempSync(join(tmpdir(), "vibemate-i18n-"));
    t.after(() => rmSync(root, { recursive: true, force: true }));
    const data = resources();
    data["zh-CN"].nav.home = "";
    for (const [locale, file] of Object.entries(RESOURCE_FILES)) {
        mkdirSync(dirname(join(root, file)), { recursive: true });
        writeFileSync(join(root, file), JSON.stringify(data[locale]));
    }
    const script = join(
        dirname(fileURLToPath(import.meta.url)),
        "i18n-resources.mjs",
    );
    const result = spawnSync(process.execPath, [script], {
        cwd: root,
        encoding: "utf8",
    });
    assert.equal(result.status, 1);
    assert.match(result.stderr, /zh-CN nav\.home: translation is empty\./);
    assert.match(result.stderr, /1 translation problem\(s\) found\./);
});
