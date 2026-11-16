# Aether Hub

An artwork-first Windows game library built with Tauri 2, Rust, React 18, TypeScript, and Tailwind CSS. Cream/indigo is the default, with navy/violet and midnight/cyan themes.

## Included

- Poster, banner, compact-list, and keyboard/mouse fullscreen carousel views.
- Search, platform filters, favorites, manual game CRUD, live appearance settings, wallpaper, and Performance Mode.
- Rust-based Steam/Epic/Riot/custom scanners with review and duplicate prevention.
- Native URI/process launches, managed artwork copies and cached remote artwork.
- Genuine Windows COM `.lnk` export and a pre-WebView headless shortcut entrypoint.
- Local transactional JSON file replacement and backup; no hosted server or cloud database.
- Windows CI builds an application executable and an offline WebView2 NSIS installer.

The browser preview is an isolated, labeled artwork showcase. Native operations do not report fake success. The installed app begins with an empty library.

## End-user installation

See [Installation](docs/INSTALLATION.md). Players use a setup executable and normal Windows shortcuts; they do not install Node.js or open a terminal.

## Development and release

Frontend source is under `src/`; Rust source is under `src/native/`. Cargo/Tauri configuration is under `src-tauri/`. The package scripts define browser development, type checking, icon generation, Tauri development/build, and native tests. Developer build infrastructure needs Node 24, Rust 1.88+, and the Windows MSVC build toolchain; these are not user prerequisites.

The **Aether Hub Windows build** GitHub Actions workflow can be started from the repository's Actions page and also runs on version tags. It installs build tools, checks TypeScript, generates native icons, runs Rust tests, builds the executable/offline installer, and uploads unsigned Windows x64 artifacts and SHA256 checksums. Native icon files are generated from the original bundled SVG before native builds.

No native build or performance benchmark has been executed in this editing environment. Follow the [Release checklist](docs/RELEASE-CHECKLIST.md) before public distribution. Configure publisher signing and retain a Cargo.lock for reproducible release builds. The loose app executable still requires WebView2; the offline setup provisions it.

This release uses bounded incremental grid rendering, not full virtualization; scan review can close while scanning completes, but scan interruption is not yet implemented. Under-50 MB total process memory is a measured goal, not a guarantee.
