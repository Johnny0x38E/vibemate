/** One shared stop on the context-window and maximum-output sliders. */
export interface TokenStop {
    /** Tokens stored for this stop. K steps are multiples of 1024; 1M is 1024². */
    tokens: number;
    /** Short label shown on the slider and the model list. */
    label: string;
}

/**
 * Preset sizes for a context window or a maximum output. Both fields use this
 * list, in this order. Each K step is that many ×1024; 1M is 1,048,576.
 * The stops are evenly spaced on the slider: 64K, 128K, 200K, 500K, 1M.
 */
export const TOKEN_STOPS: readonly TokenStop[] = [
    { tokens: 64 * 1024, label: "64K" },
    { tokens: 128 * 1024, label: "128K" },
    { tokens: 200 * 1024, label: "200K" },
    { tokens: 500 * 1024, label: "500K" },
    { tokens: 1024 * 1024, label: "1M" },
];

/**
 * Move a token count onto the nearest preset. `null` and non-positive numbers
 * stay unset, so a missing catalog value is not given a default stop.
 */
export function nearestTokenStop(tokens: number | null): number | null {
    const first = TOKEN_STOPS[0];
    if (tokens === null || tokens <= 0 || first === undefined) return null;
    let nearest: TokenStop = first;
    let distance = Math.abs(tokens - nearest.tokens);
    for (const stop of TOKEN_STOPS) {
        const next = Math.abs(tokens - stop.tokens);
        if (next < distance) {
            nearest = stop;
            distance = next;
        }
    }
    return nearest.tokens;
}

/** The short label for a stored count, after snapping onto a preset. */
export function tokenStopLabel(tokens: number | null): string | null {
    const snapped = nearestTokenStop(tokens);
    return TOKEN_STOPS.find((stop) => stop.tokens === snapped)?.label ?? null;
}
