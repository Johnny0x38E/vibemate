import type { JSX } from "react";
import { useTranslation } from "react-i18next";
import styles from "./ProviderModelsCreateGate.module.css";

/** Shown on the Models tab while creating a configuration (no instance id yet). */
export interface ProviderModelsCreateGateProps {
    hidden?: boolean;
}

export function ProviderModelsCreateGate({
    hidden = false,
}: ProviderModelsCreateGateProps): JSX.Element | null {
    const { t } = useTranslation();
    if (hidden) return null;
    return (
        <section
            className={styles["gate"]}
            aria-label={t("providers.tabs.models")}
        >
            <p className={styles["message"]}>
                {t("providers.models.createPending")}
            </p>
        </section>
    );
}
