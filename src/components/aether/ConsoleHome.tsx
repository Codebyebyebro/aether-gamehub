import { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Heart, Info, Minimize2, Play, ArrowLeft, Search, Radar, Settings2 } from "lucide-react";
import { KEYBOARD_EVENT } from "@/lib/spatialNav";
import { toast } from "sonner";
import { Game, PlaytimeEntry, Settings, desktop } from "@/lib/aether";
import { sortGames, playedLine, agoWords } from "@/lib/recent";
import { formatPlaytime } from "@/lib/format";
import type { AbsentMap } from "@/lib/absent";
import { useController } from "@/hooks/useController";
import { prefetchColors } from "@/hooks/useLivingColors";
import { playCue } from "@/lib/cue";
import { HOME_ACTIONS, DETAIL_ACTIONS, HEADER_ACTIONS, rowScroll, fullscreenAfterBigBox, searchGames, type ConsoleAction, type HeaderAction } from "@/lib/consoleNav";
import PadHints from "@/components/aether/PadHints";
import { Artwork, AbsentBadge, type Actions, type Category } from "@/components/aether/GameViews";

/** "14:05" in the player's own clock format, refreshed every few seconds. */
function useClock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => { const timer = setInterval(() => setNow(new Date()), 10_000); return () => clearInterval(timer); }, []);
  return now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

/**
 * Big Box as a console home screen: the selected game's artwork fills the screen, a row of tiles (recently played
 * first) runs along the top, and Square opens a full-screen details page. The selection follows the game, not its
 * position, so it stays put when the order changes (for example when a game you just played moves to the front).
 */
