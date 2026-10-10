import type { JSX } from "react";

interface BrandWordmarkProps {
    /** Size and visibility come from the caller's CSS module. */
    className?: string | undefined;
}

/**
 * The "vibemate" wordmark drawn as custom rounded SVG paths, so it needs no font file.
 *
 * Letters share one grid: x-height from y=9 to the y=19 baseline, round bowls of
 * radius 5, and broad round terminals that follow the selected V identity.
 * `currentColor` keeps the wordmark legible in light and dark appearance. The
 * graphic is decorative: the enclosing control names the brand.
 */
export function BrandWordmark({ className }: BrandWordmarkProps): JSX.Element {
    return (
        <svg
            className={className}
            viewBox="-1 0 103 22"
            aria-hidden="true"
            focusable="false"
        >
            <g
                fill="none"
                stroke="currentColor"
                strokeWidth="3.2"
                strokeLinecap="round"
                strokeLinejoin="round"
            >
                <path d="M1 9 L5 19 L9 9" />
                <path d="M13.5 9 V19" />
                <path d="M18 3 V19 M18 14 a5 5 0 1 0 10 0 a5 5 0 1 0 -10 0" />
                <path d="M32 14 H42 A5 5 0 1 0 40.54 17.54" />
                <path d="M46 9 V19 M46 13 a4 4 0 0 1 8 0 V19 M54 13 a4 4 0 0 1 8 0 V19" />
                <path d="M66.5 14 a5 5 0 1 0 10 0 a5 5 0 1 0 -10 0 M76.5 9 V19" />
                <path d="M82 5 V16 a3 3 0 0 0 3 3 M79.5 9 H85" />
                <path d="M89 14 H99 A5 5 0 1 0 97.54 17.54" />
            </g>
            <circle cx="13.5" cy="5" r="1.6" fill="currentColor" />
        </svg>
    );
}
