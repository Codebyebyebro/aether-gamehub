# Project conventions

- React 18 + TypeScript + Tailwind CSS; reuse customized shadcn/ui primitives without editing `src/components/ui/`.
- Keep React Router routes in `src/App.tsx`; use HashRouter for native asset URLs. The main page is `src/pages/Index.tsx`.
- Put frontend source in `src/`, pages in `src/pages/`, and Aether components in `src/components/aether/`.
- Native Windows architecture is Tauri 2. Rust source is under `src/native/`; Cargo/Tauri configuration is under `src-tauri/` with custom source paths.
- Use the typed desktop adapter in `src/lib/aether.ts`. Browser preview data is isolated and explicitly labeled; never simulate native scans, launches, file storage, or shortcut export success.
- Persist native data locally under the per-user application data directory. No hosted backend is needed for native library storage.
- Game cards/details show titles and artwork only; platform/path information belongs in management interfaces.
- Default theme is cream/indigo; also support navy/violet and midnight/cyan. Centralize colors in CSS variables, avoid gradients, and preserve accessible contrast.
- Bundle fonts locally. Windows builds generate native icons from `public/aether-mark.svg`.
- End users use the Windows offline setup and executable without terminal/Node.js steps. Never claim native artifacts or performance targets have been verified without Windows build/test evidence.
- Controller input goes through `useController` (`src/hooks/`), which is layered: the most recently mounted enabled instance receives input. Add `data-pad-skip` to controls that mouse users need but a controller should step over, and `data-pad-launch` to a cover that Cross should launch directly. A layer can pass `rules` (`PadRules`: which actions take fast taps and which repeat when held); the default keeps the strict debounce and repeats only the directions.
- The on-screen keyboard (`OnScreenKeyboard`, mounted once in `Index.tsx`; layout and editing rules in `src/lib/keyboard.ts`) opens when Cross is pressed on a text field, via the `KEYBOARD_EVENT` that `activate()` in `spatialNav.ts` dispatches. It writes through the native value setter plus an `input` event so React-controlled fields update, and it never takes focus from the field. Do not give new text fields their own pad handling; make sure they are a normal `input`/`textarea` of a text type and not `readOnly`.
- Every Rust command must be listed in `src-tauri/permissions/library.toml`; release builds reject any that are missing, and a Rust test enforces it. Do not add the dialog, fs, or shell plugins; native file dialogs and launching are done by Rust commands.
- `storage::APP_ID` must equal the Tauri `identifier`. The backend owns `playtime.json`; the interface listens for `aether:*` events and never writes playtime.
- Secrets (the Steam API key) are never written to a backup or logged, and free-text settings with strict validation keep a local draft until the value is valid.
