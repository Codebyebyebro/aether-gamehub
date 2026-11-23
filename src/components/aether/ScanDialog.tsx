import { useEffect, useRef, useState } from "react";
import { Radar, CheckCheck, Loader2, AlertTriangle, FolderSearch, ChevronRight } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Progress } from "@/components/ui/progress";
import { Game, ScanDetail, ScanProgress, ScanResult, scanGames, cancelScan, listenDesktop, desktop, gameIdentity as identity } from "@/lib/aether";
import { formatBytes, scanCountsLabel, scanFraction, scanStepLabel, shortPath } from "@/lib/format";
import { toast } from "sonner";

/** One reviewable line: a clean location on the left, the size and where it came from on the right. */
function Row({ game, detail, checked, disabled, note, onChange }: { game: Game; detail: ScanDetail; checked: boolean; disabled?: boolean; note?: string; onChange: (on: boolean) => void }) {
  const installed = detail.source === "Steam" || detail.source === "Epic";
  return <label className={`flex cursor-pointer items-center gap-3 border-b p-3 last:border-0 ${disabled ? "opacity-50" : "hover:bg-muted"}`}>
    <Checkbox checked={checked && !disabled} disabled={disabled} onCheckedChange={v => onChange(v === true)} />
    <span className="min-w-0 flex-1">
      <span className="block truncate text-sm font-medium">{game.title}</span>
      <span className="block truncate font-mono text-[10px] text-muted-foreground" title={detail.path}>{shortPath(detail.path, 64)}</span>
      {note && <span className="mt-0.5 block text-[10px] text-amber-700 dark:text-amber-400">{note}</span>}
    </span>
    <span className="w-[4.5rem] shrink-0 text-right text-[11px] tabular-nums" title={installed ? "Installed size reported by the store" : "File size"}>{formatBytes(detail.sizeBytes)}</span>
    <span className="w-16 shrink-0 text-right text-[10px] text-muted-foreground">{disabled ? "In library" : detail.source}</span>
  </label>;
}
const unknown = (g: Game): ScanDetail => ({ sizeBytes: null, path: g.target, source: g.platform === "Custom" ? "Folder" : g.platform });

