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
- Keep/minimize/close behaviors: failed handoffs retain the app; close mode leaves no Aether-owned WebView2 processes while the game continues.
- All four layouts, custom accent contrast, themes, wallpaper blur/dim, reduced motion, narrow windows, keyboard carousel navigation, and Escape exit.
- Uninstall/upgrade behavior and retained data documented. Exported shortcuts are intentionally not managed by the uninstaller.
- Signing and publisher validation configured before public distribution. Current CI artifacts are explicitly unsigned. Verify installer and executable signatures/checksums in the final published package.

## Performance evidence

Measure release builds on agreed low-end Windows hardware. Record the PC, OS, WebView2 version, library size, layout, and settings.

- Empty, 100-game, 1,000-game, and large-library startup and steady idle.
- Entire process-tree working set/private bytes, with Rust and WebView2 also reported separately.
- CPU idle over a sustained period, not a single instantaneous reading.
- Normal vs Performance Mode; visible vs minimized vs fully closed.
- Scan and artwork decoding peaks and main-thread responsiveness.

The UI initially mounts 48 cards and reveals more on explicit request; the fullscreen carousel mounts only its nearby artwork. This is bounded incremental rendering, not full virtualization. Clicking Show more increases mounted artwork. The remote disk cache has a 256 MB budget; manually imported originals are retained and need a future explicit storage-management feature if cleanup is desired.

## Not yet verified here

No native build, Windows execution, signing, installer test, or RAM/CPU benchmark was run in the editing environment. There are native parser/target/filename tests in source, but their results must be obtained from the Windows workflow. Scan review can be closed while a scan completes; this release does not interrupt filesystem scans mid-operation or provide per-file progress. Controller navigation is deliberately out of scope for this release.
