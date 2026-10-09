import type { JSX } from "react";

interface WindowDragRegionProps {
    /**
     * Placement and size come from the caller's CSS module, not from this component.
     * CSS module lookups are `string | undefined` under `noUncheckedIndexedAccess`,
     * so the type says so explicitly instead of asserting a value.
     */
    className?: string | undefined;
}

/**
 * An unpainted strip that drags the window where a native title bar used to be.
 *
 * Tauri reads the `data-tauri-drag-region` attribute on mousedown and asks the
 * window to start moving (`core:window:allow-start-dragging` must be granted). A
 * double click on it toggles maximize. Buttons, links, and fields must never be
 * placed inside a drag region, or they would stop receiving clicks as expected.
 */
export function WindowDragRegion({
    className,
}: WindowDragRegionProps): JSX.Element {
    return (
        <div className={className} data-tauri-drag-region aria-hidden="true" />
    );
}
