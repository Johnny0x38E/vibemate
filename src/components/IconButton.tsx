import { Tooltip } from "@base-ui/react/tooltip";
import type { JSX } from "react";
import { Icon, type IconName } from "./Icon";
import styles from "./IconButton.module.css";

/** Inputs for a compact auxiliary action; label names both button and tooltip. */
export interface IconButtonProps {
    icon: IconName;
    label: string;
    onClick?: () => void;
    /** Submit actions delegate to the containing form; default is non-submit. */
    type?: "button" | "submit";
    /** Keep blocked actions focusable while preventing their callback. */
    blocked?: boolean;
    /** Apply danger styling; callers supply an appropriate removal icon. */
    destructive?: boolean;
    /** Identifies the provider row that owns this control. */
    dataProviderId?: string;
}

/**
 * Auxiliary form action with mouse/keyboard hints. Base UI owns tooltip
 * dismissal and positioning; the portal avoids clipping inside form cards.
 * Page Back also uses this control (icon-only with a tooltip). Primary Save
 * keeps visible text in the secondary-page header. Auxiliary form submits
 * (for example rediscovery) may use type="submit" to preserve Enter behavior.
 */
export function IconButton({
    icon,
    label,
    onClick,
    type = "button",
    blocked = false,
    destructive = false,
    dataProviderId,
}: IconButtonProps): JSX.Element {
    return (
        <Tooltip.Provider delay={300}>
            <Tooltip.Root>
                <Tooltip.Trigger
                    type={type}
                    className={styles["button"]}
                    aria-label={label}
                    aria-disabled={blocked}
                    {...(dataProviderId !== undefined
                        ? { "data-provider-id": dataProviderId }
                        : {})}
                    data-destructive={destructive || undefined}
                    onClick={(event) => {
                        if (blocked) {
                            event.preventDefault();
                            return;
                        }
                        onClick?.();
                    }}
                >
                    <Icon name={icon} />
                </Tooltip.Trigger>
                <Tooltip.Portal>
                    <Tooltip.Positioner
                        className={styles["positioner"]}
                        side="top"
                        sideOffset={6}
                    >
                        <Tooltip.Popup className={styles["tooltip"]}>
                            {label}
                        </Tooltip.Popup>
                    </Tooltip.Positioner>
                </Tooltip.Portal>
            </Tooltip.Root>
        </Tooltip.Provider>
    );
}
