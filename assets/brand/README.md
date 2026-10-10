# vibemate brand assets

The maintainer selected the first logo concept on 2026-10-10. It pairs a
rounded V with a separate rising stroke: deep forest green `#324e40`, sage
`#a8b8a7`, and off-white `#f7f8f7`. `concept.png` is the selected AI-generated
Image Gen reference. SVG files are the manually redrawn production masters;
they remove raster shading and use flat, scalable shapes.

- `mark.svg`: transparent two-color symbol for light surfaces.
- `app-icon.svg`: the selected forest/sage V on an off-white app tile, with
  transparent outside corners. The V colors stay faithful to the reference.
- `app-icon-macos.svg`: macOS tile with the same artwork scaled to 85% around
  the canvas center. Its 408 px tile on a 512 px canvas leaves transparent
  margins so the Dock icon does not appear oversized. The tile uses paired
  cubic curves with matching tangents and curvature at their joins, and zero
  curvature where they meet straight edges, for softer continuous corners. Each corner transition spans 160 px before
  scaling, distributing the bend over more of the edge.
- `logo-expanded.svg` / `logo-collapsed.svg`: full horizontal lockup and symbol-only artwork.
- `src/components/BrandLogo.tsx`: one inline SVG lockup with a lettering mask from the actual selected reference, with no external font
  dependency. Lettering follows UI text color; the raised i dot is warm orange.

Desktop PNG and ICO files in `src-tauri/icons/` are generated from the
app icon by the repository's locked Tauri CLI:

```sh
pnpm exec tauri icon assets/brand/app-icon.svg --output /tmp/vibemate-icons
```

Copy only the existing PNG and ICO filenames from that output to `src-tauri/icons/`.
Generate the macOS ICNS separately to preserve its Dock-specific padding:

```sh
pnpm exec tauri icon assets/brand/app-icon-macos.svg --output /tmp/vibemate-macos-icons
cp /tmp/vibemate-macos-icons/icon.icns src-tauri/icons/icon.icns
```

Only copy `icon.icns` from the macOS output; other platforms use the original
master.
The CLI also generates mobile files; mobile targets are outside this product's
current scope. Do not edit generated icon geometry independently of the SVG.

The selected reference was generated with the built-in Image Gen tool. Its
brief was a convergent V with broad rounded strokes, forest/sage colors, a
rounded lowercase vibemate wordmark, and monochrome/app-icon applications.
The SVGs and reference-based wordmark are project artwork distributed under the
repository's MIT license. The old cyan/yellow Tauri rings are not part of the
vibemate identity.

The production V was corrected against the maintainer's selected reference on
2026-10-10: its lower turn is shallow and stays within the silhouette, rather
than extending into the earlier redraw's long tail. Both logo lockups embed the
same corrected icon artwork. The React lockup references the transparent two-color `mark.svg` directly
and lets the lettering follow the active theme's text color.

`selected-reference.jpg` is the maintainer's supplied copy of the selected
concept. The wordmark uses its actual letter shapes instead of a substitute
font: a cropped SVG mask turns pale background pixels transparent and fills
letter pixels with the current text color. The original i dot is masked out;
a separate `#db915b` circle has extra clearance above its stem. The expanded
export embeds that lettering reference; the collapsed export is pure SVG.
