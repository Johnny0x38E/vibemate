import type { JSX } from "react";
import { WindowControls } from "./WindowControls";
import { WindowDragRegion } from "./WindowDragRegion";
import { usesCustomWindowChrome } from "../lib/desktop/window";
import styles from "./TitlebarChrome.module.css";

interface TitlebarChromeProps {
    /** Drag strip sizing from the parent layout module. */
    dragClassName?: string | undefined;
    /** Optional outer placement class from the parent grid or flex shell. */
    className?: string | undefined;
}

/**
 * Top content-column chrome: draggable strip plus custom window controls on
 * Windows and Linux. macOS relies on native traffic lights in the sidebar.
 */
export function TitlebarChrome({
    dragClassName,
    className,
}: TitlebarChromeProps): JSX.Element {
    const customChrome = usesCustomWindowChrome();
    const outerClass = [styles["chrome"], className]
        .filter((value): value is string => value !== undefined)
        .join(" ");

    return (
        <div className={outerClass}>
            <WindowDragRegion
                className={[styles["drag"], dragClassName]
                    .filter((value): value is string => value !== undefined)
                    .join(" ")}
            />
            {customChrome && <WindowControls />}
        </div>
    );
}
