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

Custom executable scanning is limited to selected folders and six levels. The scanner skips executables under 2 MB and anything whose file name or folder (below the folder you chose) looks like an installer, updater, patcher, crash handler, helper, redistributable, browser, document, download, temp, driver, configuration, SDK, or school material, plus well-known programs such as browsers and Office. It also looks at where a file lives. Anything under Program Files, ProgramData, AppData or a hidden folder such as `.cargo` is an installed program, and is offered only when game files sit next to it (Steam, Unity, Unreal, Godot, GameMaker, RPG Maker or Ren'Py files, store or online-service libraries, or game audio and physics libraries; folders of sounds or textures alone are not enough). A file sitting loose in a Downloads folder, or whose name carries a version number such as `Rainmeter-4.5.28`, is treated as an installer and dropped; a file inside a Downloads subfolder with no game files is kept under **Filtered out**. A folder named exactly "Games" (also Steam Library, GOG Games, EA Games and similar) is trusted. The scanner never offers Aether Hub itself, and the review shows how many installed programs and installers it skipped. Games already listed by Steam or Epic are not repeated. Skipped files that are large enough to be real games are kept under **Filtered out** in the review, with the reason, so one with an unlucky name (for example a title containing "crash") can still be ticked and imported. Review every candidate, especially generic executable names, before import. Nothing runs just because it was discovered.

Executable launches support arguments and working directories. Steam/Epic URI launches delegate to their clients; configure game arguments in the storefront client. Riot uses its client executable and product-specific arguments.

Local PNG, JPEG, and WebP images are copied into Aether's data folder. A resized rendering copy avoids unnecessarily decoding large originals. Downloaded Steam/SteamGridDB artwork is cached when available. If artwork is missing, the title-only fallback remains usable.

Data is stored under `%APPDATA%\com.aetherhub.launcher`: `library.json`, `library.backup.json`, `playtime.json`, `covers/`, and `backups/`. Earlier releases used `%APPDATA%\com.aetherhub.desktop`; the first time this version starts it copies your library and the covers it uses into the new folder and leaves the old folder untouched (you may delete it once you are happy). Corrupt libraries are not silently overwritten. Save failures show an error.

## Controller

Connect a PS5 DualSense by USB or Bluetooth (Xbox controllers work too). The D-pad or left stick moves, **Cross** launches the focused game or presses the focused control, **Circle** goes back or closes a dialog, **L1/R1** switch category tabs (or Settings tabs), and **Square** opens a game's details. Big Box has its own category pills for L1/R1. The controller is only read while Aether Hub is the focused window, so it never competes with a running game. A short, soft click plays each time the selection moves (and when the on-screen keyboard's highlighted key changes); switch it off under **Customize UI → Navigation sounds**.

## Playtime, Steam sync, and Discord

Playtime is recorded for games launched from Aether Hub while the launcher stays open (Do Nothing or Minimize). Exported Windows shortcuts hand off without opening the interface, so they are not tracked.

**Steam playtime sync** (Settings, Integrations) reads total hours from your public Steam profile: set "Game details" to Public, then enter your profile link, custom name, or SteamID64. A Steam Web API key is optional and makes syncing steadier. Totals only ever go up, so time played through Aether Hub is not counted twice.

**Discord Rich Presence** needs your own free Discord application: create one at discord.com/developers/applications, name it, and paste its Application ID into Settings. Friends then see the game you launched, with a play timer, for Steam, Epic, Riot, and standalone games. Optional: upload an image called `aether_hub` under Rich Presence, Art Assets. It requires the Discord desktop app.

## Backups

Settings, Backup keeps recent backups of your library, playtime, and cover references in the folder you choose (default: inside the data folder). Pick a folder inside OneDrive, Dropbox, or Google Drive and your library follows you to other PCs; Aether Hub itself has no server. **Export to a file** also embeds your custom cover images, and **Restore** saves your current library as a "before a restore" backup first. Your Steam API key is never written to a backup.

## Shortcuts

Choose **Create shortcuts folder**, then select a destination. Aether creates real Windows `.lnk` files. Existing files are left untouched; collisions get numbered names. Shortcuts use the installed launcher executable with a game ID, perform native launch handoff, and exit without opening the UI. Keep Aether installed and the game entry present for these shortcuts to work.

## Resource settings

**When a game starts** (Settings, General): **Do Nothing** keeps the launcher open; **Minimize Launcher** keeps it available and does not release all RAM; **Close Launcher Completely** exits after a successful handoff and releases all of its processes. Closing does not close the game or storefront client, and it means playtime tracking and Discord status pause, because nothing is left running. The launcher does not automatically reopen when the game exits.

**Performance Mode** disables wallpaper, blur, shadows, and animations, requests smaller Steam artwork, and loads fewer posters at a time. Total memory includes WebView2. Under 50 MB and near-zero idle CPU are targets requiring real Windows measurements, not guaranteed figures. While no controller is connected, the controller code does no polling.

## Privacy

No telemetry, launcher authentication, cloud database, or automatic background scanning. Steam artwork retrieval uses Steam's CDN. Steam playtime sync, if you turn it on, asks Steam for your public profile (steamcommunity.com, or api.steampowered.com if you provide a key). Discord Rich Presence talks only to the Discord app on your PC. Both are off by default. SteamGridDB is optional, uses your key only for the current editor session, and requires confirmation of artwork matches. Keys are not saved. All library and appearance settings stay local.

## Release status

Source and a Windows build pipeline are included. No compiled executable or installer has been verified in this editing environment. Install, upgrade, uninstall, real platform launches, COM shortcuts, and performance measurements must pass the release checklist before a build is called production-ready.
