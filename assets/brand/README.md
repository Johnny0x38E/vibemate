# vibemate brand assets

The maintainer selected the first logo concept on 2026-10-10. It pairs a
rounded V with a separate rising stroke: deep forest green `#324e40`, sage
`#a8b8a7`, and off-white `#f7f8f7`. `concept.png` is the selected AI-generated
Image Gen reference. SVG files are the manually redrawn production masters;
they remove raster shading and use flat, scalable shapes.

- `mark.svg`: transparent two-color symbol for light surfaces.
- `app-icon.svg`: off-white symbol on a forest-green tile, with transparent
  outside corners. The silhouette remains visible on light and dark surfaces.
- `src/components/BrandWordmark.tsx`: custom SVG letter paths, with round
  terminals and no external font dependency. Its color follows the UI theme.

Desktop PNG, ICO, and ICNS files in `src-tauri/icons/` are generated from the
app icon by the repository's locked Tauri CLI:

```sh
pnpm exec tauri icon assets/brand/app-icon.svg --output /tmp/vibemate-icons
```

Copy only the existing desktop filenames from that output to `src-tauri/icons/`.
The CLI also generates mobile files; mobile targets are outside this product's
current scope. Do not edit generated icon geometry independently of the SVG.

The selected reference was generated with the built-in Image Gen tool. Its
brief was a convergent V with broad rounded strokes, forest/sage colors, a
rounded lowercase vibemate wordmark, and monochrome/app-icon applications.
The SVGs and custom wordmark are project artwork distributed under the
repository's MIT license. The old cyan/yellow Tauri rings are not part of the
vibemate identity.
