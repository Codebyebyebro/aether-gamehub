# Native release gate

Do not mark a build production-ready solely because the web preview renders.

## Required Windows checks

- Successful frontend type check, production build, Rust tests, and Tauri x64 build in the supplied Windows workflow. Retain the generated Cargo.lock and use locked native dependencies for a published release.
- Offline install on clean Windows 10 and 11 x64, with and without WebView2; first-run settings and Start menu/desktop launches without developer tools.
- Real Steam and Epic launches; VALORANT and League of Legends discovery/launch on representative client installations; custom executable paths with spaces and Unicode, raw argument quoting, working directories, and missing-path errors.
- Review and deduplication on repeated scans, including Riot products sharing the same client executable but using different arguments. Malformed manifests, access-denied paths, duplicate custom roots, helper exclusions, and six-level scan behavior.
- Local artwork save/restart after moving originals; cached artwork restart while offline; API failure and invalid-key cases. Covers folder access remains scoped.
- Add, edit, remove, favorites, save failure rollback, backup creation, and corrupted-library recovery messaging.
- `.lnk` creation, Explorer properties, launch with Hub open/closed, path names and collisions, partial export failure, removed entries, and no interface/WebView startup in headless mode.
- Keep/minimize/close behaviors: failed handoffs retain the app; close mode leaves no Aether-owned WebView2 processes while the game continues. Confirm that playtime and Discord status work in Do Nothing and Minimize, and pause (as documented) in Close.
- `npm run tauri build` on a clean Windows checkout (no icons present): it must produce `src-tauri/target/release/Aether Hub.exe` and an installer under `bundle/nsis`, with no extra steps.
- Upgrade from the previous release: the library, covers, and settings appear under `%APPDATA%\com.aetherhub.launcher`, the old folder is untouched, and the NSIS installer upgrades in place rather than installing side by side.
- Controller: DualSense over USB and over Bluetooth, with and without Steam running (Steam Input may present it as an Xbox pad); D-pad/stick repeat, Cross/Circle/L1/R1/Square, Big Box, dialogs, dropdowns, and Settings tabs. Confirm nothing reacts while a game has focus.
- On-screen keyboard with a physical DualSense: Cross on the search box, a game-editor field (title, target, arguments), the Steam profile and Steam API key fields, the Discord Application ID, the accent colour box, and the scan "Preview folder path" box each opens it; type with Cross, Square deletes (hold to repeat), Triangle spaces, L1/R1 move the cursor, L2 toggles Shift (a second press locks caps, a third clears it), R2 closes with Done, Circle closes without leaving the dialog underneath. Check that L2 and R2 register as triggers over USB, over Bluetooth, and with Steam Input running (it may remap them or report them as buttons), that a quick double letter ("ll") is typed twice, and that the Steam API key (and the SteamGridDB key in the editor) show dots in the keyboard's preview line and are never written to a log. Also check it over a real Radix dialog (focus trap, outside-click, Escape) and with the Windows on-screen keyboard disabled.
- Scanner on a real drive containing school/CCNA material, installers, browsers, and documents: none appear; the Filtered out list explains each large rejection; sizes and paths are readable; Steam/Epic games are not duplicated by a custom scan of their folders.
- Scan progress and Cancel: on a large drive, the bar moves one step per source and custom folder and the item count rises; "Cancel scan" (mouse and Cross) closes the review within a second or two and adds nothing; closing the review with Escape, Circle or × also stops the scan; starting a new scan straight after a cancel works instead of reporting that one is already running. Repeat on a slow or network drive and note how long the stop takes.
- Discord: with the Discord desktop app running, a launched game shows with a timer and clears on exit; Test connection reports a wrong Application ID and a closed Discord clearly.
- Steam sync against a public profile with and without an API key, a private profile (clear message), a bad key, and offline.
- Backups: automatic backup after a change, rotation, export with covers, restore on a second PC, restore of a file from a newer version refused, and the "before a restore" backup.
- All four layouts, custom accent contrast, themes, wallpaper blur/dim, reduced motion, narrow windows, keyboard carousel navigation, and Escape exit.
- Uninstall/upgrade behavior and retained data documented. Exported shortcuts are intentionally not managed by the uninstaller.
- Signing and publisher validation configured before public distribution. Current CI artifacts are explicitly unsigned. Verify installer and executable signatures/checksums in the final published package.

