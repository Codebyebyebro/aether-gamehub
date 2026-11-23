import { useEffect, useRef, useState } from "react";
import { ImageDown, Loader2, CheckCircle2, ChevronRight } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { Game, findArtwork, cancelArtwork, listenDesktop } from "@/lib/aether";
import { ArtFound, ArtOutcome, ArtProgress, artworkPercent, artworkSummary, wantedFrom } from "@/lib/artwork";
import { toast } from "sonner";

/**
 * "Find missing artwork". Asks first (it looks the games' names up online), then shows progress, then what was added and
 * what was left alone. `apply` puts the found covers onto the library and returns how many games changed. Closing the
 * window while it runs stops the lookup; covers found up to then are still added.
 */
export default function ArtworkFinder({ games, close, apply, afterScan = false }: { games: Game[]; close: () => void; apply: (found: ArtFound[]) => Promise<number>; afterScan?: boolean }) {
  // Fixed when the window opens: once covers are applied those games no longer need artwork, but the results still name them.
  const [wanted] = useState(() => wantedFrom(games));
  const [titles] = useState(() => new Map(wanted.map(g => [g.id, g.title])));
  const [phase, setPhase] = useState<"ask" | "running" | "done">("ask");
  const [key, setKey] = useState(""); const [progress, setProgress] = useState<ArtProgress | null>(null); const [stopping, setStopping] = useState(false);
  const [outcome, setOutcome] = useState<ArtOutcome | null>(null); const [applied, setApplied] = useState(0); const [error, setError] = useState("");
  const open = useRef(true); const running = useRef(false);
  useEffect(() => () => { open.current = false; if (running.current) cancelArtwork().catch(() => {}); }, []);

  async function start() {
    setPhase("running"); setError(""); setProgress({ done: 0, total: wanted.length, title: wanted[0]?.title ?? "" }); running.current = true;
    const off = await listenDesktop<ArtProgress>("aether:artwork-progress", p => { if (open.current) setProgress(p); });
    try {
      const result = await findArtwork(wanted, key.trim());
      running.current = false;
      const count = await apply(result.found);
      if (!open.current) { if (count) toast.success(`${count} cover${count === 1 ? "" : "s"} added to your library`); return; }
      setOutcome(result); setApplied(count); setPhase("done");
    } catch (e) {
      running.current = false;
      if (open.current) { setError(String(e)); setPhase("ask"); } else toast.error(String(e));
    } finally { off(); }
  }
  function stop() { setStopping(true); cancelArtwork().catch(e => { setStopping(false); toast.error(String(e)); }); }
  const percent = artworkPercent(progress);
  const added = outcome?.found.filter(f => titles.has(f.id)) ?? [];

  return <Dialog open onOpenChange={v => !v && close()}><DialogContent className="max-h-[88vh] max-w-[600px] overflow-y-auto rounded-3xl bg-card p-7">
    <DialogHeader><div className="mb-3 flex h-12 w-12 items-center justify-center rounded-2xl bg-secondary text-[var(--aether-accent)]"><ImageDown size={23} /></div>
      <DialogTitle className="display-font text-2xl font-extrabold">{phase === "done" ? "Your shelf, dressed up." : "Find missing artwork."}</DialogTitle>
      <DialogDescription className="text-xs leading-relaxed">{phase === "done" ? artworkSummary(outcome!, applied, wanted.length) : `${afterScan ? "Some of the games you just added" : `${wanted.length} game${wanted.length === 1 ? "" : "s"}`} ${wanted.length === 1 && !afterScan ? "has" : "have"} no cover picture yet.`}</DialogDescription></DialogHeader>

    {phase === "ask" && <div className="mt-4 space-y-4">
      <p className="text-xs leading-relaxed text-muted-foreground">Aether Hub can look {wanted.length === 1 ? "it" : `all ${wanted.length}`} up by name in Steam’s public store. No account or sign-in is needed, and only the game names are sent. A cover is added only when the name matches exactly; anything uncertain is left as it is, so you never get the wrong picture.</p>
      <details className="rounded-xl border p-3 text-xs"><summary className="cursor-pointer font-medium">Games not sold on Steam? Add a SteamGridDB key (optional)</summary>
        <p className="my-3 leading-relaxed text-muted-foreground">Games such as VALORANT or Fortnite aren’t on Steam. With your own free SteamGridDB API key, those are looked up there too. The key is used for this search only and is not saved.</p>
        <Input type="password" autoComplete="off" placeholder="SteamGridDB API key" aria-label="SteamGridDB API key" className="aether-field text-xs" value={key} onChange={e => setKey(e.target.value)} /></details>
      {error && <p role="alert" className="rounded-xl border border-destructive/40 bg-destructive/10 p-3 text-xs leading-relaxed">{error}</p>}
      <div className="flex justify-end gap-2"><Button variant="ghost" className="rounded-xl" onClick={close}>{afterScan ? "Not now" : "Cancel"}</Button><Button className="aether-primary gap-2" disabled={!wanted.length} onClick={start}><ImageDown size={15} />Find artwork</Button></div>
    </div>}

    {phase === "running" && <div className="flex flex-col items-center gap-3 py-8 text-center">
      <Loader2 className="animate-spin text-[var(--aether-accent)]" size={28} />
      <p className="mt-1 max-w-full truncate text-sm">{stopping ? "Stopping after this game…" : progress?.title ? `Looking up ${progress.title}…` : "Finishing up…"}</p>
      <Progress value={percent} aria-valuenow={percent} aria-label="Artwork search progress" className="h-2 w-full max-w-sm" />
      <p className="text-xs tabular-nums text-muted-foreground">{Math.min(progress?.done ?? 0, wanted.length)} of {wanted.length} games</p>
      <Button variant="outline" className="mt-2 rounded-xl" disabled={stopping} onClick={stop}>Stop</Button>
    </div>}

    {phase === "done" && outcome && <div className="mt-4 space-y-3">
      {added.length > 0 && <div className="overflow-hidden rounded-xl border"><div className="border-b bg-muted/60 px-3 py-1.5 text-[10px] font-bold uppercase tracking-wider text-muted-foreground">Added</div>
        <ul className="max-h-56 overflow-auto">{added.map(f => <li key={f.id} className="flex items-center gap-3 border-b px-3 py-2 text-xs last:border-0"><CheckCircle2 size={14} className="shrink-0 text-[var(--aether-accent)]" /><span className="min-w-0 flex-1 truncate font-medium">{titles.get(f.id)}</span><span className="shrink-0 text-[10px] text-muted-foreground">{f.source}</span></li>)}</ul></div>}
      {outcome.unmatched.length > 0 && <details className="group rounded-xl border" open={!added.length}><summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-2.5 text-xs font-medium"><ChevronRight size={14} className="transition-transform group-open:rotate-90" />Left as they were ({outcome.unmatched.length})</summary>
        <ul className="max-h-56 overflow-auto border-t">{outcome.unmatched.map(u => <li key={u.id} className="flex items-center gap-3 border-b px-3 py-2 text-xs last:border-0"><span className="min-w-0 flex-1 truncate">{u.title}</span><span className="shrink-0 text-right text-[10px] text-muted-foreground">{u.reason}</span></li>)}</ul>
        <p className="border-t px-3 py-2 text-[11px] leading-relaxed text-muted-foreground">You can still choose a picture yourself: edit the game and click its cover.</p></details>}
      <div className="flex justify-end"><Button className="aether-primary px-6" onClick={close}>Done</Button></div>
    </div>}
  </DialogContent></Dialog>;
}
