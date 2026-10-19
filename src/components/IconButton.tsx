import { Tooltip } from "@base-ui/react/tooltip";
import { useState, type JSX } from "react";
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
    /** Keep the icon target and focus ring while omitting its frame. */
    borderless?: boolean;
    /** Identifies the provider row that owns this control. */
    dataProviderId?: string;
    /** When set, exposes expand/collapse state for disclosure controls. */
    expanded?: boolean;
    /** Element id of the region this disclosure controls (`aria-controls`). */
    controls?: string;
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
    borderless = false,
    dataProviderId,
    expanded,
    controls,
}: IconButtonProps): JSX.Element {
    // Hover/focus still owns the normal open state. A press closes the hint
    // explicitly because its action can replace the page that owns this
    // trigger (for example the Agent list opening a detail page), and a
    // portaled hint must not outlive that unmount.
    const [tooltipOpen, setTooltipOpen] = useState(false);
    return (
        <Tooltip.Provider delay={300}>
            <Tooltip.Root open={tooltipOpen} onOpenChange={setTooltipOpen}>
                <Tooltip.Trigger
                    type={type}
                    className={[
                        styles["button"],
                        ...(borderless ? [styles["borderless"]] : []),
                    ].join(" ")}
                    aria-label={label}
                    aria-disabled={blocked}
                    {...(dataProviderId !== undefined
                        ? { "data-provider-id": dataProviderId }
                        : {})}
                    {...(expanded !== undefined
                        ? { "aria-expanded": expanded }
                        : {})}
                    {...(controls !== undefined
                        ? { "aria-controls": controls }
                        : {})}
                    data-destructive={destructive || undefined}
                    onPointerDown={() => {
                        setTooltipOpen(false);
                    }}
                    onClick={(event) => {
                        setTooltipOpen(false);
                        if (blocked) {
                            event.preventDefault();
                            return;
                        }
                        onClick?.();
                    }}
                >
                    <Icon name={icon} />
                </Tooltip.Trigger>
                {/* Mount the portal only while open: a page change can unmount
                    this trigger, and a hidden portaled hint must not survive. */}
                {tooltipOpen && (
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
                )}
            </Tooltip.Root>
        </Tooltip.Provider>
    );
}
