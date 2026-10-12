# Agent identification marks

These assets identify third-party Agents in vibemate's catalog and details, not
vibemate itself. They do not indicate affiliation, endorsement, an installation,
a connection, or configuration deployment support. Do not redraw, stretch,
recolor, animate, or combine them into the vibemate brand.

## Newly sourced marks (2026-10-12)

| Files                                     | Official source                                                                                                                                                                                                                                                                                | Usage and copyright boundary                                                                                                                                                                                                                                                                                                 |
| ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `claude-code.svg`                         | Anthropic's official Claude site [favicon SVG](https://claude.ai/favicon.svg) (`viewBox="0 0 248 248"`, fill `#D97757`)                                                                                                                                                                                                          | Unmodified official Claude starburst mark in vector form. Shared across light and dark appearances. The earlier 192px Claude Code documentation favicon was a soft raster and was replaced so the mark stays sharp at 20px and 40px. Anthropic owns the mark; no MIT asset license or separate redistribution permission was found. Use here is solely product identification, not a claim of an open-source grant.                                     |
| `codex-light.svg`, `codex-dark.svg`       | OpenAI's official [brand page](https://openai.com/brand/) and its [logo archive](https://cdn.openai.com/brand/openai-logos.zip), `OAI_OpenAI-Blossom_Black.svg` / `OAI_OpenAI-Blossom_White.svg`                                                                                               | Unmodified OpenAI Blossom, used to identify OpenAI's Codex; this is not a separately verified Codex-specific logo. Governed by OpenAI's brand guidelines/usage terms, **not** the Codex code license or vibemate's MIT license. Keep original colors/spacing, subordinate to vibemate branding, and never imply endorsement. |
| `opencode-light.svg`, `opencode-dark.svg` | Anomaly's official [light](https://github.com/anomalyco/opencode/blob/dev/packages/console/app/src/asset/brand/opencode-logo-light-square.svg) / [dark](https://github.com/anomalyco/opencode/blob/dev/packages/console/app/src/asset/brand/opencode-logo-dark-square.svg) square brand assets | Unmodified upstream files. Repository MIT notice is retained in `opencode-LICENSE.txt`; copyright (c) 2025 opencode. This does not grant trademark rights.                                                                                                                                                                   |
| `hermes-light.png`, `hermes-dark.png`     | Nous Research's official `logo.png` / `logo-dark.png` from the [hermes-agent website images](https://github.com/NousResearch/hermes-agent/tree/main/website/static/img)                                                                                                                          | Unmodified official transparent silhouettes (black for light, white for dark). These replace the official `favicon.svg` white-tile icon, which drew a filled rounded background behind the mark. Repository MIT notice is retained in `hermes-LICENSE.txt`; copyright (c) 2025 Nous Research. This does not grant trademark rights.                                       |

The third-party marks remain owned by their respective owners. Source download
availability is not a general redistribution or relicensing grant. Anthropic and
OpenAI assets are excluded from vibemate's MIT license; downstream distributions
must evaluate applicable brand/identification-use rights themselves. Review these
terms before publishing; no legal clearance or separate permission is claimed.

Only passive image content is bundled: SVG checks found no scripts, event
handlers, `foreignObject`, embedded images, or external references. SVG geometry,
colors and original view boxes are preserved; formatter whitespace may differ.
The app loads local assets, never third-party image URLs at runtime.

OpenAI Blossom keeps its official 716×716 canvas and clear-space margins. Because
that padding makes the flower optically smaller than the other catalog marks,
`AgentIcon` scales only the displayed image (165%) and clips the empty margin;
the file itself is unmodified.

## Existing marks

Pi and Grok Build assets are unchanged. Their original sources are recorded in
[frontend documentation](../../docs/frontend.md): Pi's official favicon and the
Grok symbol traced from the February 2025 wordmark, with the documented Wikimedia
source. This new sourcing pass does not establish additional rights for those
existing files.