export default function ConsoleHome({ games, times, settings, actions, close, category, absent = {}, playingId, onCurrent, steppingAside, stayFullscreen, fullscreenBefore, onScan, onSettings }: {
  games: Game[]; times: Record<string, PlaytimeEntry>; settings: Settings; actions: Actions; close: () => void; category: Category;
  absent?: AbsentMap; playingId?: string | null; onCurrent?: (game: Game | undefined) => void;
  /** Asked as Big Box closes: is it only stepping aside for a dialog, and has the player asked to stay full screen? */
  steppingAside: () => boolean; stayFullscreen: () => boolean;
  /** Whether the window was full screen before Big Box first opened; kept by the page across stepping aside. */
  fullscreenBefore: { current: boolean | null };
  /** Big Box steps aside for these and comes back when they close. */
  onScan: () => void; onSettings: () => void;
}) {
  const [query, setQuery] = useState("");
  const ordered = useMemo(() => searchGames(sortGames(games, "recent", times), query), [games, times, query]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [details, setDetails] = useState(false);
  /** Where the controller is: the row of games, or the buttons under the title (Down goes there, Up comes back). */
  const [zone, setZone] = useState<"header" | "row" | "actions">("row");
  const [actionIndex, setActionIndex] = useState(0);
  const [headerIndex, setHeaderIndex] = useState(0);
  const current = ordered.find(g => g.id === selectedId) ?? ordered[0];
  const index = current ? ordered.indexOf(current) : -1;
  const clock = useClock();
  const row = useRef<HTMLDivElement>(null);
  const buttons = details ? DETAIL_ACTIONS : HOME_ACTIONS;

  function move(step: number) { if (ordered.length < 2) return; setSelectedId(ordered[(index + step + ordered.length) % ordered.length].id); playCue("move"); }
  function cycle(step: number) { category.set(category.options[(category.options.indexOf(category.value) + step + category.options.length) % category.options.length]); }
  function moveAction(step: number) { const next = Math.max(0, Math.min(buttons.length - 1, actionIndex + step)); if (next !== actionIndex) { setActionIndex(next); playCue("move"); } }
  function toActions() { if (zone !== "actions") { setZone("actions"); setActionIndex(0); playCue("move"); } }
  function toRow() { if (zone !== "row" && !details && ordered.length) { setZone("row"); playCue("move"); } }
  function toHeader() { if (zone !== "header" && !details) { setZone("header"); setHeaderIndex(0); playCue("move"); } }
  function moveHeader(step: number) { const next = Math.max(0, Math.min(HEADER_ACTIONS.length - 1, headerIndex + step)); if (next !== headerIndex) { setHeaderIndex(next); playCue("move"); } }
  const searchField = useRef<HTMLInputElement>(null);
  function runHeader(action: HeaderAction) {
    if (action === "search") { const f = searchField.current; if (f) { f.focus(); f.dispatchEvent(new CustomEvent(KEYBOARD_EVENT, { bubbles: true })); } }
    else if (action === "scan") onScan(); else if (action === "settings") onSettings(); else close();
  }
  function openDetails(open: boolean) { setDetails(open); setZone(open ? "actions" : "row"); setActionIndex(0); if (open) playCue("open"); }
  const play = () => { if (current) actions.launch(current); };
  const favorite = () => { if (current) { actions.favorite(current); playCue("favorite"); } };
  const run = (action: ConsoleAction) => { if (action === "play") play(); else if (action === "details") openDetails(true); else if (action === "favorite") favorite(); else openDetails(false); };

  useEffect(() => { setSelectedId(null); setDetails(false); setZone(z => z === "header" ? z : "row"); }, [category.value]);
  useEffect(() => { setSelectedId(null); }, [query]);
  useEffect(() => { onCurrent?.(current); }, [current, onCurrent]);
  useEffect(() => () => onCurrent?.(undefined), [onCurrent]);
  // Neighbours' colours are fetched ahead, so Living colors is ready before the selection gets there.
  const neighbours = index >= 0 ? [1, -1, 2, -2].map(n => ordered[(index + n + ordered.length) % ordered.length]) : [];
  const neighbourKey = neighbours.map(g => g.id).join();
  useEffect(() => { prefetchColors(neighbours); }, [neighbourKey]); // eslint-disable-line react-hooks/exhaustive-deps -- keyed by the games, not the array
  // Only ever scroll the row sideways, to where the selected tile will sit once it has finished growing. Measuring a
  // tile mid-animation, or letting the browser scroll vertically as well, is what made the row jump up and down.
  useEffect(() => { const r = row.current; if (r && index >= 0) r.scrollTo({ left: rowScroll(index, r.clientWidth), behavior: settings.performance ? "auto" : "smooth" }); }, [index, settings.performance]);
  // The focused button follows the controller, so its ring shows and Enter or a screen reader agree with it.
  useEffect(() => {
    const root = document.querySelector<HTMLElement>('[aria-label="Big Box library"]');
    if (zone === "actions") root?.querySelector<HTMLElement>(`[data-console-action="${buttons[actionIndex]}"]`)?.focus({ preventScroll: true });
    else if (zone === "header") { const el = root?.querySelector<HTMLElement>(`[data-console-header="${HEADER_ACTIONS[headerIndex]}"]`); if (el && document.activeElement !== el) el.focus({ preventScroll: true }); }
    else if (root && root.contains(document.activeElement) && document.activeElement !== root) root.focus({ preventScroll: true });
  }, [zone, actionIndex, headerIndex, details, buttons]);

  const root = () => document.querySelector<HTMLElement>('[aria-label="Big Box library"]');
  const pad = useController({
    left: () => { if (zone === "header") moveHeader(-1); else if (zone === "row" && !details) move(-1); else moveAction(-1); },
    right: () => { if (zone === "header") moveHeader(1); else if (zone === "row" && !details) move(1); else moveAction(1); },
    down: () => { if (details) return; if (zone === "header") toRow(); else toActions(); },
    up: () => { if (zone === "actions") toRow(); else if (zone === "row") toHeader(); },
    confirm: () => { if (zone === "header") runHeader(HEADER_ACTIONS[headerIndex]); else if (!current) return; else if (zone === "row" && !details) play(); else run(buttons[actionIndex]); },
    details: () => { if (current && zone !== "header") openDetails(!details); }, secondary: () => { if (zone !== "header") favorite(); },
    back: () => { if (details) openDetails(false); else if (zone === "actions" || (zone === "header" && ordered.length)) toRow(); else close(); },
    prevTab: () => { if (!details) cycle(-1); }, nextTab: () => { if (!details) cycle(1); },
  });
  // Big Box is full screen. On the way out it only leaves full screen if it turned it on itself, the player has not asked
  // to stay full screen, and it is not just stepping aside for Settings or a scan.
  const asks = useRef({ steppingAside, stayFullscreen }); asks.current = { steppingAside, stayFullscreen };
  useEffect(() => {
    if (!desktop) return;
    const win = getCurrentWindow();
    // Coming back from Settings or a scan, the window is still full screen from this Big Box: keep the first answer.
    const ready: Promise<void> = fullscreenBefore.current !== null ? Promise.resolve()
      : win.isFullscreen().catch(() => false).then(full => { fullscreenBefore.current = full; });
    void ready.then(() => win.setFullscreen(true)).catch(e => toast.error(String(e)));
    return () => { void ready.then(() => {
      if (asks.current.steppingAside()) return; // full screen stays, and so does what was true before Big Box
      const keep = fullscreenAfterBigBox(!!fullscreenBefore.current, asks.current.stayFullscreen());
      fullscreenBefore.current = null;
      if (!keep) win.setFullscreen(false).catch(() => {});
    }); };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps -- once per Big Box; the page owns `fullscreenBefore`
  useEffect(() => {
    const previous = document.activeElement as HTMLElement;
    const root = document.querySelector<HTMLElement>('[aria-label="Big Box library"]');
    root?.setAttribute("tabindex", "-1"); root?.focus();
    return () => previous?.focus();
  }, []);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      // Typing in the search field: only Down, Enter and Escape mean something to Big Box; every other key is text.
      if (e.target === searchField.current) {
        if (e.key === "ArrowDown" || e.key === "Enter") { e.preventDefault(); searchField.current?.blur(); setZone(ordered.length ? "row" : "header"); root()?.focus({ preventScroll: true }); }
        else if (e.key === "Escape") { e.preventDefault(); searchField.current?.blur(); if (query) setQuery(""); else root()?.focus({ preventScroll: true }); }
        return;
      }
      const onButton = !!(e.target as HTMLElement).closest("button");
      if (["Escape", "ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown"].includes(e.key)) e.preventDefault();
      if (e.key === "Escape") { if (details) openDetails(false); else if (zone === "actions" || (zone === "header" && ordered.length)) toRow(); else close(); }
      if (e.key === "ArrowRight") { if (zone === "header") moveHeader(1); else if (zone === "row" && !details) move(1); else moveAction(1); }
      if (e.key === "ArrowLeft") { if (zone === "header") moveHeader(-1); else if (zone === "row" && !details) move(-1); else moveAction(-1); }
      if (e.key === "ArrowDown" && !details) { if (zone === "header") toRow(); else toActions(); }
      if (e.key === "ArrowUp") { if (zone === "actions") toRow(); else if (zone === "row") toHeader(); }
      if ((e.key === "i" || e.key === "I") && !e.ctrlKey && !e.altKey) openDetails(!details);
      if (e.key === "Enter" && current && !onButton && zone !== "header") { e.preventDefault(); if (!e.repeat) play(); }
      if (e.key === "Tab") {
        const root = document.querySelector<HTMLElement>('[aria-label="Big Box library"]');
        const all = root?.querySelectorAll<HTMLButtonElement>("button:not([disabled])");
        if (!all?.length) return;
        const first = all[0], last = all[all.length - 1];
        if (!root.contains(document.activeElement) || document.activeElement === root) { e.preventDefault(); (e.shiftKey ? last : first).focus(); }
        else if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
        else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  const entry = current ? times[current.id] : undefined;
  const line = current ? playedLine(entry) : "";
  const playing = !!current && current.id === playingId;
  const hints: [string, string][] = details ? [["◀ ▶", "Choose"], ["✕", "OK"], ["△", "Favorite"], ["○", "Back"]]
    : zone === "actions" ? [["◀ ▶", "Choose"], ["✕", "OK"], ["▲", "Games"], ["○", "Back"]]
    : zone === "header" ? [["◀ ▶", "Choose"], ["✕", "OK"], ["▼", "Games"], ["○", "Back"]]
    : [["◀ ▶", "Browse"], ["▲", "Search & settings"], ["▼", "Options"], ["✕", "Play"], ["□", "Details"], ["△", "Favorite"], ["○", "Exit"], ["L1 R1", "Category"]];
  const focusClass = (action: ConsoleAction) => zone === "actions" && buttons[actionIndex] === action ? "is-focus" : "";
  const headClass = (action: HeaderAction) => `console-head-btn ${zone === "header" && HEADER_ACTIONS[headerIndex] === action ? "is-focus" : ""}`;

  return <div role="dialog" aria-modal="true" aria-label="Big Box library" className="bigbox-bg console-home fixed inset-0 z-40 flex flex-col overflow-hidden text-white">
    {current && <div key={current.id} className="console-hero" aria-hidden><Artwork game={current} banner performance={settings.performance} /></div>}
    <div className="console-shade" aria-hidden />

    <header className="relative z-10 flex items-center justify-between gap-4 px-8 pt-6">
      <div className="flex items-center gap-3"><img src="/aether-mark.svg" className="h-9 w-9" alt="" /><span className="display-font font-bold">Aether Hub</span></div>
      <div role="group" aria-label="Category" className="flex flex-wrap items-center justify-center gap-1.5">{pad.connected && <kbd className="pad-key">L1</kbd>}{category.options.map(option => <button key={option} aria-pressed={category.value === option} onClick={() => category.set(option)} className={`h-8 rounded-full px-4 text-[11px] font-semibold ${category.value === option ? "bg-white text-[#131a2c]" : "bg-white/10 text-white/75 hover:bg-white/20"}`}>{option === "All" ? "All games" : option === "Custom" ? "Local" : option}</button>)}{pad.connected && <kbd className="pad-key">R1</kbd>}</div>
      <div className="flex items-center gap-2">
        <label className={`${headClass("search")} console-search`}><Search size={15} className="shrink-0" /><input ref={searchField} data-console-header="search" type="search" value={query} onChange={e => setQuery(e.target.value)} onFocus={() => { setZone("header"); setHeaderIndex(0); }} placeholder="Search" aria-label="Search games" className="w-28 bg-transparent text-[12px] text-white outline-none placeholder:text-white/55 xl:w-40" /></label>
        <button data-console-header="scan" className={headClass("scan")} aria-label="Scan PC for games" title="Scan PC for games" onClick={onScan}><Radar size={16} /></button>
        <button data-console-header="settings" className={headClass("settings")} aria-label="Settings" title="Settings" onClick={onSettings}><Settings2 size={16} /></button>
        <span className="console-clock mx-2 tabular-nums" aria-label={`Time ${clock}`}>{clock}</span>
        <button data-console-header="exit" className={`${headClass("exit")} gap-2 px-3`} onClick={close}><Minimize2 size={15} />Exit</button>
      </div>
    </header>

    {current ? <>
      <div ref={row} role="listbox" aria-label="Games" aria-activedescendant={`tile-${current.id}`} className="console-row relative z-10 mt-7 flex items-end gap-3 overflow-x-auto px-8 pb-3 pt-2">
        {ordered.map(g => { const on = g.id === current.id; return <button key={g.id} id={`tile-${g.id}`} data-tile={g.id} role="option" aria-selected={on} aria-label={g.title} onClick={() => { setZone("row"); if (on) play(); else { setSelectedId(g.id); playCue("move"); } }} className={`console-tile shrink-0 overflow-hidden rounded-2xl ${on ? "is-on" : ""}`}><Artwork game={g} performance={settings.performance} /></button>; })}
      </div>

      {!details ? <main className="relative z-10 mt-auto px-8 pb-6 md:px-12">
        <p className="console-eyebrow">{playing ? "Now playing" : entry?.lastPlayed ? "Jump back in" : current.favorite ? "Favorite" : current.platform === "Custom" ? "Local game" : current.platform}</p>
        <h1 className="display-font console-title max-w-4xl">{current.title}</h1>
        <AbsentBadge reason={absent[current.id]} className="mt-3" />
        <p className="mt-2 text-[14px] text-white/80">{line ? line[0].toUpperCase() + line.slice(1) : "Not played yet"}</p>
        <div className="mt-6 flex items-center gap-3">
          <button data-console-action="play" className={`console-play ${focusClass("play")}`} onClick={play}><Play size={18} fill="currentColor" />Play{pad.connected && zone === "row" && <kbd className="pad-key ml-1 border-white/40">✕</kbd>}</button>
          <button data-console-action="details" className={`console-round ${focusClass("details")}`} aria-label="Details" onClick={() => openDetails(true)}><Info size={20} /></button>
          <button data-console-action="favorite" className={`console-round ${focusClass("favorite")}`} aria-label={current.favorite ? "Remove favorite" : "Favorite"} aria-pressed={current.favorite} onClick={favorite}><Heart size={20} fill={current.favorite ? "currentColor" : "none"} /></button>
        </div>
      </main>
      : <section aria-label={`${current.title} details`} className="console-details relative z-10 mx-8 mb-6 mt-auto flex gap-8 rounded-3xl p-6 md:mx-12 md:p-8">
        <div className="hidden aspect-[2/3] w-48 shrink-0 overflow-hidden rounded-2xl shadow-2xl md:block"><Artwork game={current} performance={settings.performance} /></div>
        <div className="min-w-0 flex-1">
          <p className="mb-4 flex items-center gap-2 text-[12px] text-white/60"><ArrowLeft size={14} />Details</p>
          <h1 className="display-font text-3xl font-extrabold md:text-4xl">{current.title}</h1>
          <AbsentBadge reason={absent[current.id]} className="mt-2" />
          <dl className="mt-5 grid max-w-xl grid-cols-2 gap-x-8 gap-y-4 text-[13px]">
            <div><dt className="text-white/60">Time played</dt><dd className="mt-0.5 font-semibold">{formatPlaytime(entry?.seconds)}</dd></div>
            <div><dt className="text-white/60">Last played</dt><dd className="mt-0.5 font-semibold">{entry?.lastPlayed ? agoWords(entry.lastPlayed) : "Never"}</dd></div>
            <div><dt className="text-white/60">Platform</dt><dd className="mt-0.5 font-semibold">{current.platform === "Custom" ? "Local game" : current.platform}</dd></div>
            <div><dt className="text-white/60">Favorite</dt><dd className="mt-0.5 font-semibold">{current.favorite ? "Yes" : "No"}</dd></div>
          </dl>
          <div className="mt-6 flex items-center gap-3">
            <button data-console-action="play" className={`console-play ${focusClass("play")}`} onClick={play}><Play size={18} fill="currentColor" />Play</button>
            <button data-console-action="favorite" className={`console-round ${focusClass("favorite")}`} aria-label={current.favorite ? "Remove favorite" : "Favorite"} aria-pressed={current.favorite} onClick={favorite}><Heart size={20} fill={current.favorite ? "currentColor" : "none"} /></button>
            <button data-console-action="back" className={`console-round ${focusClass("back")}`} aria-label="Back to games" onClick={() => openDetails(false)}><ArrowLeft size={20} /></button>
          </div>
        </div>
      </section>}
    </> : <div className="relative z-10 flex flex-1 flex-col items-center justify-center gap-2 text-white/70"><p>{query ? `No games match “${query}”.` : "No games match your filters."}</p>{query && <button className="rounded-full bg-white/10 px-4 py-1.5 text-[12px] hover:bg-white/20" onClick={() => setQuery("")}>Clear search</button>}</div>}

    <footer className="relative z-10 flex items-center justify-center gap-8 px-8 pb-5 text-[11px] text-white/60">{pad.connected ? <PadHints items={hints} /> : <><span>← → Browse</span><span>Enter Play</span><span>I Details</span><span>Esc {details ? "Back" : "Exit"}</span></>}<span className="tabular-nums">{ordered.length ? index + 1 : 0} / {ordered.length}</span></footer>
  </div>;
}
