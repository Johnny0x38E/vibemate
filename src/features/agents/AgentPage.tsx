import type { JSX, ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ProviderPage } from "../providers/ProviderPage";

/** Read-only Agent details with the shared title and top Back control. */
export interface AgentPageProps {
    title: ReactNode;
    /** Status and version summary shown at the end of the title row. */
    meta?: ReactNode;
    onBack: () => void;
    backBlocked?: boolean;
    children: ReactNode;
}

/** Agent details share the secondary-page shell while retaining their own Back label. */
export function AgentPage({
    title,
    meta,
    onBack,
    backBlocked = false,
    children,
}: AgentPageProps): JSX.Element {
    const { t } = useTranslation();
    return (
        <ProviderPage
            title={title}
            meta={meta}
            onBack={onBack}
            backBlocked={backBlocked}
            backAriaLabel={t("agents.backLabel")}
            scrollContent
        >
            {children}
        </ProviderPage>
    );
}