export default function ScanDialog({ existing, folders, close, importGames }: { existing: Game[]; folders: string[]; close: () => void; importGames: (games: Game[]) => Promise<void> }) {
  const [result, setResult] = useState<ScanResult | null>(null); const [error, setError] = useState(""); const [selected, setSelected] = useState<Set<string>>(new Set()); const [busy, setBusy] = useState(false);
  const [scanId] = useState(() => crypto.randomUUID()); const [progress, setProgress] = useState<ScanProgress | null>(null); const [stopping, setStopping] = useState(false);
  // While the scan call is unsettled, closing this review (Esc, Circle, ×) stops the scan rather than leaving it running.
  const running = useRef(true); const stopRequested = useRef(false);
  const duplicates = new Set(existing.map(identity));
  useEffect(() => {
    let active = true, unlisten: (() => void) | undefined;
    (async () => {
      // Subscribe before the scan starts so its first progress events are not missed.
      const off = await listenDesktop<ScanProgress>("aether:scan-progress", p => { if (p.scanId === scanId) setProgress(p); });
      if (!active) { off(); return; }
      unlisten = off;
      const r = await scanGames(folders, scanId).finally(() => { running.current = false; });
      if (!active) return;
      if (r.cancelled || stopRequested.current) { close(); return; }
      setResult(r); setSelected(new Set(r.games.filter(g => !duplicates.has(identity(g))).map(g => g.id)));
    })().catch(e => { if (active) setError(String(e)); });
    return () => { active = false; unlisten?.(); if (running.current) cancelScan(scanId).catch(() => {}); };
  }, []);
  function stop() { stopRequested.current = true; setStopping(true); cancelScan(scanId).catch(e => { stopRequested.current = false; setStopping(false); toast.error(String(e)); }); }
  const available = result?.games.filter(g => !duplicates.has(identity(g))) || [];
  const rescue = result?.filtered.filter(f => !duplicates.has(identity(f.game))) || [];
  const toggle = (id: string) => (on: boolean) => setSelected(s => { const next = new Set(s); if (on) next.add(id); else next.delete(id); return next; });
  async function confirm() {
    if (!result) return; setBusy(true);
    try { await importGames([...result.games, ...result.filtered.map(f => f.game)].filter(g => selected.has(g.id) && !duplicates.has(identity(g)))); close(); }
    catch(e) { toast.error(String(e)); } finally { setBusy(false); }
  }
  // ui/progress.tsx draws the bar from `value` but does not hand it to the progressbar element, so pass it for screen readers.
  const percent = Math.round(scanFraction(progress) * 100);
  const stats = result?.stats; const byName = stats ? stats.blacklisted + stats.knownTools : 0;
  const skipped = stats ? [stats.tooSmall && `${stats.tooSmall} under 2 MB`, byName && `${byName} by name or program type`, stats.programs && `${stats.programs} installed programs with no game files`, stats.installers && `${stats.installers} installers or downloads`, stats.alreadyManaged && `${stats.alreadyManaged} already listed by Steam or Epic`].filter(Boolean) : [];
  return <Dialog open onOpenChange={v => !v && close()}><DialogContent className="max-h-[88vh] max-w-[720px] overflow-y-auto rounded-3xl bg-card p-7"><DialogHeader><div className="mb-3 flex h-12 w-12 items-center justify-center rounded-2xl bg-secondary text-[var(--aether-accent)]"><Radar size={23} /></div><DialogTitle className="display-font text-2xl font-extrabold">Discover your next session.</DialogTitle><DialogDescription className="text-xs">Review detected games before adding them. Nothing launches automatically.</DialogDescription></DialogHeader>
    {error ? <div className="my-5 rounded-2xl border bg-muted p-5"><FolderSearch size={28} className="mb-3 text-muted-foreground" /><p className="text-sm font-semibold">{desktop ? "We couldn’t finish this scan." : "Your PC, not this browser."}</p><p className="mt-2 text-xs leading-relaxed text-muted-foreground">{error}</p><Button variant="outline" className="mt-4 rounded-xl" onClick={close}>Back to library</Button></div>
    : !result ? <div className="flex flex-col items-center gap-3 py-10 text-center"><Loader2 className="animate-spin text-[var(--aether-accent)]" size={28} /><p className="mt-1 text-sm">{stopping ? "Stopping the scan…" : scanStepLabel(progress)}</p><Progress value={percent} aria-valuenow={percent} aria-valuetext={scanStepLabel(progress)} aria-label="Scan progress" className="h-2 w-full max-w-sm" /><p className="text-xs tabular-nums text-muted-foreground">{scanCountsLabel(progress)}</p>{progress?.folder && <p className="max-w-full truncate font-mono text-[10px] text-muted-foreground" title={progress.folder}>{shortPath(progress.folder, 64)}</p>}<Button variant="outline" className="mt-2 rounded-xl" disabled={stopping} onClick={stop}>{stopping && <Loader2 size={14} className="mr-2 animate-spin" />}{stopping ? "Stopping…" : "Cancel scan"}</Button><p className="text-[11px] text-muted-foreground">Nothing is added until you review the results.</p></div>
    : <>
      <div className="my-4 flex items-center justify-between text-xs"><span className="text-muted-foreground">{result.games.length} detected · {available.length} new</span><button className="flex items-center gap-1.5 text-[var(--aether-accent)]" onClick={() => setSelected(new Set(available.every(g => selected.has(g.id)) ? [] : [...selected, ...available.map(g => g.id)]))}><CheckCheck size={14} />{available.length && available.every(g => selected.has(g.id)) ? "Deselect all" : "Select all"}</button></div>
      <div className="overflow-hidden rounded-xl border">
        <div className="flex items-center gap-3 border-b bg-muted/60 px-3 py-1.5 text-[10px] font-bold uppercase tracking-wider text-muted-foreground"><span className="w-4" /><span className="flex-1">Game and location</span><span className="w-[4.5rem] text-right">Size</span><span className="w-16 text-right">Source</span></div>
        <div className="max-h-80 overflow-auto">{result.games.length ? result.games.map(g => <Row key={g.id} game={g} detail={result.details[g.id] ?? unknown(g)} checked={selected.has(g.id)} disabled={duplicates.has(identity(g))} onChange={toggle(g.id)} />) : <div className="p-8 text-center text-sm text-muted-foreground">No games found. Add a custom folder in Settings or add a game manually.</div>}</div>
      </div>
      {skipped.length > 0 && <p className="mt-3 text-[11px] leading-relaxed text-muted-foreground">Skipped automatically: {skipped.join(" · ")}.</p>}
      {rescue.length > 0 && <details className="group mt-3 rounded-xl border"><summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-2.5 text-xs font-medium"><ChevronRight size={14} className="transition-transform group-open:rotate-90" />Filtered out ({rescue.length})<span className="font-normal text-muted-foreground">Large files rejected by name. Tick any that are really games.</span></summary><div className="max-h-56 overflow-auto border-t">{rescue.map(f => <Row key={f.game.id} game={f.game} detail={f.detail} note={f.reason} checked={selected.has(f.game.id)} onChange={toggle(f.game.id)} />)}</div></details>}
      {result.warnings.map((w,i) => <p key={i} className="mt-3 flex gap-2 text-xs text-muted-foreground"><AlertTriangle size={14} className="shrink-0" />{w}</p>)}
      <div className="mt-5 flex justify-end gap-2"><Button variant="ghost" className="rounded-xl" onClick={close}>Cancel</Button><Button className="aether-primary" disabled={!selected.size || busy} onClick={confirm}>{busy && <Loader2 size={14} className="mr-2 animate-spin" />}Import {selected.size} game{selected.size === 1 ? "" : "s"}</Button></div>
    </>}
  </DialogContent></Dialog>;
}
