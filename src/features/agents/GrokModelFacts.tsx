import type { JSX } from "react";
import { useTranslation } from "react-i18next";
import type { GrokContextWindow, GrokImageInput } from "../../lib/desktop/grok";
import styles from "./ModelFacts.module.css";

/** Translate Rust's context summary without deriving native settings or edits. */
export function GrokModelFacts({
    context,
    imageInput,
    alias,
}: {
    context: GrokContextWindow;
    imageInput: GrokImageInput;
    alias: string;
}): JSX.Element {
    const { t, i18n } = useTranslation();
    const count = context.value === null ? null : Number(context.value);
    // Integer strings preserve native i64 values beyond JavaScript's safe range.
    const value =
        count !== null && Number.isSafeInteger(count)
            ? new Intl.NumberFormat(
                  i18n.resolvedLanguage ?? i18n.language,
              ).format(count)
            : context.value;
    return (
        <section
            className={styles["facts"]}
            aria-label={t("agents.modelConfig.grokFacts.heading")}
        >
            <h3 className={styles["heading"]}>
                {t("agents.modelConfig.grokFacts.heading")}
            </h3>
            <dl className={styles["list"]}>
                <div className={styles["fact"]}>
                    <dt className={styles["term"]}>
                        {t("agents.modelConfig.piFacts.items.contextWindow")}
                    </dt>
                    <dd className={styles["definition"]}>
                        <span className={styles["value"]}>
                            {value === null
                                ? t(
                                      context.state === "unchanged"
                                          ? "agents.modelConfig.grokFacts.existingValue"
                                          : "agents.modelConfig.piFacts.notStatedValue",
                                  )
                                : t("agents.modelConfig.piFacts.tokens", {
                                      value,
                                  })}
                        </span>
                        <span>
                            {t(
                                `agents.modelConfig.piFacts.sources.${context.source}`,
                            )}
                        </span>
                        <span>
                            {context.state === "notStated"
                                ? t("agents.modelConfig.grokFacts.defaultState")
                                : t(
                                      `agents.modelConfig.piFacts.states.${context.state}`,
                                      {
                                          target: `model.${alias}.context_window`,
                                      },
                                  )}
                        </span>
                    </dd>
                </div>
                <div className={styles["fact"]}>
                    <dt className={styles["term"]}>
                        {t("agents.modelConfig.piFacts.items.imageInput")}
                    </dt>
                    <dd className={styles["definition"]}>
                        <span className={styles["value"]}>
                            {imageInput.value === null
                                ? t(
                                      imageInput.state === "unchanged"
                                          ? "agents.modelConfig.grokFacts.existingValue"
                                          : "agents.modelConfig.piFacts.notStatedValue",
                                  )
                                : t(
                                      imageInput.value
                                          ? "agents.modelConfig.piFacts.textAndImage"
                                          : "agents.modelConfig.piFacts.textOnly",
                                  )}
                        </span>
                        <span>
                            {t(
                                `agents.modelConfig.piFacts.sources.${imageInput.source}`,
                            )}
                        </span>
                        <span>
                            {imageInput.state === "notStated"
                                ? t(
                                      "agents.modelConfig.grokFacts.imageDefaultState",
                                  )
                                : t(
                                      `agents.modelConfig.piFacts.states.${imageInput.state}`,
                                      { target: `model.${alias}.vision` },
                                  )}
                        </span>
                    </dd>
                </div>
            </dl>
            <ul className={styles["notes"]}>
                <li>{t("agents.modelConfig.grokFacts.notes.context")}</li>
                <li>{t("agents.modelConfig.grokFacts.notes.reasoning")}</li>
                <li>
                    {t("agents.modelConfig.grokFacts.notes.requestSettings")}
                </li>
            </ul>
        </section>
    );
}
