import { useId, type JSX } from "react";
import brandMark from "../../assets/brand/mark.svg";
import reference from "../../assets/brand/selected-reference.jpg";

interface BrandLogoProps {
    /** The rail uses the symbol alone; the expanded sidebar uses the full lockup. */
    collapsed: boolean;
    /** Layout and theme text color come from the caller. */
    className?: string | undefined;
}

/**
 * Compose the selected V and the reference's actual lettering in one SVG.
 * The cropped reference supplies a lettering mask, so no substitute font is
 * guessed or shipped. Its white background becomes transparent; currentColor
 * keeps the lettering legible in every theme. The i dot is a brand accent.
 */
export function BrandLogo({
    collapsed,
    className,
}: BrandLogoProps): JSX.Element {
    const id = useId();
    return (
        <svg
            className={className}
            viewBox={collapsed ? "0 0 32 32" : "0 0 138 32"}
            aria-hidden="true"
            focusable="false"
        >
            <image href={brandMark} width="32" height="32" />
            {!collapsed && (
                <>
                    <defs>
                        {/* Convert the selected reference's dark lettering to a white
                        alpha mask. The transfer removes its pale background noise. */}
                        <filter
                            id={`${id}-alpha`}
                            colorInterpolationFilters="sRGB"
                        >
                            <feColorMatrix
                                type="matrix"
                                values="0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 -0.333 -0.333 -0.333 0 1"
                            />
                            <feComponentTransfer>
                                <feFuncA
                                    type="linear"
                                    slope="1.6"
                                    intercept="-0.1"
                                />
                            </feComponentTransfer>
                        </filter>
                        <mask
                            id={`${id}-letters`}
                            maskUnits="userSpaceOnUse"
                            x="0"
                            y="0"
                            width="852"
                            height="160"
                        >
                            <image
                                href={reference}
                                x="-200"
                                y="-744"
                                width="1254"
                                height="1254"
                                filter={`url(#${id}-alpha)`}
                            />
                        </mask>
                    </defs>
                    <g transform="translate(40 6) scale(0.112)">
                        <rect
                            width="852"
                            height="160"
                            fill="currentColor"
                            mask={`url(#${id}-letters)`}
                        />
                        <circle cx="142" cy="16" r="16" fill="#db915b" />
                    </g>
                </>
            )}
        </svg>
    );
}
