import { useState } from "react";
import { Loader2, RefreshCw, Gamepad2 } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Settings, PlaytimeStore, desktop, steamSyncNow } from "@/lib/aether";
import { formatAgo } from "@/lib/format";
import { toast } from "sonner";
import { cardClass, fail, fieldClass } from "./shared";

const validKey = (value: string) => /^[0-9a-fA-F]{32}$/.test(value);
const intervals = [{ hours: 1, name: "Every hour" }, { hours: 6, name: "Every 6 hours" }, { hours: 24, name: "Once a day" }];

export default function SteamSettings({ settings, update, playtime, refreshPlaytime }: { settings: Settings; update: (patch: Partial<Settings>) => void; playtime: PlaytimeStore; refreshPlaytime: () => unknown }) {
  const [key, setKey] = useState(settings.steamApiKey); // committed only when complete, so a half-typed key never reverts the form
  const [busy, setBusy] = useState(false);
  const steam = playtime.steam;
  const incomplete = key !== "" && !validKey(key);
  const ready = settings.steamProfile.trim() !== "" && !incomplete;
  function editKey(value: string) { const next = value.replace(/[^0-9a-fA-F]/g, "").slice(0, 32); setKey(next); if (next === "" || validKey(next)) update({ steamApiKey: next }); }
  async function sync(profile = settings.steamProfile, apiKey = key) {
    if (!profile.trim()) { toast.error("Enter your Steam profile first."); return; }
    setBusy(true);
    try {
      const result = await steamSyncNow(profile, validKey(apiKey) ? apiKey : "");
      toast.success(`Matched ${result.matched} of your library’s games (${result.owned} owned on Steam)${result.raised ? `. Updated the playtime of ${result.raised} game${result.raised === 1 ? "" : "s"}` : ". Everything was already up to date"}.`);
    } catch (e) { fail(e); } finally { setBusy(false); refreshPlaytime(); }
  }
  function toggle(on: boolean) { update({ steamSyncEnabled: on }); if (on && ready && !steam.syncedAt) sync(); }
  const status = steam.error ? steam.error : steam.syncedAt ? `Last synced ${formatAgo(steam.syncedAt)}. ${steam.matched} of your library’s games matched (${steam.owned} owned on Steam).` : "Not synced yet.";
  return <section className={cardClass}>
    <div className="flex items-center justify-between gap-3"><h3 className="flex items-center gap-2 text-sm font-semibold"><Gamepad2 size={16} />Steam playtime sync</h3><Switch aria-label="Steam playtime sync" checked={settings.steamSyncEnabled} onCheckedChange={toggle} /></div>
    <p className="mt-2 text-xs leading-relaxed text-muted-foreground">Keeps the hours in your library accurate by reading the total playtime from your public Steam profile. Totals only ever go up, so time you played through Aether Hub is never counted twice.</p>
    {settings.steamSyncEnabled && <div className="mt-4 space-y-3">
      <label className="block text-xs font-medium">Steam profile
        <input value={settings.steamProfile} onChange={e => update({ steamProfile: e.target.value })} autoComplete="off" spellCheck={false} placeholder="steamcommunity.com/id/yourname, or your SteamID64" className={`${fieldClass} mt-1.5`} />
      </label>
      <label className="block text-xs font-medium">Steam Web API key <span className="font-normal text-muted-foreground">(optional)</span>
        <input type="password" value={key} onChange={e => editKey(e.target.value)} autoComplete="off" spellCheck={false} placeholder="32 characters. Leave empty to use the public profile" className={`${fieldClass} mt-1.5 font-mono`} />
      </label>
      {incomplete && <p className="text-[11px] text-destructive">A Steam Web API key is 32 characters (0 to 9 and a to f). It will be saved once complete.</p>}
      <p className="text-[11px] leading-relaxed text-muted-foreground">Your profile’s <strong>Game details</strong> must be set to Public. A key makes syncing steadier; it stays on this PC and is never written to a backup.</p>
      <div className="flex flex-wrap items-center gap-2">
        <Select value={String(settings.steamSyncHours)} onValueChange={hours => update({ steamSyncHours: Number(hours) })}>
          <SelectTrigger aria-label="How often to sync" className="h-8 w-44 rounded-xl text-xs"><SelectValue /></SelectTrigger>
          <SelectContent>{intervals.map(i => <SelectItem key={i.hours} value={String(i.hours)} className="text-xs">{i.name}</SelectItem>)}</SelectContent>
        </Select>
        <Button variant="outline" className="h-8 rounded-xl text-xs" disabled={!desktop || busy || !ready} onClick={() => sync()}>{busy ? <Loader2 size={13} className="mr-2 animate-spin" /> : <RefreshCw size={13} className="mr-2" />}Sync now</Button>
      </div>
      <p className={`text-[11px] leading-relaxed ${steam.error ? "text-destructive" : "text-muted-foreground"}`}>{status}</p>
      {!desktop && <p className="text-[11px] text-muted-foreground">Syncing needs the Windows desktop app.</p>}
    </div>}
  </section>;
}
