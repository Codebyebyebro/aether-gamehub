import { useEffect, useState } from "react";
import { Play, Heart, MoreHorizontal, Pencil, Trash2, ChevronLeft, ChevronRight, Minimize2, Gamepad2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Game, Settings, imageUrl, steamImage, desktop } from "@/lib/aether";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import { useController } from "@/hooks/useController";
import PadHints from "@/components/aether/PadHints";

export function Artwork({ game, banner = false, performance = false, className = "" }: { game: Game; banner?: boolean; performance?: boolean; className?: string }) {
  const [failed, setFailed] = useState(false);
  const [cached, setCached] = useState<{ source: string; path: string } | null>(null);
  const original = banner ? game.banner || game.cover : game.cover || game.banner;
  const source = performance && game.steamId && original.includes("steamstatic.com") ? `https://cdn.akamai.steamstatic.com/steam/apps/${game.steamId}/header.jpg` : imageUrl(original) || (game.steamId ? steamImage(game.steamId, banner) : "");
  useEffect(() => { setFailed(false); let active = true; if (desktop && source.startsWith("https:")) invoke<string>("resolve_artwork", { url: source }).then(path => { if (active) { setCached({ source, path }); setFailed(false); } }).catch(() => {}); return () => { active = false; }; }, [source]);
  const resolved = cached?.source === source ? imageUrl(cached.path) : desktop && source.startsWith("https:") ? "" : source;
  return resolved && !failed ? <img src={resolved} alt={game.title} loading="lazy" decoding="async" className={`h-full w-full object-cover ${className}`} onError={() => setFailed(true)} /> : <div className={`flex h-full w-full flex-col items-center justify-center gap-4 bg-secondary px-4 text-center text-[var(--aether-accent)] ${className}`}><Gamepad2 size={36} strokeWidth={1} /><span className="display-font text-sm font-bold">{game.title}</span></div>;
}
interface Actions { launch: (g: Game) => void; edit: (g: Game) => void; remove: (g: Game) => void; favorite: (g: Game) => void; detail: (g: Game) => void }
export function GameMenu({ game, actions }: { game: Game; actions: Actions }) {
  return <DropdownMenu><DropdownMenuTrigger asChild><button data-pad-skip className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground hover:bg-muted" aria-label={`Manage ${game.title}`} onClick={e => e.stopPropagation()}><MoreHorizontal size={17} /></button></DropdownMenuTrigger><DropdownMenuContent align="end" className="rounded-xl p-1.5"><DropdownMenuItem className="rounded-lg text-xs" onClick={() => actions.launch(game)}><Play size={14} className="mr-2" />Launch game</DropdownMenuItem><DropdownMenuItem className="rounded-lg text-xs" onClick={() => actions.favorite(game)}><Heart size={14} className="mr-2" />{game.favorite ? "Remove favorite" : "Favorite"}</DropdownMenuItem><DropdownMenuItem className="rounded-lg text-xs" onClick={() => actions.edit(game)}><Pencil size={14} className="mr-2" />Edit game</DropdownMenuItem><DropdownMenuItem className="rounded-lg text-xs text-destructive focus:text-destructive" onClick={() => actions.remove(game)}><Trash2 size={14} className="mr-2" />Remove from library</DropdownMenuItem></DropdownMenuContent></DropdownMenu>;
}
export function GameGrid({ games, settings, actions }: { games: Game[]; settings: Settings; actions: Actions }) {
  const page = settings.performance ? 24 : 48; // fewer artwork decodes at a time on low-end PCs
  const [limit, setLimit] = useState(page);
  useEffect(() => setLimit(page), [games, page]);
  const list = settings.view === "list"; const banner = settings.view === "banner";
  if (settings.view === "carousel") return null;
  return <><div className={list ? "space-y-2" : "grid"} style={!list ? { gridTemplateColumns: `repeat(auto-fill, minmax(min(100%, ${banner ? settings.size * 1.6 : settings.size}px), 1fr))`, gap: `${settings.gap}px` } : undefined}>{games.slice(0, limit).map((g, i) => list ? <div key={g.id} className="flex items-center gap-4 rounded-xl border bg-card p-2 pr-3"><button data-pad-skip className="h-14 w-24 shrink-0 overflow-hidden rounded-lg" onClick={() => actions.detail(g)}><Artwork game={g} banner performance={settings.performance} /></button><button className="flex-1 text-left text-sm font-semibold" onClick={() => actions.detail(g)}>{g.title}</button><Button variant="ghost" size="icon" className="rounded-xl text-[var(--aether-accent)]" aria-label={`Launch ${g.title}`} onClick={() => actions.launch(g)}><Play size={17} /></Button><GameMenu game={g} actions={actions} /></div> : <article key={g.id} className="group min-w-0"><button data-pad-launch={g.id} data-pad-default={i === 0 ? "" : undefined} onClick={() => actions.detail(g)} className={`game-cover block w-full shadow-[0_4px_12px_rgba(30,25,60,0.08)] transition-transform duration-200 hover:-translate-y-1 ${banner ? "aspect-[16/9]" : "aspect-[2/3]"}`} style={{ borderRadius: settings.radius }}><Artwork game={g} banner={banner} performance={settings.performance} /><span className="absolute inset-0 flex items-center justify-center bg-slate-950/20 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"><span className="flex h-12 w-12 items-center justify-center rounded-full bg-white/95 text-[var(--aether-accent)]"><Play size={19} fill="currentColor" /></span></span>{g.favorite && <span className="absolute right-2 top-2 rounded-full bg-slate-950/45 p-1.5 text-white"><Heart size={10} fill="currentColor" /></span>}</button><div className="mt-2.5 flex items-center justify-between gap-1">{settings.titles ? <button data-pad-skip className="truncate text-left text-[12px] font-semibold hover:text-[var(--aether-accent)]" title={g.title} onClick={() => actions.detail(g)}>{g.title}</button> : <span />}<GameMenu game={g} actions={actions} /></div></article>)}</div>{games.length > limit && <Button variant="outline" className="mx-auto mt-8 flex rounded-xl text-xs" onClick={() => setLimit(v => v + page)}>Show more games ({games.length - limit} remaining)</Button>}</>;
}
/** The platform filter shared with the main library, so L1/R1 and the pills here change what the page shows too. */
export interface Category { options: readonly string[]; value: string; set: (value: string) => void }
export function Carousel({ games, settings, actions, close, category }: { games: Game[]; settings: Settings; actions: Actions; close: () => void; category: Category }) {
  const [index, setIndex] = useState(0);
  const current = games[index % Math.max(1, games.length)];
  function move(n: number) { setIndex(i => (i + n + games.length) % Math.max(1, games.length)); }
  function cycle(n: number) { category.set(category.options[(category.options.indexOf(category.value) + n + category.options.length) % category.options.length]); }
  useEffect(() => setIndex(0), [category.value]);
  // This layer sits on top of the page's own controller handling for as long as Big Box is open.
  const pad = useController({ left: () => move(-1), right: () => move(1), confirm: () => { if (current) actions.launch(current); }, back: close, prevTab: () => cycle(-1), nextTab: () => cycle(1) });
  useEffect(() => { if (desktop) getCurrentWindow().setFullscreen(true).catch(e => toast.error(String(e))); return () => { if (desktop) getCurrentWindow().setFullscreen(false).catch(() => {}); }; }, []);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement;
    const root = document.querySelector<HTMLElement>('[aria-label="Big Box library"]');
    root?.setAttribute("tabindex", "-1"); root?.focus();
    return () => previous?.focus();
  }, []);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (["Escape", "ArrowRight", "ArrowLeft"].includes(e.key)) e.preventDefault();
      if (e.key === "Escape") close();
      if (e.key === "ArrowRight") move(1);
      if (e.key === "ArrowLeft") move(-1);
      if (e.key === "Enter" && current && !(e.target as HTMLElement).closest("button")) { e.preventDefault(); if (!e.repeat) actions.launch(current); }
      if (e.key === "Tab") {
        const root = document.querySelector<HTMLElement>('[aria-label="Big Box library"]');
        const buttons = root?.querySelectorAll<HTMLButtonElement>("button:not([disabled])");
        if (!buttons?.length) return;
        const first = buttons[0], last = buttons[buttons.length - 1];
        if (!root.contains(document.activeElement) || document.activeElement === root) { e.preventDefault(); (e.shiftKey ? last : first).focus(); }
        else if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
        else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [current, games, close, actions]);
  return <div role="dialog" aria-modal="true" aria-label="Big Box library" className="fixed inset-0 z-40 flex flex-col bg-[#131a2c] text-white"><header className="flex items-center justify-between gap-4 p-6"><div className="flex items-center gap-3"><img src="/aether-mark.svg" className="h-9 w-9" alt="" /><span className="display-font font-bold">Aether Hub <span className="ml-3 text-xs font-normal text-white/60">Big Box</span></span></div><div role="group" aria-label="Category" className="flex flex-wrap items-center justify-center gap-1.5">{pad.connected && <kbd className="pad-key">L1</kbd>}{category.options.map(option => <button key={option} aria-pressed={category.value === option} onClick={() => category.set(option)} className={`h-8 rounded-full px-4 text-[11px] font-semibold ${category.value === option ? "bg-white text-[#131a2c]" : "bg-white/10 text-white/70 hover:bg-white/20"}`}>{option === "All" ? "All games" : option === "Custom" ? "Local" : option}</button>)}{pad.connected && <kbd className="pad-key">R1</kbd>}</div><Button variant="ghost" className="rounded-xl text-white hover:bg-white/10 hover:text-white" onClick={close}><Minimize2 size={16} className="mr-2" />Exit fullscreen</Button></header>{current ? <main className="flex min-h-0 flex-1 flex-col items-center justify-center px-4"><div className="flex w-full max-w-5xl items-center gap-4"><button aria-label="Previous game" className="rounded-full bg-white/10 p-3 hover:bg-white/20" onClick={() => move(-1)}><ChevronLeft /></button><button className="aspect-[16/8] min-w-0 flex-1 overflow-hidden rounded-3xl shadow-2xl" onClick={() => actions.launch(current)}><Artwork game={current} banner performance={settings.performance} /></button><button aria-label="Next game" className="rounded-full bg-white/10 p-3 hover:bg-white/20" onClick={() => move(1)}><ChevronRight /></button></div><h1 className="mt-7 text-center text-3xl font-extrabold md:text-4xl">{current.title}</h1><Button className="aether-primary mt-5 h-11 gap-2 px-8" onClick={() => actions.launch(current)}><Play size={16} fill="currentColor" />Play game</Button><div className="mt-6 flex gap-3">{[-1,0,1].map(n => { const g = games[(index + n + games.length) % games.length]; return <button key={n} aria-label={g.title} onClick={() => move(n)} className={`h-16 w-28 overflow-hidden rounded-lg border-2 ${n === 0 ? "border-white" : "border-transparent opacity-40"}`}><Artwork game={g} banner performance={settings.performance} /></button>; })}</div></main> : <div className="flex flex-1 items-center justify-center text-white/70">No games match your filters.</div>}<footer className="flex justify-center gap-8 p-5 text-[11px] text-white/50">{pad.connected ? <PadHints items={[["D-pad", "Browse"], ["✕", "Play"], ["○", "Exit"], ["L1 R1", "Category"]]} /> : <><span>← → Browse</span><span>Enter Play</span><span>Esc Exit</span></>}<span>{games.length ? index % games.length + 1 : 0} / {games.length}</span></footer></div>;
}
