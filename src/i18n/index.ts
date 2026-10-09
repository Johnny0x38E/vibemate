import { createInstance, type i18n } from "i18next";
import { initReactI18next } from "react-i18next";
import en from "../locales/en.json";
import zhCN from "../locales/zh-CN.json";

/** Supported UI locales; configuration values and user content are not localized. */
export type AppLocale = "zh-CN" | "en";

// JSON imports retain key shapes, but not literal strings. This constrains keys;
// interpolation names are checked against both resources by tests, not inferred.
declare module "i18next" {
    interface CustomTypeOptions {
        defaultNS: "translation";
        returnNull: false;
        strictKeyChecks: true;
        resources: { translation: typeof en };
    }
}

/** Map a system language tag to the phase-1 locale, defaulting to English. */
export function resolveSystemLocale(systemLanguage: string): AppLocale {
    // Some platforms use underscores. Match the whole language subtag so unrelated
    // names such as "zhang" cannot accidentally select Chinese.
    const language = systemLanguage.trim().replace(/_/g, "-").split("-")[0];
    return language?.toLowerCase() === "zh" ? "zh-CN" : "en";
}

/**
 * Prepare an independent translator with bundled resources and React bindings.
 * Callers must await this before mounting translated UI; P08 resolves the saved
 * preference first. Importing this module performs no detection or persistence.
 * The React plugin sets its default instance; mount an explicit I18nextProvider
 * when multiple instances are used so each React tree keeps its own translator.
 */
export async function createAppI18n(locale: AppLocale): Promise<i18n> {
    const instance = createInstance();
    await instance.use(initReactI18next).init({
        lng: locale,
        fallbackLng: "en",
        supportedLngs: ["zh-CN", "en"],
        load: "currentOnly",
        defaultNS: "translation",
        // Each instance owns its data so tests and future windows cannot remove or
        // mutate another instance's bundled resources.
        resources: {
            en: { translation: structuredClone(en) },
            "zh-CN": { translation: structuredClone(zhCN) },
        },
        initAsync: false,
        returnNull: false,
        returnEmptyString: false,
        // React escapes text when rendering. Never pass translations to raw HTML.
        interpolation: { escapeValue: false },
    });
    return instance;
}
