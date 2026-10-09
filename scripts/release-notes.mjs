/** Validate release identity and extract its notes before any upload occurs. */
import { readFileSync, appendFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { randomUUID } from "node:crypto";

/**
 * Read one stable release section and require all app version files to agree.
 * Throws on missing/duplicate/empty notes or mismatched versions. No files change.
 */
export function readReleaseMetadata(projectRoot, tag) {
  const match = /^v(\d+\.\d+\.\d+)$/.exec(tag ?? "");
  if (!match)
    throw new Error("Release tags must use vX.Y.Z (stable versions only).");
  const version = match[1];
  const read = (file) => readFileSync(resolve(projectRoot, file), "utf8");
  const cargo = read("src-tauri/Cargo.toml");
  // Only inspect [package]; dependency versions are unrelated to the app version.
  const packageSection = cargo
    .split(/\r?\n(?=\[)/)
    .find((section) => section.trimStart().startsWith("[package]"));
  const cargoVersion = /^version\s*=\s*"([^"]+)"\s*$/m.exec(
    packageSection ?? "",
  )?.[1];
  const lock = JSON.parse(read("package-lock.json"));
  const versions = {
    "package.json": JSON.parse(read("package.json")).version,
    "package-lock.json": lock.version,
    "package-lock.json root package": lock.packages?.[""]?.version,
    "src-tauri/Cargo.toml": cargoVersion,
    "src-tauri/tauri.conf.json": JSON.parse(read("src-tauri/tauri.conf.json"))
      .version,
  };
  for (const [file, value] of Object.entries(versions)) {
    if (value !== version)
      throw new Error(`${file} version ${value} does not match ${tag}.`);
  }

  const lines = read("CHANGELOG.md").split(/\r?\n/);
  const heading = `## [${version}] - `;
  const matches = lines.flatMap((line, index) =>
    line.startsWith(`## [${version}]`) ? [index] : [],
  );
  if (matches.length !== 1)
    throw new Error(
      `Expected one CHANGELOG section for ${version}; found ${matches.length}.`,
    );
  const start = matches[0];
  if (
    !lines[start].startsWith(heading) ||
    !/^\d{4}-\d{2}-\d{2}$/.test(lines[start].slice(heading.length))
  ) {
    throw new Error(`CHANGELOG heading must use ${heading}YYYY-MM-DD.`);
  }
  const next = lines.findIndex(
    (line, index) => index > start && line.startsWith("## "),
  );
  const notes = lines
    .slice(start + 1, next === -1 ? undefined : next)
    .join("\n")
    .trim();
  // Subheadings alone are not useful release notes.
  if (!notes.split("\n").some((line) => line.trim() && !line.startsWith("#"))) {
    throw new Error(`CHANGELOG notes for ${version} are empty.`);
  }
  return { tag, version, notes };
}

/** Emit multiline Actions outputs without interpreting changelog text as code. */
function writeActionsOutputs(file, metadata) {
  for (const [key, value] of Object.entries(metadata)) {
    // A random marker avoids collisions with Markdown content in the release body.
    const marker = `VIBEMATE_${randomUUID()}`;
    appendFileSync(file, `${key}<<${marker}\n${value}\n${marker}\n`);
  }
}

// Importing this module in tests must not execute the CLI or write Actions outputs.
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  try {
    const metadata = readReleaseMetadata(
      process.cwd(),
      process.argv[2] ?? process.env.GITHUB_REF_NAME,
    );
    if (process.env.GITHUB_OUTPUT)
      writeActionsOutputs(process.env.GITHUB_OUTPUT, metadata);
    else console.log(metadata.notes);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
