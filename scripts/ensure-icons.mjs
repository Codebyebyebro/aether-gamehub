// Generates src-tauri/icons from public/aether-mark.svg when they are missing, so a fresh checkout builds with a
// single `npm run tauri build`. npm runs this automatically before every `npm run tauri ...` (the "pretauri" hook).
import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
if (existsSync(join(root, "src-tauri", "icons", "icon.ico"))) process.exit(0);

console.log("Generating the app icons from public/aether-mark.svg ...");
const windows = process.platform === "win32";
const cli = join(root, "node_modules", ".bin", windows ? "tauri.cmd" : "tauri");
const result = spawnSync(cli, ["icon", "public/aether-mark.svg"], { cwd: root, stdio: "inherit", shell: windows });
if (result.status !== 0) {
  console.error("Could not generate the icons. Run `npm install` first, then try again.");
  process.exit(result.status ?? 1);
}