## Windows build workflow

`.github/workflows/windows-release.yml` has two jobs. **check** runs on every push to main or master and on every pull request: `pnpm install --frozen-lockfile`, `typecheck`, `test:logic`, `vite build`, then `cargo test --locked`, which includes the permission-list test. **package** runs from the Actions tab ("Run workflow") and on `v*` tags: it builds the executable and the NSIS installer, writes SHA-256 sums, then runs `scripts/smoke-windows.ps1` on the loose executable and on a silently installed copy. That script starts the app, checks that it is still running after 20 seconds, and records one rough memory reading in the run summary. It is a start-up check, not a benchmark, and it does not exercise any feature or the controller.

The two start-up steps are new and have not run on a runner yet. They report failures in the run summary without failing the run, so open the summary page after every run. The artifact is unsigned. Until check and package have both run green once, everything under "Not yet verified here" still stands. The same script can be run on a PC against an installed copy: `powershell -File scripts\smoke-windows.ps1 -Exe "<path to Aether Hub.exe>"`.

## Performance evidence

Measure release builds on agreed low-end Windows hardware. Record the PC, OS, WebView2 version, library size, layout, and settings.

- Empty, 100-game, 1,000-game, and large-library startup and steady idle.
- Entire process-tree working set/private bytes, with Rust and WebView2 also reported separately.
- CPU idle over a sustained period, not a single instantaneous reading.
- Normal vs Performance Mode; visible vs minimized vs fully closed.
- Scan and artwork decoding peaks and main-thread responsiveness.

The UI initially mounts 48 cards and reveals more on explicit request; the fullscreen carousel mounts only its nearby artwork. This is bounded incremental rendering, not full virtualization. Clicking Show more increases mounted artwork. The remote disk cache has a 256 MB budget; manually imported originals are retained and need a future explicit storage-management feature if cleanup is desired.

## Not yet verified here

No native build, Windows execution, signing, installer test, or RAM/CPU benchmark was run in the editing environment. There are native parser/target/filename tests in source, but their results must be obtained from the Windows workflow. Not yet verified by anyone: a Windows build and installer, a physical controller, live Discord, and live Steam endpoints. Nothing that needs Windows, Discord, Steam's servers, or a physical controller has been exercised: the checks above must be run by a person on Windows.

The scan progress and Cancel change was written in a workspace without network access, so `cargo check`, `cargo test` on the full Tauri crate, `vite build`, and `tsc` against the real React and Radix types were not run for it. What was run instead: the new `scan_control.rs` tests and all scanner and util tests, compiled against small stand-in crates for walkdir, serde, serde_json, url, and uuid; the permission-list test on the real files; the new `scan_games`/`cancel_scan` commands type-checked (including that their futures are `Send`) against stand-ins shaped like the Tauri and tokio APIs; `npm run test:logic`; a partial TypeScript check; and a Chromium run of the real scan review with a simulated backend and DualSense, where the Radix, lucide, and sonner packages were stand-ins. Run `npm run typecheck`, `npm run test:logic`, `npm run build`, and `npm run native:test` on Windows before relying on it.

The on-screen keyboard was written in the same kind of workspace, so the same tools were not run for it either. What was run instead: the pure keyboard logic and the per-layer pad rules in `npm run test:logic`-style unit tests (36 passing), checked by breaking each rule on purpose and confirming a test fails; a partial TypeScript check of the new and changed files against stand-in React types (no new errors in the default settings; in strict mode the two inline `onPointerDown`/`onMouseDown` handlers are reported as untyped only because the stand-in types intrinsic elements as `any`, and the existing editor and settings components get the same report); and a Chromium run of the real `Index.tsx` with a simulated backend and a simulated DualSense, typing into the search box, a controlled editor field, a blur-committed accent field and a masked password field inside a dialog-like overlay. Radix, lucide, and sonner were stand-ins there, so the interaction with a real Radix dialog, and the analog L2/R2 values from a physical pad, have not been seen.
