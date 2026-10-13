import { Tooltip } from "@base-ui/react/tooltip";
import type { JSX } from "react";
import { useTranslation } from "react-i18next";
import type {
    CapabilityOverrides,
    ModelCapabilities,
} from "../../lib/desktop/models";
import {
    capabilityDeclaration,
    supportedModelCapabilities,
} from "./modelCapabilities";
import styles from "./ModelCapabilityIcons.module.css";

interface ModelCapabilityIconsProps {
    capabilities: ModelCapabilities;
    capabilityOverrides: CapabilityOverrides;
}

/**
 * Show one icon per effectively supported capability: user declarations win
 * over catalog evidence in both directions. An unknown or explicitly
 * unsupported capability gets no icon, and every icon names its source.
 */
export function ModelCapabilityIcons({
    capabilities,
    capabilityOverrides,
}: ModelCapabilityIconsProps): JSX.Element | null {
    const { t } = useTranslation();
    const supported = supportedModelCapabilities(
        capabilities,
        capabilityOverrides,
    );
    if (supported.length === 0) return null;
    return (
        <span className={styles["icons"]}>
            {supported.map((capability) => {
                const declaration = capabilityDeclaration(
                    capabilityOverrides,
                    capability,
                );
                const label = t(
                    `providers.models.capabilities.${declaration === null ? "catalog" : "user"}`,
                    {
                        capability: t(
                            `providers.models.capabilities.${capability}`,
                        ),
                    },
                );
                return (
                    <Tooltip.Provider key={capability} delay={300}>
                        <Tooltip.Root>
                            <Tooltip.Trigger
                                render={
                                    <span
                                        role="img"
                                        aria-label={label}
                                        tabIndex={0}
                                    />
                                }
                                className={styles["icon"]}
                            >
                                <svg viewBox="0 0 24 24" aria-hidden="true">
                                    {capability === "thinking" && (
                                        <path d="M9 18h6m-5 3h4m-6-6.2a7 7 0 1 1 8 0c-.8.6-1 1.2-1 2.2h-4c0-1-.2-1.6-1-2.2Z" />
                                    )}
                                    {capability === "imageInput" && (
                                        <>
                                            <rect
                                                x="3"
                                                y="4"
                                                width="18"
                                                height="16"
                                                rx="2"
                                            />
                                            <circle cx="8.5" cy="9" r="1.5" />
                                            <path d="m21 15-5-5L5 20" />
                                        </>
                                    )}
                                    {capability === "toolCalls" && (
                                        <>
                                            <path d="M8 7a2 2 0 1 0-2 2v6a2 2 0 1 0 2 2v-2h8v2a2 2 0 1 0 2-2V9a2 2 0 1 0-2-2v2H8Z" />
                                            <path d="M12 9v5" />
                                        </>
                                    )}
                                </svg>
                            </Tooltip.Trigger>
                            <Tooltip.Portal>
                                <Tooltip.Positioner
                                    className={styles["positioner"]}
                                    side="top"
                                    sideOffset={6}
                                >
                                    <Tooltip.Popup
                                        className={styles["tooltip"]}
                                    >
                                        {label}
                                    </Tooltip.Popup>
                                </Tooltip.Positioner>
                            </Tooltip.Portal>
                        </Tooltip.Root>
                    </Tooltip.Provider>
                );
            })}
        </span>
    );
}
