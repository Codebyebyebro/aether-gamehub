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
