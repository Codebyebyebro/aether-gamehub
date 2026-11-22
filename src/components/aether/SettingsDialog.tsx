import { useState } from "react";
import { FolderPlus, X, Zap, ShieldCheck, Monitor, ScanSearch } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Settings, PlaytimeStore, Restored, pickFile, desktop } from "@/lib/aether";
import DiscordSettings from "@/components/aether/settings/DiscordSettings";
import SteamSettings from "@/components/aether/settings/SteamSettings";
import BackupSettings from "@/components/aether/settings/BackupSettings";
import { toast } from "sonner";

const launchModes = [
  { id: "keep", name: "Do Nothing", note: "Aether Hub stays open beside your game. Playtime tracking and Discord status keep working." },
  { id: "minimize", name: "Minimize Launcher", note: "Aether Hub minimizes to the taskbar and keeps tracking playtime and Discord status quietly in the background." },
  { id: "close", name: "Close Launcher Completely", note: "Frees all of the launcher’s memory and CPU while you play. Playtime tracking and Discord status pause, because nothing is left running. Platform clients may stay open." },
] as const;

interface Props {
  settings: Settings; update: (patch: Partial<Settings>) => void; close: () => void; firstRun?: boolean;
  playtime: PlaytimeStore; flush: () => Promise<void>; applyRestore: (restored: Restored) => void; refreshPlaytime: () => unknown;
}
export default function SettingsDialog({ settings, update, close, firstRun = false, playtime, flush, applyRestore, refreshPlaytime }: Props) {
  const [manual, setManual] = useState("");
  async function folder() { try { const path = await pickFile("folder"); if (path && !settings.folders.includes(path)) update({ folders: [...settings.folders, path] }); } catch(e) { toast.error(String(e)); } }
  const mode = launchModes.find(m => m.id === settings.afterLaunch) ?? launchModes[1];
  return <Dialog open onOpenChange={v => !v && close()}><DialogContent className="max-h-[90vh] max-w-xl overflow-y-auto rounded-3xl bg-card p-7"><DialogHeader>{firstRun && <img src="/aether-mark.svg" className="mb-4 h-12 w-12" alt="Aether Hub" />}<DialogTitle className="display-font text-2xl font-extrabold">{firstRun ? "Welcome to your next adventure." : "Built around your PC."}</DialogTitle><DialogDescription className="text-xs">{firstRun ? "A few preferences. You can change these anytime." : "A little less overhead. A lot more play."}</DialogDescription></DialogHeader>
    <Tabs defaultValue="general" className="mt-3">
      <TabsList className="grid w-full grid-cols-3 rounded-xl"><TabsTrigger value="general" className="rounded-lg text-xs">General</TabsTrigger><TabsTrigger value="integrations" className="rounded-lg text-xs">Integrations</TabsTrigger><TabsTrigger value="backup" className="rounded-lg text-xs">Backup</TabsTrigger></TabsList>
      <TabsContent value="general" className="mt-5 space-y-6">
        <section><h3 className="mb-3 flex items-center gap-2 text-sm font-semibold"><Monitor size={16} />When a game starts</h3>
          <Select value={settings.afterLaunch} onValueChange={afterLaunch => update({ afterLaunch: afterLaunch as Settings["afterLaunch"] })}><SelectTrigger aria-label="When a game starts" className="h-10 rounded-xl text-xs"><SelectValue /></SelectTrigger><SelectContent>{launchModes.map(m => <SelectItem key={m.id} value={m.id} className="text-xs">{m.name}</SelectItem>)}</SelectContent></Select>
          <p className="mt-3 text-xs leading-relaxed text-muted-foreground">{mode.note}</p></section>
        <section className="rounded-xl bg-secondary p-4"><div className="flex items-center justify-between"><h3 className="flex items-center gap-2 text-sm font-semibold"><Zap size={16} />Performance mode</h3><Switch aria-label="Performance mode" checked={settings.performance} onCheckedChange={performance => update({ performance })} /></div><p className="mt-2 text-xs leading-relaxed text-muted-foreground">Turns off blur, shadows and animations, skips the wallpaper, and loads smaller artwork and fewer posters at a time. WebView2 still needs its own memory, so there is no fixed 50 MB promise; for zero use while gaming, choose “Close Launcher Completely”.</p></section>
        <section><h3 className="mb-2 text-sm font-semibold">Custom scan folders</h3><p className="mb-3 text-xs leading-relaxed text-muted-foreground">Steam, Epic, and Riot locations are discovered automatically. Add folders for your other games. Custom scans are limited to six levels.</p><div className="space-y-2">{settings.folders.map(f => <div key={f} className="flex items-center gap-2 rounded-xl border px-3 py-2 text-xs"><span className="flex-1 truncate font-mono">{f}</span><button aria-label={`Remove ${f}`} onClick={() => update({ folders: settings.folders.filter(x => x !== f) })}><X size={14} /></button></div>)}</div><Button variant="outline" className="mt-3 w-full rounded-xl text-xs" onClick={folder}><FolderPlus size={14} className="mr-2" />Choose folder</Button>{!desktop && <div className="mt-2 flex gap-2"><input aria-label="Preview custom folder path" className="aether-field min-w-0 flex-1 px-3 text-xs" placeholder="Preview folder path" value={manual} onChange={e => setManual(e.target.value)} /><Button variant="ghost" className="rounded-xl text-xs" onClick={() => { if (manual.trim() && !settings.folders.includes(manual.trim())) update({ folders: [...settings.folders, manual.trim()] }); setManual(""); }}>Add</Button></div>}
          <p className="mt-3 flex gap-2 text-[11px] leading-relaxed text-muted-foreground"><ScanSearch size={14} className="mt-0.5 shrink-0" /><span>The scanner skips programs under 2 MB and anything named like an installer, updater, crash handler, helper, redistributable, browser, document or school folder. Files skipped for their name are listed under “Filtered out” in the scan review, so a real game with an unlucky name can still be imported.</span></p></section>
      </TabsContent>
      <TabsContent value="integrations" className="mt-5 space-y-6"><DiscordSettings settings={settings} update={update} /><SteamSettings settings={settings} update={update} playtime={playtime} refreshPlaytime={refreshPlaytime} /></TabsContent>
      <TabsContent value="backup" className="mt-5"><BackupSettings settings={settings} update={update} flush={flush} applyRestore={applyRestore} /></TabsContent>
    </Tabs>
    <div className="mt-6 space-y-6"><section className="flex gap-3 border-t pt-4"><ShieldCheck size={20} className="shrink-0 text-[var(--aether-accent)]" /><p className="text-xs leading-relaxed text-muted-foreground">Local library. No Aether account. No telemetry. Steam artwork is fetched from its CDN; optional SteamGridDB requests use your session-only key. If you turn them on, Steam playtime sync asks Steam for your public profile and Discord status talks only to the Discord app on this PC. Storefront clients may require their usual login.</p></section><Button className="aether-primary w-full" onClick={() => { update({ onboarded: true }); close(); }}>{firstRun ? "Let’s make it yours" : "Done"}</Button></div></DialogContent></Dialog>;
}
