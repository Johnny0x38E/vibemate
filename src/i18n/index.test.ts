import { describe, expect, expectTypeOf, it } from "vitest";
import en from "../locales/en.json";
import zhCN from "../locales/zh-CN.json";
import { createAppI18n, resolveSystemLocale } from "./index";

describe("system locale resolution", () => {
    it.each([
        ["zh", "zh-CN"],
        ["zh-CN", "zh-CN"],
        ["zh-TW", "zh-CN"],
        ["zh-Hant-HK", "zh-CN"],
        ["ZH_hant", "zh-CN"],
        ["en", "en"],
        ["en-GB", "en"],
        ["fr-FR", "en"],
        ["", "en"],
        ["zhang", "en"],
    ])("maps %s to %s", (input, expected) => {
        expect(resolveSystemLocale(input)).toBe(expected);
    });
});

// Flatten only translation data, not configuration or user content. P09 will
// extend this check with empty-value and plural validation for all future UI.
function placeholders(resource: object, prefix = ""): Record<string, string[]> {
    const result: Record<string, string[]> = {};
    for (const [key, entry] of Object.entries(resource)) {
        const value: unknown = entry;
        const path = prefix ? `${prefix}.${key}` : key;
        if (typeof value === "string") {
            result[path] = Array.from(value.matchAll(/{{\s*([^},\s]+)[^}]*}}/g))
                .map((match) => match[1] ?? "")
                .sort();
        } else if (typeof value === "object" && value !== null) {
            Object.assign(result, placeholders(value, path));
        } else {
            throw new Error(`Invalid translation at ${path}`);
        }
    }
    return result;
}

describe("bundled translations", () => {
    it("keeps keys and interpolation names aligned across resources", () => {
        expectTypeOf(zhCN).toEqualTypeOf(en);
        expect(placeholders(zhCN)).toEqual(placeholders(en));
    });

    it("initializes the requested language before returning a ready instance", async () => {
        const instance = await createAppI18n("zh-CN");
        expect(instance.isInitialized).toBe(true);
        expect(instance.resolvedLanguage).toBe("zh-CN");
        expect(instance.t("settings.startup.loading")).toBe(
            "正在读取语言偏好…",
        );
    });

    it("preserves interpolated values inside translated sentences", async () => {
        const instance = await createAppI18n("zh-CN");
        expect(instance.t("settings.appearance.modes.light")).toBe("浅色");
    });

    it("exposes settings tab labels in both locales", async () => {
        const enInstance = await createAppI18n("en");
        expect(enInstance.t("settings.tabs.general")).toBe("General");
        expect(enInstance.t("settings.tabs.about")).toBe("About");
        expect(enInstance.t("settings.tabs.ariaLabel")).toBe(
            "Settings sections",
        );
        const zhInstance = await createAppI18n("zh-CN");
        expect(zhInstance.t("settings.tabs.general")).toBe("常规");
        expect(zhInstance.t("settings.tabs.about")).toBe("关于");
        expect(zhInstance.t("settings.tabs.ariaLabel")).toBe("设置分区");
    });

    it("uses locale-aware plural rules and Intl number formatting", async () => {
        const instance = await createAppI18n("en");
        expect(instance.t("app.areaCount", { count: 1 })).toBe("1 area");
        expect(instance.t("app.areaCount", { count: 1200 })).toBe(
            "1,200 areas",
        );
        await instance.changeLanguage("zh-CN");
        expect(instance.t("app.areaCount", { count: 1 })).toBe("1 个领域");
        expect(instance.t("app.areaCount", { count: 1200 })).toBe(
            "1,200 个领域",
        );
    });

    it("provides Intl date formatting for both locales", async () => {
        const date = new Date("2026-10-09T12:00:00Z");
        const options = {
            year: "numeric",
            month: "long",
            timeZone: "UTC",
        } as const;
        for (const locale of ["en", "zh-CN"] as const) {
            const instance = await createAppI18n(locale);
            expect(
                instance.services.formatter?.format(
                    date,
                    "datetime",
                    locale,
                    options,
                ),
            ).toBe(new Intl.DateTimeFormat(locale, options).format(date));
        }
    });

    it("falls back to English rather than a key when Chinese resources are absent", async () => {
        const instance = await createAppI18n("zh-CN");
        instance.removeResourceBundle("zh-CN", "translation");
        expect(instance.t("settings.startup.loading")).toBe(
            "Reading language preference…",
        );
        // Independent instances must not share mutable resource stores.
        const next = await createAppI18n("zh-CN");
        expect(next.t("settings.startup.loading")).toBe("正在读取语言偏好…");
    });
});
