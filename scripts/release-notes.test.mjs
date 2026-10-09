import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { stringify } from "yaml";
import { readReleaseMetadata } from "./release-notes.mjs";

/** Isolate release files from the real checkout and remove them after each test. */
function fixture(t, changelog) {
  const root = mkdtempSync(join(tmpdir(), "vibemate-release-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, "src-tauri"));
  const put = (name, value) => writeFileSync(join(root, name), value);
  put(
    "package.json",
    JSON.stringify({ version: "0.1.0", dependencies: { react: "^19.1.0" } }),
  );
  // pnpm 12 stores tool dependencies separately from the application importer.
  put(
    "pnpm-lock.yaml",
    stringify({
      lockfileVersion: "9.0",
      importers: {
        ".": {
          packageManagerDependencies: {
            pnpm: { specifier: "12.10.1", version: "12.10.1" },
          },
        },
      },
    }) +
      "---\n" +
      stringify({
        lockfileVersion: "9.0",
        importers: {
          ".": {
            dependencies: {
              react: { specifier: "^19.1.0", version: "19.3.0" },
            },
          },
        },
      }),
  );
  put("src-tauri/tauri.conf.json", JSON.stringify({ version: "0.1.0" }));
  put(
    "src-tauri/Cargo.toml",
    '[package]\nname = "vibemate"\nversion = "0.1.0"\n\n[dependencies]\ntauri = "2"\n',
  );
  put("CHANGELOG.md", changelog);
  return { root, put };
}

const release = "## [0.1.0] - 2026-10-09\n\n### Added\n\n- First release.\n";

test("extracts only the tagged version, preserving Markdown and excluding Unreleased", (t) => {
  const { root } = fixture(
    t,
    `# Changelog\n\n## [Unreleased]\n\n- Future work.\n\n${release}\n## [0.0.9] - 2026-10-01\n\n- Old work.\n`,
  );
  assert.deepEqual(readReleaseMetadata(root, "v0.1.0"), {
    tag: "v0.1.0",
    version: "0.1.0",
    notes: "### Added\n\n- First release.",
  });
});

test("rejects mismatched app versions before creating release notes", (t) => {
  const { root, put } = fixture(t, release);
  put("src-tauri/tauri.conf.json", JSON.stringify({ version: "0.2.0" }));
  assert.throws(() => readReleaseMetadata(root, "v0.1.0"), /does not match/);
});

test("rejects a stale pnpm dependency declaration", (t) => {
  const { root, put } = fixture(t, release);
  put(
    "package.json",
    JSON.stringify({ version: "0.1.0", dependencies: { react: "^20.0.0" } }),
  );
  assert.throws(() => readReleaseMetadata(root, "v0.1.0"), /pnpm-lock.*react/);
});

test("rejects removed or unresolved locked dependencies", (t) => {
  const { root, put } = fixture(t, release);
  put("package.json", JSON.stringify({ version: "0.1.0" }));
  assert.throws(() => readReleaseMetadata(root, "v0.1.0"), /pnpm-lock.*react/);
  put(
    "package.json",
    JSON.stringify({ version: "0.1.0", dependencies: { react: "^19.1.0" } }),
  );
  put(
    "pnpm-lock.yaml",
    stringify({
      importers: {
        ".": {
          dependencies: {
            react: { specifier: "^19.1.0" },
          },
        },
      },
    }),
  );
  assert.throws(() => readReleaseMetadata(root, "v0.1.0"), /pnpm-lock.*react/);
});

test("rejects missing, ambiguous, and malformed application lockfiles", (t) => {
  const { root, put } = fixture(t, release);
  rmSync(join(root, "pnpm-lock.yaml"));
  assert.throws(() => readReleaseMetadata(root, "v0.1.0"), /pnpm-lock.yaml/);
  put("pnpm-lock.yaml", "importers: [\n");
  assert.throws(() => readReleaseMetadata(root, "v0.1.0"), /invalid YAML/);
  const tools = stringify({
    importers: { ".": { packageManagerDependencies: {} } },
  });
  put("pnpm-lock.yaml", tools);
  assert.throws(
    () => readReleaseMetadata(root, "v0.1.0"),
    /application root importer/,
  );
  put("pnpm-lock.yaml", "importers: {'.': {}}\n---\nimporters: {'.': {}}\n");
  assert.throws(
    () => readReleaseMetadata(root, "v0.1.0"),
    /application root importer/,
  );
});

test("rejects missing and duplicate release sections", (t) => {
  const { root, put } = fixture(t, "## [Unreleased]\n\n- Upcoming.\n");
  assert.throws(() => readReleaseMetadata(root, "v0.1.0"), /found 0/);
  put("CHANGELOG.md", `${release}\n${release}`);
  assert.throws(() => readReleaseMetadata(root, "v0.1.0"), /found 2/);
});

test("rejects empty notes and incorrectly formatted headings", (t) => {
  const { root, put } = fixture(t, "## [0.1.0] - 2026-10-09\n\n### Added\n");
  assert.throws(() => readReleaseMetadata(root, "v0.1.0"), /empty/);
  put("CHANGELOG.md", "## [0.1.0]\n\n- Release.\n");
  assert.throws(() => readReleaseMetadata(root, "v0.1.0"), /heading must use/);
});

test("rejects prerelease and malformed tags", (t) => {
  const { root } = fixture(t, release);
  for (const tag of [
    "0.1.0",
    "v0.1.0-beta.1",
    "v0.1",
    "v0.1.0; echo invalid",
  ]) {
    assert.throws(() => readReleaseMetadata(root, tag), /Release tags/);
  }
});
