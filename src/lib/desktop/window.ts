import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * Whether this build should render custom minimize/maximize/close controls.
 * macOS keeps native traffic lights in the overlay title bar; Windows and Linux
 * use undecorated windows with in-app controls (see platform Tauri config).
 */
export function usesCustomWindowChrome(): boolean {
    if (!isTauri()) return false;
    const platform = import.meta.env.TAURI_ENV_PLATFORM;
    return platform === "windows" || platform === "linux";
}

/** Minimize the main window; no-op in browser preview. */
export async function minimizeWindow(): Promise<void> {
    if (!usesCustomWindowChrome()) return;
    await getCurrentWindow().minimize();
}

/** Toggle maximized state on the main window. */
export async function toggleMaximizeWindow(): Promise<void> {
    if (!usesCustomWindowChrome()) return;
    await getCurrentWindow().toggleMaximize();
}

/** Close the main window. */
export async function closeWindow(): Promise<void> {
    if (!usesCustomWindowChrome()) return;
    await getCurrentWindow().close();
}

/** Read whether the main window is currently maximized. */
export async function isWindowMaximized(): Promise<boolean> {
    if (!usesCustomWindowChrome()) return false;
    return getCurrentWindow().isMaximized();
}

/**
 * Subscribe to maximize changes after an initial read. Returns an unlisten function.
 */
export async function listenWindowMaximized(
    listener: (maximized: boolean) => void,
): Promise<() => void> {
    if (!usesCustomWindowChrome()) return () => {};
    const window = getCurrentWindow();
    async function synchronize(): Promise<void> {
        listener(await window.isMaximized());
    }
    await synchronize();
    return window.onResized(() => {
        void synchronize();
    });
}
