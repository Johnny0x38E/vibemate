import type { JSX, ReactNode } from "react";
import { Icon, type IconName } from "./Icon";
import styles from "./PageModuleHeader.module.css";

/** Sidebar destinations that share one module heading pattern. */
export type ModuleNavIcon = Extract<
    IconName,
    "home" | "providers" | "agents" | "skills" | "mcp" | "settings"
>;

/** Inputs for {@link PageModuleHeader}. */
export interface PageModuleHeaderProps {
    icon: ModuleNavIcon;
    title: ReactNode;
    /** Primary actions or secondary meta aligned on the end of the row. */
    trailing?: ReactNode;
    /** Extra classes on the outer row (spacing below the header, etc.). */
    className?: string;
    headingId?: string;
    tabIndex?: number;
    /** Marks the heading for programmatic focus after async list recovery. */
    dataFocus?: "heading";
}

/**
 * Shared module page title: the same 20 px nav icon as the sidebar, 1 rem
 * semibold heading, and an optional trailing slot (for example "New configuration").
 */
export function PageModuleHeader({
    icon,
    title,
    trailing,
    className,
    headingId,
    tabIndex,
    dataFocus,
}: PageModuleHeaderProps): JSX.Element {
    const rowClass = [styles["header"], className]
        .filter((value): value is string => value !== undefined && value !== "")
        .join(" ");

    return (
        <div className={rowClass}>
            <h1
                className={styles["title"]}
                {...(headingId !== undefined ? { id: headingId } : {})}
                {...(tabIndex !== undefined ? { tabIndex } : {})}
                {...(dataFocus !== undefined
                    ? { "data-focus": dataFocus }
                    : {})}
            >
                <Icon name={icon} />
                <span className={styles["titleText"]}>{title}</span>
            </h1>
            {trailing}
        </div>
    );
}
