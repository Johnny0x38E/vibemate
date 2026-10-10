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

describe("bundled translations", () => {
    it("gives both resources the same TypeScript key shape", () => {
        // Type-level only: `pnpm run typecheck` fails if the JSON shapes differ.
        // Values, parameters and plural forms are checked by `pnpm run check:i18n`.
        expectTypeOf(zhCN).toEqualTypeOf(en);
    });

    it("initializes the requested language before returning a ready instance", async () => {
        const instance = await createAppI18n("zh-CN");
        expect(instance.isInitialized).toBe(true);
        expect(instance.resolvedLanguage).toBe("zh-CN");
        expect(instance.t("settings.startup.loading")).toBe(
            "正在读取语言偏好…",
        );
    });

    it("resolves nested message keys in Chinese", async () => {
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
        // The bundled UI has no plural message yet, so this adds temporary
        // fixture messages to one independent instance. Fixture keys are not in
        // the bundled resource type; a string-keyed view of the bound translator
        // calls them without weakening the typed keys used by application code.
        const instance = await createAppI18n("en");
        const fixtures = {
            en: {
                itemCount_one: "{{count, number}} item",
                itemCount_other: "{{count, number}} items",
            },
            "zh-CN": { itemCount_other: "{{count, number}} 项" },
        } as const;
        for (const [locale, messages] of Object.entries(fixtures))
            instance.addResourceBundle(
                locale,
                "translation",
                { fixture: messages },
                true,
                true,
            );
        const english: (key: string, options: { count: number }) => string =
            instance.getFixedT("en");
        const chinese: (key: string, options: { count: number }) => string =
            instance.getFixedT("zh-CN");
        expect(english("fixture.itemCount", { count: 1 })).toBe("1 item");
        expect(english("fixture.itemCount", { count: 1200 })).toBe(
            "1,200 items",
        );
        // Chinese has only the "other" category, including for a count of one.
        expect(chinese("fixture.itemCount", { count: 1 })).toBe("1 项");
        expect(chinese("fixture.itemCount", { count: 1200 })).toBe("1,200 项");
        // Fixtures stay inside this instance; a new translator has none of them.
        const fresh: (key: string, options: { count: number }) => string = (
            await createAppI18n("en")
        ).getFixedT("en");
        expect(fresh("fixture.itemCount", { count: 1 })).toBe(
            "fixture.itemCount",
        );
    });

    it("exposes provider models list titles and footer hints in both locales", async () => {
        const enInstance = await createAppI18n("en");
        expect(enInstance.t("providers.models.filter.selected")).toBe(
            "Selected",
        );
        expect(enInstance.t("providers.models.filter.all")).toBe("All");
        expect(enInstance.t("providers.models.allSelectedLoaded")).toBe(
            "All selected models are shown.",
        );
        expect(enInstance.t("providers.models.upstreamAllLoaded")).toBe(
            "All loaded upstream models are shown.",
        );
        expect(enInstance.t("providers.models.unsavedLeave")).toBe(
            "You have unsaved model checkbox changes. Leave without saving?",
        );
        const zhInstance = await createAppI18n("zh-CN");
        expect(zhInstance.t("providers.models.filter.selected")).toBe("已选");
        expect(zhInstance.t("providers.models.filter.all")).toBe("全部");
        expect(zhInstance.t("providers.models.allSelectedLoaded")).toBe(
            "已展示全部已选模型",
        );
        expect(zhInstance.t("providers.models.upstreamAllLoaded")).toBe(
            "已展示全部已加载的上游模型",
        );
        expect(zhInstance.t("providers.models.unsavedLeave")).toBe(
            "模型勾选尚未保存，确定要离开吗？",
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
