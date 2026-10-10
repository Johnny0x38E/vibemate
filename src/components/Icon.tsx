import type { JSX, ReactNode } from "react";

/** Small decorative icons shared by navigation, appearance and page controls. */
export type IconName =
    | "home"
    | "providers"
    | "agents"
    | "skills"
    | "mcp"
    | "settings"
    | "collapse"
    | "system"
    | "light"
    | "dark"
    | "back"
    | "close"
    | "plus"
    | "edit"
    | "eyeOff"
    | "logFile"
    | "folder"
    | "check"
    | "error";

/** Render an inline SVG; the containing control must provide its accessible name. */
export function Icon({ name }: { name: IconName }): JSX.Element {
    let content: ReactNode;
    switch (name) {
        case "home":
            content = <path d="m3 10 9-7 9 7v10H3Zm6 10v-7h6v7" />;
            break;
        case "providers":
            content = (
                <>
                    <rect x="3" y="4" width="18" height="7" rx="2" />
                    <rect x="3" y="14" width="18" height="7" rx="2" />
                    <path d="M7 7.5h.01M7 17.5h.01" />
                </>
            );
            break;
        case "agents":
            content = (
                <>
                    <rect x="4" y="6" width="16" height="14" rx="3" />
                    <path d="M12 3v3M8 11h.01M16 11h.01M8 16h8" />
                </>
            );
            break;
        case "skills":
            content = (
                <>
                    <path d="m12 3 9 5-9 5-9-5Zm-9 9 9 5 9-5M3 16l9 5 9-5" />
                </>
            );
            break;
        case "mcp":
            content = (
                <>
                    <rect x="2" y="8" width="6" height="8" rx="1" />
                    <rect x="16" y="8" width="6" height="8" rx="1" />
                    <path d="M8 12h8" />
                </>
            );
            break;
        case "settings":
            content = (
                <>
                    <circle cx="12" cy="12" r="3" />
                    <path d="m10 3-1 3-3 1-3-1-1 4 2 2-2 2 1 4 3-1 3 1 1 3h4l1-3 3-1 3 1 1-4-2-2 2-2-1-4-3 1-3-1-1-3Z" />
                </>
            );
            break;
        case "collapse":
            content = (
                <>
                    <rect x="3" y="4" width="18" height="16" rx="2" />
                    <path d="M9 4v16" />
                </>
            );
            break;
        case "system":
            content = (
                <>
                    <rect x="3" y="4" width="18" height="13" rx="2" />
                    <path d="M12 17v4M8 21h8" />
                </>
            );
            break;
        case "light":
            content = (
                <>
                    <circle cx="12" cy="12" r="4" />
                    <path d="M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.4 1.4M17.6 17.6L19 19M5 19l1.4-1.4M17.6 6.4L19 5" />
                </>
            );
            break;
        case "dark":
            content = (
                <path d="M20.5 14.2A9 9 0 0 1 9.8 3.5a9 9 0 1 0 10.7 10.7Z" />
            );
            break;
        case "back":
            content = <path d="M19 12H5m6-6-6 6 6 6" />;
            break;
        case "close":
            content = <path d="M6 6l12 12M18 6 6 18" />;
            break;
        case "plus":
            content = <path d="M12 5v14M5 12h14" />;
            break;
        case "eyeOff":
            content = (
                <>
                    <path d="m3 3 18 18M10.6 10.6a2 2 0 0 0 2.8 2.8M9.5 5.4A10 10 0 0 1 12 5c5.5 0 9 7 9 7a16 16 0 0 1-3 3.8M6.2 6.2A20 20 0 0 0 3 12s3.5 7 9 7a10 10 0 0 0 5-1.5" />
                </>
            );
            break;
        case "edit":
            content = <path d="M4 20h4L19 9l-4-4L4 16Zm9-13 4 4" />;
            break;
        case "logFile":
            content = (
                <>
                    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z" />
                    <path d="M14 2v6h6M16 13H8M16 17H8M10 9H8" />
                </>
            );
            break;
        case "folder":
            content = (
                <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7l-2-2H4a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2Z" />
            );
            break;
        case "check":
            content = <path d="M5 12l5 5L20 7" />;
            break;
        case "error":
            content = (
                <>
                    <circle cx="12" cy="12" r="9" />
                    <path d="M8 8l8 8M16 8l-8 8" />
                </>
            );
            break;
    }
    return (
        <svg
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
        >
            {content}
        </svg>
    );
}
