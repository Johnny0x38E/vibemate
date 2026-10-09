# Releases

`CHANGELOG.md` is the source of release notes. Write entries in English under
`## [Unreleased]`, using sections such as `### Added`, `### Changed`, and
`### Fixed`. Describe user-visible behavior and known limitations.

## Prepare a stable version

1. Update the version in `package.json`, `src-tauri/Cargo.toml`, and
   `src-tauri/tauri.conf.json`. Refresh and commit both lockfiles.
2. Move the ready changes into `## [X.Y.Z] - YYYY-MM-DD` in `CHANGELOG.md` and
   leave an `Unreleased` section for subsequent work. The heading is part of the
   release format; do not rename it or duplicate the same version.
3. Run the checks in `CONTRIBUTING.md`, including `npm run test:release`.
   Run `node scripts/release-notes.mjs vX.Y.Z` to preview the exact release body.
4. Commit the release changes. When ready to start remote builds, create and
   push the matching `vX.Y.Z` Git tag to GitHub.

The current scaffold has only an `Unreleased` entry. It is not a published
0.1.0 release; tagging it before adding versioned notes will fail validation.

## GitHub Actions behavior

`.github/workflows/release.yml` runs on stable version tags. It checks version
agreement across the manifests and npm lockfile, tests the note extractor, and
reads only the matching changelog section. Missing, duplicate, or empty notes
stop the workflow before a draft is created. Changelog content is passed as data,
never interpolated into a shell command or executable script.

The workflow creates a **draft** GitHub Release, then builds and attaches:

| Target              | Bundles             |
| ------------------- | ------------------- |
| macOS Apple Silicon | App archive and DMG |
| macOS Intel         | App archive and DMG |
| Windows x64         | NSIS installer      |
| Linux x64           | DEB and AppImage    |

Review all four successful build jobs and assets before publishing the draft
through GitHub. A failed build can leave a partial draft; it is never published
automatically. Reruns can update a draft; published releases are not overwritten.
Use a new version for subsequent corrections.

The workflow uses the repository's `GITHUB_TOKEN` with `contents: write` only in
release jobs. It requires a GitHub repository with Actions enabled. No remote
repository, tag, or Release is created by adding these local files.

## Signing and future app updates

OS signing/notarization credentials are not configured yet. Initial bundles may
trigger OS trust prompts. Configure signing before a public distribution that
requires it. The workflow does not configure an in-app updater or publish
updater metadata; that is a separate feature with its own signature requirements.

Local tests validate extraction and workflow formatting, not remote build success.
The full installer matrix is verified when the tagged workflow runs on GitHub.
