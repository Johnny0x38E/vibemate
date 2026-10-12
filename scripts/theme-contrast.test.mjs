/*
 * Palette values in `src/themes.css` are easy to edit and hard to review by eye,
 * so this check reads that file and enforces the contrast floors the interface
 * relies on:
 *
 * - chip labels on their own capsule background (4.5:1, WCAG AA text);
 * - danger text on the page background (4.5:1; the destructive button label and
 *   the notification tone icon use these foregrounds directly);
 * - success and danger foregrounds on the page background and on the tinted
 *   notification card (3:1, WCAG non-text contrast for the 8 px status dot and
 *   the 18 px tone icon).
 *
 * It runs with the other release checks because `src` may not import Node file
 * APIs; the desktop boundary owns filesystem access in the application.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const THEMES_CSS = readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), "..", "src", "themes.css"),
    "utf8",
);

/** The brightness suffixes every palette declares. */
const BRIGHTNESSES = ["light", "dark"];

/** How much of the status background the notification card keeps. */
const NOTIFICATION_TINT = 0.55;

/**
 * Palette blocks in file order. The default theme has no `data-theme`
 * selector, so it is read from its `data-palette` entry instead.
 */
function readPalettes(source) {
    const palettes = [];
    for (const block of source.matchAll(/(:root[^{]*)\{([\s\S]*?)\n\}/g)) {
        const selector = block[1] ?? "";
        const body = block[2] ?? "";
        const name =
            selector.match(/data-theme="(\w+)"/)?.[1] ??
            selector.match(/data-palette="(\w+)"/)?.[1];
        if (name === undefined) continue;
        const values = {};
        for (const declaration of body.matchAll(
            /(--[\w-]+):\s*(#[0-9a-fA-F]{6})\s*;/g,
        )) {
            values[declaration[1]] = declaration[2].toLowerCase();
        }
        palettes.push({ name, values });
    }
    return palettes;
}

/** Read one palette value, failing loudly when a theme lost the token. */
function token(palette, property) {
    const value = palette.values[property];
    assert.ok(value !== undefined, `${palette.name} is missing ${property}`);
    return value;
}

/** WCAG relative luminance of an `#rrggbb` color. */
function luminance(color) {
    const linear = (offset) => {
        const channel = parseInt(color.slice(1 + offset, 3 + offset), 16) / 255;
        return channel <= 0.04045
            ? channel / 12.92
            : ((channel + 0.055) / 1.055) ** 2.4;
    };
    return 0.2126 * linear(0) + 0.7152 * linear(2) + 0.0722 * linear(4);
}

/** WCAG contrast ratio between two `#rrggbb` colors, from 1 to 21. */
function contrast(foreground, background) {
    const first = luminance(foreground);
    const second = luminance(background);
    return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
}

/** Mix two `#rrggbb` colors the way `color-mix(in srgb, ...)` does. */
function mix(foreground, background, weight) {
    const channels = [0, 2, 4].map((offset) => {
        const front = parseInt(foreground.slice(1 + offset, 3 + offset), 16);
        const back = parseInt(background.slice(1 + offset, 3 + offset), 16);
        return Math.round(front * weight + back * (1 - weight));
    });
    return `#${channels
        .map((channel) => channel.toString(16).padStart(2, "0"))
        .join("")}`;
}

const palettes = readPalettes(THEMES_CSS);

test("themes.css declares every built-in palette", () => {
    assert.deepEqual(
        palettes.map((palette) => palette.name),
        [
            "macos",
            "windows",
            "forest",
            "graphite",
            "linen",
            "iris",
            "ocean",
            "notion",
        ],
    );
});

for (const palette of palettes) {
    for (const brightness of BRIGHTNESSES) {
        test(`${palette.name} ${brightness} keeps text and icons readable`, () => {
            const pageBackground = token(
                palette,
                `--theme-background-${brightness}`,
            );
            const surface = token(palette, `--theme-surface-${brightness}`);

            for (const role of ["info", "count"]) {
                const foreground = token(
                    palette,
                    `--theme-chip-${role}-fg-${brightness}`,
                );
                const background = token(
                    palette,
                    `--theme-chip-${role}-bg-${brightness}`,
                );
                const ratio = contrast(foreground, background);
                assert.ok(
                    ratio >= 4.5,
                    `${role} chip ${foreground} on ${background} is ${ratio.toFixed(2)}:1`,
                );
            }

            const danger = token(
                palette,
                `--theme-status-danger-fg-${brightness}`,
            );
            const success = token(
                palette,
                `--theme-status-success-fg-${brightness}`,
            );
            const dangerText = contrast(danger, pageBackground);
            assert.ok(
                dangerText >= 4.5,
                `danger text ${danger} on ${pageBackground} is ${dangerText.toFixed(2)}:1`,
            );
            const successIcon = contrast(success, pageBackground);
            assert.ok(
                successIcon >= 3,
                `success icon ${success} on ${pageBackground} is ${successIcon.toFixed(2)}:1`,
            );

            for (const [role, foreground] of [
                ["success", success],
                ["danger", danger],
            ]) {
                // The notification card keeps 55% of the status background over
                // the surface (see Notifications.module.css).
                const card = mix(
                    token(palette, `--theme-status-${role}-bg-${brightness}`),
                    surface,
                    NOTIFICATION_TINT,
                );
                const ratio = contrast(foreground, card);
                assert.ok(
                    ratio >= 3,
                    `${role} icon ${foreground} on the notification card ${card} is ${ratio.toFixed(2)}:1`,
                );
            }
        });
    }
}
