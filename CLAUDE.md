# Aether Hub: notes for Claude

Read this first, then `AI_RULES.md` (project conventions, which still apply) and `README.md` (feature list).

## Working with the owner

- The owner is **not technical**. Explain every step in plain words, and give exact copy-paste PowerShell
  commands, one per code block, with what each does and what they should see. Say what to click and where.
- Don't ask permission for simple, routine steps such as merging your own finished PR into `main`. Just do them
  and say so. Still ask before anything public or hard to undo that they haven't asked for.
- Their PC runs **Windows**, with a **PS5 DualSense** controller. They run the **installed app** (from GitHub
  releases), or `pnpm run desktop:dev` from `%USERPROFILE%\aether-gamehub` when testing unreleased changes, after a
  `git pull` on `main`.
- Their Node is 26 and pnpm is 10. A Linux Mint version is wanted later; Linux support is currently minimal (see
  "Platform" below).

## How the work flows

1. Make changes on the session's branch, keeping it up to date with `origin/main`.
2. Run the checks (below). Screenshot UI changes, using a browser stand-in for the desktop side (see "Testing the
   UI without Windows").
3. Commit, push, and open a PR to `main` with a clear description. GitHub's Windows check (`check` job of
   `.github/workflows/windows-release.yml`) runs on the PR. It is the only real Windows compile and test; wait for it,
   then merge.
4. Tell the owner, in plain words, what changed, what to try, and the exact steps to get it (`git pull`, then
   `pnpm run desktop:dev`, or install a release).
5. Releases follow `docs/RELEASING.md`:
   - Bump the version in all five places (`version.test.ts` enforces it), and turn the change log's
     "Not released yet" section into `## x.y.z`.
   - Run the workflow on `main` with `draft_release: true`. The owner tries it, then publishes.
   - Released so far: 0.1.0, 0.2.0, 0.3.0.

## Checks (run all before every PR)

```
pnpm install --frozen-lockfile
pnpm run test:logic          # node --test, src/**/*.test.ts (pure logic only: no DOM)
pnpm run typecheck
pnpm exec eslint <changed files>   # whole-project lint has old issues in src/components/ui (don't edit those)
pnpm run build
pnpm run desktop:icons && cargo test --manifest-path src-tauri/Cargo.toml
```

On Linux, Rust needs `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`.

The full app can't be cross-compiled for Windows from Linux, because `ring` needs MSVC. To check new Windows-only
Rust, copy the self-contained module into a scratch crate that depends on the same `windows = "0.58"` feature list as
`src-tauri/Cargo.toml`, and run `cargo check --target x86_64-pc-windows-msvc`. Keep Windows-only modules
self-contained so this works.

## Architecture

- **Frontend** (`src/`): React 18, TypeScript, Tailwind.
  - `src/pages/Index.tsx` is the whole app shell: library, dialogs, Big Box mount, menus.
  - `src/lib/aether.ts` is the typed adapter to the native commands. The browser preview returns labelled
    "desktop only" errors and never fakes success.
  - Pure, tested logic lives in `src/lib/*.ts`, each with a `*.test.ts` beside it.
- **Native** (`src/native/*.rs`, Tauri 2; Cargo config in `src-tauri/`):
  - Every command must be listed in both `generate_handler!` in `main.rs` and
    `src-tauri/permissions/library.toml`. A test enforces this.
  - Settings live in `models.rs` (`#[serde(default)]`, camelCase). Add the field there, in `Settings` in
    `aether.ts`, and in `defaults`, with a migration test (an old JSON without the field still loads).
- **Controller:**
  - `src/hooks/padCore.ts` turns Gamepad frames into actions: up/down/left/right, confirm (✕), back (○),
    details (□), secondary (△), prevTab/nextTab (L1/R1), triggers, `menu` (Options), `home` (PS).
  - `useController` is a layer stack: only the top layer gets input. `setPadFallback` handles Options and PS
    app-wide. Each handled action plays its own sound from `cue.ts` (`playCue`). Screens play the `move` click
    themselves, and only when the selection really moves.
  - The browser only sees the pad while the window is focused. Background input is native:
    - `padwatch.rs`: controller connect events and `connected()`, via Windows.Gaming.Input.
    - `homebutton.rs`: the PS button via Raw Input, and the Xbox guide button via XInputGetStateEx. Opt-in.
    - `foreground.rs`: `AttachThreadInput`, so the window can come to the front over a game.
- **Big Box** is `ConsoleHome.tsx`, the console home screen. It has three zones:
  - header (Search, Scan, Settings, Exit),
  - the tile row (recently played first),
  - actions (Play, Details, Favorite), plus a details page.
  Layout rules are in `src/lib/consoleNav.ts`.
- **Console menus** use `ConsoleMenu.tsx` (Cancel-first confirm steps):
  - Power menu: `power.rs` (sleep, restart, shut down only).
  - Quick menu over a running game: `QuickMenu.tsx`. Close game uses `session::close`, which ends only the
    processes the session follows.
- **Sessions and playtime:**
  - `session.rs` follows a launched game (pid, install folder, or process names) and records playtime.
  - When a game ends, `background::after_session` brings Big Box back if Console mode is on and a controller is
    connected.
- **Artwork:**
  - `artwork.rs` keeps an approved-CDN allow-list and a local cache.
  - `art_finder.rs` handles "Find missing artwork": exact-name Steam store match, optional SteamGridDB.
  - `palette.rs` picks the Living colors tint.
- **Styling:** colours come from CSS variables in `src/globals.css`, and there are no gradients (AI_RULES).
  Performance mode (`.performance`) disables animation, blur and shadows. Living colors uses `--game-tint` and
  `html[data-tint]`.
- **Logo:** `public/aether-mark.svg`. Native icons are generated from it by `pnpm run desktop:icons`
  (gitignored output).

## Platform

- Windows-only today: launching store URIs (`ShellExecuteW`), Steam, Epic and Riot discovery (registry,
  ProgramData), `.lnk` export, start with Windows, the global hotkey, controller detection, the PS button, power
  actions and bringing the window to the front.
- On Linux these return "needs Windows" errors. The UI, Living colors and Big Box work.

## Testing the UI without Windows

- Serve with `pnpm exec vite --port 8080`, and drive the page with Playwright (preinstalled).
- Add an init script that stands in for the desktop side:
  - set `globalThis.isTauri = true`;
  - define `window.__TAURI_INTERNALS__` with `invoke`, `transformCallback`, `convertFileSrc`, and
    `metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } }`;
  - answer the commands the page calls, and `plugin:event|listen` for events.
- Pretend controller: override `navigator.getGamepads` with a standard-mapping pad, and dispatch
  `gamepadconnected`.
- Say in your report which parts were only verified with stand-ins.
