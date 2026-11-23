import { useCallback, useEffect, useState } from "react";
import { DatabaseBackup, FolderOpen, Loader2, RotateCcw, Upload } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { Settings, BackupOverview, Restored, desktop, pickFile, backupOverview, backupNow, backupExport, backupRestore, openBackupFolder } from "@/lib/aether";
import { formatAgo, formatBytes, shortPath } from "@/lib/format";
import { toast } from "sonner";
import { cardClass, fail } from "./shared";

type Busy = "" | "now" | "export" | "restore";
export default function BackupSettings({ settings, update, flush, applyRestore }: { settings: Settings; update: (patch: Partial<Settings>) => void; flush: () => Promise<void>; applyRestore: (restored: Restored) => void }) {
  const [overview, setOverview] = useState<BackupOverview | null>(null);
  const [busy, setBusy] = useState<Busy>("");
  const [confirm, setConfirm] = useState<{ path?: string; what: string } | null>(null);
  const reload = useCallback((dir: string) => backupOverview(dir).then(setOverview).catch(fail), []);
  useEffect(() => { if (desktop) reload(settings.backupDir); }, [settings.backupDir, reload]);
  async function run(kind: Exclude<Busy, "">, job: () => Promise<void>) { setBusy(kind); try { await flush(); await job(); } catch (e) { fail(e); } finally { setBusy(""); } }
  const backUp = () => run("now", async () => { const made = await backupNow(settings.backupDir, settings.backupKeep, false); toast.success(`Backed up to ${made.info.name}`); await reload(settings.backupDir); });
  const exportFile = () => run("export", async () => { const done = await backupExport(); if (done) toast.success(`Exported with cover images to ${shortPath(done.path, 60)}`, { description: done.warnings.join(" ") || undefined }); });
  const restore = (path?: string) => run("restore", async () => {
    const restored = await backupRestore(path);
    if (!restored) return;
    applyRestore(restored); await reload(settings.backupDir);
    toast.success(`Restored ${restored.games} game${restored.games === 1 ? "" : "s"} and their playtime.`, { description: restored.missingCovers ? `${restored.missingCovers} cover image${restored.missingCovers === 1 ? " was" : "s were"} not in the backup and will use Steam or title artwork.` : "Your previous library was saved first, so this can be undone from the list." });
  });
  async function chooseFolder() { try { const dir = await pickFile("folder"); if (dir) update({ backupDir: dir }); } catch (e) { fail(e); } }
  const recent = overview?.backups.slice(0, 5) ?? [];
  const folder = overview?.dir ?? settings.backupDir ?? "";
  return <div className="space-y-6">
    <section className={cardClass}>
      <div className="flex items-center justify-between gap-3"><h3 className="flex items-center gap-2 text-sm font-semibold"><DatabaseBackup size={16} />Backup &amp; cloud folder</h3><Switch aria-label="Back up automatically" checked={settings.backupEnabled} onCheckedChange={backupEnabled => update({ backupEnabled })} /></div>
      <p className="mt-2 text-xs leading-relaxed text-muted-foreground">Backs up your library, playtime and cover artwork references shortly after you change something, and when Aether Hub starts. Aether Hub has no server: choose a folder inside OneDrive, Dropbox or Google Drive and your library follows you to other PCs.</p>
      <div className="mt-4 space-y-3">
        <div className="flex items-center gap-2 rounded-xl border px-3 py-2 text-xs"><span className="min-w-0 flex-1 truncate font-mono" title={folder}>{folder ? shortPath(folder, 64) : "Default folder in Aether Hub’s data"}</span>
          <button className="shrink-0 text-[var(--aether-accent)] disabled:opacity-40" disabled={!desktop} onClick={chooseFolder}>Change</button>
          {settings.backupDir && <button className="shrink-0 text-muted-foreground" onClick={() => update({ backupDir: "" })}>Default</button>}
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Select value={String(settings.backupKeep)} onValueChange={keep => update({ backupKeep: Number(keep) })}>
            <SelectTrigger aria-label="How many backups to keep" className="h-8 w-40 rounded-xl text-xs"><SelectValue /></SelectTrigger>
            <SelectContent>{[5, 10, 20, 50].map(n => <SelectItem key={n} value={String(n)} className="text-xs">Keep {n} backups</SelectItem>)}</SelectContent>
          </Select>
          <Button variant="outline" className="h-8 rounded-xl text-xs" disabled={!desktop || !!busy} onClick={backUp}>{busy === "now" && <Loader2 size={13} className="mr-2 animate-spin" />}Back up now</Button>
          <Button variant="ghost" className="h-8 rounded-xl text-xs" disabled={!desktop || !folder} onClick={() => openBackupFolder(settings.backupDir).catch(fail)}><FolderOpen size={13} className="mr-2" />Open folder</Button>
        </div>
        {overview?.lastError && <p className="text-[11px] text-destructive">The last automatic backup failed: {overview.lastError}</p>}
        {!desktop && <p className="text-[11px] text-muted-foreground">Backups need the Windows desktop app.</p>}
      </div>
    </section>
    <section>
      <h3 className="mb-2 text-sm font-semibold">Recent backups</h3>
      {recent.length ? <div className="divide-y rounded-xl border">{recent.map(b => <div key={b.path} className="flex items-center gap-3 px-3 py-2.5 text-xs"><span className="min-w-0 flex-1"><span className="block">{formatAgo(b.createdAt)}{b.name.includes("before-restore") && <span className="ml-2 rounded bg-secondary px-1.5 py-0.5 text-[10px] text-muted-foreground">before a restore</span>}</span><span className="block truncate text-[10px] text-muted-foreground" title={b.name}>{b.name} · {formatBytes(b.sizeBytes)}</span></span><Button variant="ghost" className="h-7 rounded-lg px-2 text-[11px]" disabled={!!busy} onClick={() => setConfirm({ path: b.path, what: `the backup from ${formatAgo(b.createdAt)}` })}><RotateCcw size={12} className="mr-1.5" />Restore</Button></div>)}</div> : <p className="rounded-xl border p-4 text-xs text-muted-foreground">{desktop ? "No backups yet. The first one is made shortly after your next change." : "Nothing to show in the browser preview."}</p>}
    </section>
    <section className="flex flex-wrap gap-2">
      <Button variant="outline" className="rounded-xl text-xs" disabled={!desktop || !!busy} onClick={exportFile}>{busy === "export" ? <Loader2 size={14} className="mr-2 animate-spin" /> : <Upload size={14} className="mr-2" />}Export to a file…</Button>
      <Button variant="outline" className="rounded-xl text-xs" disabled={!desktop || !!busy} onClick={() => setConfirm({ what: "a backup file you choose" })}><RotateCcw size={14} className="mr-2" />Restore from a file…</Button>
      <p className="w-full text-[11px] leading-relaxed text-muted-foreground">An exported file also carries your custom cover images, so it works on a PC that has never seen them. Your Steam API key is never included.</p>
    </section>
    <AlertDialog open={!!confirm} onOpenChange={open => !open && setConfirm(null)}><AlertDialogContent className="rounded-3xl bg-card"><AlertDialogHeader><AlertDialogTitle>Replace your library?</AlertDialogTitle><AlertDialogDescription>Your library, playtime and settings will be replaced by {confirm?.what}. What you have now is saved as a “before a restore” backup first, so you can undo this from the list. Your installed games are never touched.</AlertDialogDescription></AlertDialogHeader><AlertDialogFooter><AlertDialogCancel className="rounded-xl">Keep my library</AlertDialogCancel><AlertDialogAction className="aether-primary rounded-xl" onClick={() => { const target = confirm; setConfirm(null); if (target) restore(target.path); }}>Restore</AlertDialogAction></AlertDialogFooter></AlertDialogContent></AlertDialog>
  </div>;
}
