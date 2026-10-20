import type { TFunction } from "i18next";
import type { JSX } from "react";
import { useTranslation } from "react-i18next";
import type { PiModelFact } from "../../lib/desktop/pi";
import styles from "./PiModelFacts.module.css";

type Translator = TFunction;

/**
 * The Pi model facts of one plan: each setting's value, where it came from and what
 * the plan does with it. Rust builds every row, so this component only translates
 * the known tokens, formats token counts for the active language and adds the fixed
 * notes about what vibemate does not edit. It never recomputes or edits a value.
 */
export function PiModelFacts({ facts }: { facts: PiModelFact[] }): JSX.Element {
    const { t, i18n } = useTranslation();
    const numbers = new Intl.NumberFormat(
        i18n.resolvedLanguage ?? i18n.language,
    );
    return (
        <section
            className={styles["facts"]}
            aria-label={t("agents.modelConfig.piFacts.heading")}
        >
            <h3 className={styles["heading"]}>
                {t("agents.modelConfig.piFacts.heading")}
            </h3>
            <dl className={styles["list"]}>
                {facts.map((fact) => (
                    <div className={styles["fact"]} key={fact.item}>
                        <dt className={styles["term"]}>
                            {t(`agents.modelConfig.piFacts.items.${fact.item}`)}
                        </dt>
                        <dd className={styles["definition"]}>
                            <span className={styles["value"]}>
                                {valueText(fact, t, numbers)}
                            </span>
                            <span>
                                {t(
                                    `agents.modelConfig.piFacts.sources.${fact.source}`,
                                )}
                            </span>
                            <span>
                                {t(
                                    `agents.modelConfig.piFacts.states.${fact.state}`,
                                    { target: fact.target },
                                )}
                            </span>
                        </dd>
                    </div>
                ))}
            </dl>
            <ul className={styles["notes"]}>
                <li>{t("agents.modelConfig.piFacts.notes.requestSettings")}</li>
                <li>{t("agents.modelConfig.piFacts.notes.maxOutput")}</li>
                <li>{t("agents.modelConfig.piFacts.notes.precedence")}</li>
            </ul>
        </section>
    );
}

/**
 * Display text for one fact value. Known Rust tokens are translated; token counts
 * are formatted for the locale; anything unrecognized is shown exactly as received.
 */
function valueText(
    fact: PiModelFact,
    t: Translator,
    numbers: Intl.NumberFormat,
): string {
    if (fact.value === null)
        return t("agents.modelConfig.piFacts.notStatedValue");
    if (fact.item === "contextWindow" || fact.item === "maxOutputTokens") {
        const count = Number(fact.value);
        return Number.isSafeInteger(count)
            ? t("agents.modelConfig.piFacts.tokens", {
                  value: numbers.format(count),
              })
            : fact.value;
    }
    if (fact.item === "reasoning") {
        if (fact.value === "true")
            return t("agents.modelConfig.piFacts.reasoningSupported");
        if (fact.value === "false")
            return t("agents.modelConfig.piFacts.reasoningNotSupported");
    }
    if (fact.item === "imageInput") {
        if (fact.value === "text, image")
            return t("agents.modelConfig.piFacts.textAndImage");
        if (fact.value === "text")
            return t("agents.modelConfig.piFacts.textOnly");
    }
    return fact.value;
}
