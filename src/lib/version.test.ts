import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

/** Every place the version number lives must agree, or an install thinks it is out of date or the release is named wrongly. */
const read = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
const conf: string = JSON.parse(read("src-tauri/tauri.conf.json")).version;
const cargo = /^version\s*=\s*"([^"]+)"/m.exec(read("src-tauri/Cargo.toml"))?.[1];
const lock = /name = "aether-hub"\r?\nversion = "([^"]+)"/.exec(read("src-tauri/Cargo.lock"))?.[1];
const hint = "To release, change the version in src-tauri/tauri.conf.json, src-tauri/Cargo.toml, the aether-hub entry of src-tauri/Cargo.lock and the two v-numbers in the sidebar of src/pages/Index.tsx, and add a section to docs/CHANGELOG.md.";

test("the version looks like 1.2.3 and the installer, the program and the lock file agree", () => {
  assert.match(conf, /^\d+\.\d+\.\d+$/, hint);
  assert.equal(cargo, conf, `Cargo.toml says ${cargo}, tauri.conf.json says ${conf}. ${hint}`);
  assert.equal(lock, conf, `Cargo.lock says ${lock}, tauri.conf.json says ${conf}. A locked build would fail. ${hint}`);
});
test("the version shown in the sidebar is the real one", () => {
  const shown = [...read("src/pages/Index.tsx").matchAll(/v\d+\.\d+\.\d+/g)].map(m => m[0]);
  assert.ok(shown.length >= 1, "the sidebar shows a version");
  assert.deepEqual([...new Set(shown)], [`v${conf}`], hint);
});
test("the change log has a written section for this version, because the release notes are taken from it", () => {
  const log = read("docs/CHANGELOG.md").replace(/\r\n/g, "\n");
  const section = new RegExp(`^## ${conf.replace(/\./g, "\\.")}\\n([\\s\\S]*?)(?=^## |$(?![\\s\\S]))`, "m").exec(log)?.[1] ?? "";
  assert.ok(section.trim().length > 20, `docs/CHANGELOG.md needs a "## ${conf}" section with something in it. ${hint}`);
});
