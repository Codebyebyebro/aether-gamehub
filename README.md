# Aether Hub

An artwork-first Windows game library built with Tauri 2, Rust, React 18, TypeScript, and Tailwind CSS. Cream/indigo is the default, with navy/violet and midnight/cyan themes.

## Included

- Poster, banner, compact-list, and fullscreen "Big Box" carousel views, all usable with mouse, keyboard, or a PS5 DualSense (or Xbox) controller: D-pad/left stick move, Cross selects or launches, Circle goes back, L1/R1 switch category tabs, Square opens details. A soft click plays as the selection moves; Customize UI → Navigation sounds turns it off.
- A PS5-style on-screen keyboard for the controller: press Cross on any text field (search, game editor, settings) and a keyboard docks at the bottom. D-pad moves, Cross types the highlighted key, Square deletes, Triangle adds a space, L1/R1 move the cursor, L2 is Shift (twice for caps lock), R2 is Done, and Circle closes it. A symbols page, a live preview line, and masked password fields are included; a physical keyboard and the mouse still work in the same field. The layout and button roles follow the console's keyboard, with its own artwork.
- Search, platform filters, favorites, manual game CRUD, live appearance settings, wallpaper, and Performance Mode.
- Rust-based Steam/Epic/Riot/custom scanners with strict filters (executables under 2 MB, installer/updater/crash-handler/helper/redistributable/browser/document/school keywords, and well-known non-game programs are skipped; programs under Program Files, AppData and similar are offered only when game files sit next to them, and installers loose in Downloads are dropped) and a review that shows each file's size and clean path, lists what was filtered out and why, and prevents duplicates. While a scan runs it shows which source or folder it is on, how many items it has checked and how many games it has found, and it stops when the player presses Cancel or closes the review.
- Playtime tracking for games launched from Aether Hub, optional Steam playtime sync from a public profile, and optional Discord Rich Presence.
- Automatic and manual backups of the library, playtime, and cover references to a folder you choose (for example inside OneDrive), with one-click restore.
- "When a game starts" can do nothing, minimize the launcher, or close it completely.
- Optional, all off by default: keep running in the system tray, a Ctrl+Alt+Space shortcut to bring the launcher back, start with Windows (quietly in the tray), and open Big Box when a controller connects. An optional "a newer version is available" check against this project's GitHub releases; nothing is downloaded or installed automatically.
- Big Box top bar (Up from the games): search with the on-screen keyboard, scan, settings and exit, so nothing needs the mouse; Big Box steps aside for Scan and Settings and returns. F11 toggles full screen anywhere; leaving Big Box keeps full screen if it was on before or "Stay full screen after Big Box" is set.
- Console controls: Options opens a power menu (exit to desktop, quit, sleep/restart/shut down with a Cancel-first confirm; `shutdown.exe` and `SetSuspendState`, nothing else can be invoked); a launch sound and Gamepad dual-rumble when launching with the controller; and an opt-in PS / Xbox button listener (Raw Input with `RIDEV_INPUTSINK` for DualSense/DualShock 4 HID reports, `XInputGetStateEx` for the Xbox guide bit; only that bit is read) that opens a quick menu over a running game (resume, home, close game — ending only the tracked game's processes — and power) or Big Box otherwise. Bringing the window forward from the background uses `AttachThreadInput` so Windows hands over focus.
- Console mode (with a controller connected): starts straight in full-screen Big Box with a splash and chime; Big Box is a console home screen (full-screen art of the selected game, a tile row with recently played first, clock, Square for a details page, Triangle to favorite); and when a game ends, the launcher comes back in Big Box. Controllers are detected natively (Windows.Gaming.Input, count only, no buttons read). On by default; Settings → Background.
- Living colors: the interface takes on the main colour of the game in focus (pointer, controller, details window, Big Box selection, spotlight), picked in Rust from the cached artwork. Solid colours with easing, no gradients; text keeps its usual surfaces. Toggle in Customize UI; off in Performance mode.
- Jump back in: the spotlight follows play (now playing, then the most recently played three), with play time and last-played date; a Recently played row; Recently played and Most played sorts.
- "Find missing artwork": games with no cover (Epic, Riot, folder scans) are looked up by name in Steam's public store, with no account or key, and optionally on SteamGridDB with a key used for that run only. A cover is set only on an exact, unambiguous name match whose picture actually downloads; everything else is left alone and listed with the reason. Only game titles are sent. Offered after a scan that adds games without artwork.
- Native URI/process launches, managed artwork copies and cached remote artwork.
- Genuine Windows COM `.lnk` export and a pre-WebView headless shortcut entrypoint.
- Local transactional JSON file replacement and backup; no hosted server or cloud database.
- Windows CI builds an application executable and an offline WebView2 NSIS installer.

The browser preview is an isolated, labeled artwork showcase. Native operations do not report fake success. The installed app begins with an empty library.

## End-user installation

See [Installation](docs/INSTALLATION.md). Maintainers: [Releasing](docs/RELEASING.md) and [Code signing](docs/CODE-SIGNING.md). Players use a setup executable and normal Windows shortcuts; they do not install Node.js or open a terminal.

## Development and release

Frontend source is under `src/`; Rust source is under `src/native/`. Cargo/Tauri configuration is under `src-tauri/`. The package scripts define browser development, type checking, icon generation, Tauri development/build, and tests.

On a Windows development machine, `npm run tauri build` produces `src-tauri/target/release/Aether Hub.exe` and the installer under `src-tauri/target/release/bundle/nsis/`. Icons are generated automatically on first use. `npm run test:logic` runs the controller, navigation, and formatting tests; `npm run native:test` runs the Rust tests. A Rust command that is missing from `src-tauri/permissions/library.toml` is rejected at runtime by release builds, so a test fails the build if the two lists differ. Developer build infrastructure needs Node 24, Rust 1.88+, and the Windows MSVC build toolchain; these are not user prerequisites.

The **Aether Hub Windows build** GitHub Actions workflow can be started from the repository's Actions page and also runs on version tags. It installs build tools, checks TypeScript, generates native icons, runs Rust tests, builds the executable/offline installer, and uploads unsigned Windows x64 artifacts and SHA256 checksums. Native icon files are generated from the original bundled SVG before native builds.

On a GitHub-hosted Windows runner the project type-checks against the real React and Radix types, passes its logic and Rust test suites, builds the production frontend and the offline installer, installs silently, and the installed app starts and stays up for 20 seconds; "Not yet verified here" in the release checklist has the details and what that does not cover. A physical controller, real Discord and Steam connections, the interface inside real dialogs on a screen, code signing, and performance on a low-end PC have not been tested. Follow the [Release checklist](docs/RELEASE-CHECKLIST.md) before public distribution. Configure publisher signing and retain a Cargo.lock for reproducible release builds. The loose app executable still requires WebView2; the offline setup provisions it.

This release uses bounded incremental grid rendering, not full virtualization; a cancelled scan stops at its next file or folder, so a single slow folder read (a network drive, a disk waking up) can delay the stop briefly. Under-50 MB total process memory is a measured goal, not a guarantee: WebView2 alone needs memory of its own. "Close Launcher Completely" is the only setting that frees everything.
