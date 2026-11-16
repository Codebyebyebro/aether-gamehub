# Aether Hub for Windows

## Install without a terminal

1. Download the x64 **setup executable** from a verified Aether Hub release.
2. Open the setup file and follow its installation screens.
3. Open **Aether Hub** from the Start menu or its desktop shortcut.
4. Choose your launch preferences, then scan for installed games or add a game manually.

Supported target: Windows 10/11 x64. The offline installer bundles WebView2 provisioning. No Node.js, Rust, terminal, or Aether account is needed on the player's PC. Windows may request permission to install WebView2 if the runtime is missing. Offline installation does not make storefront clients or DRM work offline.

The application executable is also included in build artifacts, but a loose `.exe` still requires WebView2 already installed. Use setup for the cleanest experience. Current build artifacts are unsigned development builds; Windows may show SmartScreen warnings. Do not bypass warnings for files whose source you do not trust.

## Library and artwork

Aether starts with an empty native library. Browser showcase games are not your installed library. Steam, Epic, and Riot discovery reads local installation files without collecting credentials. The platform clients may still require their usual login.

Custom executable scanning is limited to selected folders and six levels. Review every candidate, especially generic executable names, before import. Nothing runs just because it was discovered.

Executable launches support arguments and working directories. Steam/Epic URI launches delegate to their clients; configure game arguments in the storefront client. Riot uses its client executable and product-specific arguments.

Local PNG, JPEG, and WebP images are copied into Aether's data folder. A resized rendering copy avoids unnecessarily decoding large originals. Downloaded Steam/SteamGridDB artwork is cached when available. If artwork is missing, the title-only fallback remains usable.

Data is stored under `%APPDATA%\com.aetherhub.desktop`: `library.json`, `library.backup.json`, and `covers/`. Corrupt libraries are not silently overwritten. Save failures show an error. Back up this directory before manual recovery or moving to another PC.

## Shortcuts

Choose **Create shortcuts folder**, then select a destination. Aether creates real Windows `.lnk` files. Existing files are left untouched; collisions get numbered names. Shortcuts use the installed launcher executable with a game ID, perform native launch handoff, and exit without opening the UI. Keep Aether installed and the game entry present for these shortcuts to work.

## Resource settings

**Minimize** keeps the launcher available and does not release all RAM. **Close Hub** exits after a successful handoff and releases its processes; it does not close the game or storefront client. The launcher does not automatically reopen when the game exits.

**Performance Mode** disables wallpaper, blur, and transitions and requests smaller Steam artwork. Total memory includes WebView2. Under 50 MB and near-zero idle CPU are targets requiring real Windows measurements, not guaranteed figures.

## Privacy

No telemetry, launcher authentication, cloud database, or automatic background scanning. Steam artwork retrieval uses Steam's CDN. SteamGridDB is optional, uses your key only for the current editor session, and requires confirmation of artwork matches. Keys are not saved. All library and appearance settings stay local.

## Release status

Source and a Windows build pipeline are included. No compiled executable or installer has been verified in this editing environment. Install, upgrade, uninstall, real platform launches, COM shortcuts, and performance measurements must pass the release checklist before a build is called production-ready.
