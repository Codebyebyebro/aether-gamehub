import { useEffect, useState } from "react";
import { AlertTriangle, Copy, Loader2, Users } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Settings, DiscordStatus, desktop, discordStatus, discordTest } from "@/lib/aether";
import { toast } from "sonner";
import { cardClass, fail, fieldClass } from "./shared";

const validId = (value: string) => /^\d{17,20}$/.test(value);
const portal = "discord.com/developers/applications";

export default function DiscordSettings({ settings, update }: { settings: Settings; update: (patch: Partial<Settings>) => void }) {
  // The backend rejects a half-typed id, and a rejected save reverts the form, so only valid values are committed.
  const [draft, setDraft] = useState(settings.discordAppId);
  const [status, setStatus] = useState<DiscordStatus | null>(null);
  const [testing, setTesting] = useState(false);
  const incomplete = draft !== "" && !validId(draft);
  useEffect(() => {
    if (!desktop) return;
    let alive = true;
    const poll = () => discordStatus().then(s => alive && setStatus(s)).catch(() => {});
    poll(); const timer = setInterval(poll, 4000);
    return () => { alive = false; clearInterval(timer); };
  }, []);
  function edit(value: string) {
    const digits = value.replace(/\D/g, "").slice(0, 20);
    setDraft(digits);
    if (digits === "" || validId(digits)) update({ discordAppId: digits });
  }
  async function test() {
    setTesting(true);
    try { await discordTest(draft); toast.success("Discord accepted the test. It shows on your profile for a moment, then clears."); }
    catch (e) { fail(e); } finally { setTesting(false); }
  }
  async function copyLink() { try { await navigator.clipboard.writeText(`https://${portal}`); toast.success("Link copied. Paste it into your browser."); } catch { toast.message(`Open ${portal} in your browser.`); } }
  const note = status?.error ? status.error : status?.playing ? `Showing “${status.playing}” on your Discord profile.` : "Ready. Your status appears when you launch a game from Aether Hub.";
  return <section className={cardClass}>
    <div className="flex items-center justify-between gap-3"><h3 className="flex items-center gap-2 text-sm font-semibold"><Users size={16} />Discord Rich Presence</h3><Switch aria-label="Discord Rich Presence" checked={settings.discordEnabled} onCheckedChange={discordEnabled => update({ discordEnabled })} /></div>
    <p className="mt-2 text-xs leading-relaxed text-muted-foreground">Friends see the game you launch from Aether Hub, with a play timer: Steam, Epic, Riot or any standalone game. It clears when the game closes. Only the Discord app on this PC is contacted.</p>
    {settings.discordEnabled && <div className="mt-4 space-y-3">
      <label className="block text-xs font-medium">Discord Application ID
        <input value={draft} onChange={e => edit(e.target.value)} inputMode="numeric" autoComplete="off" spellCheck={false} placeholder="18 digits, from the Discord developer portal" className={`${fieldClass} mt-1.5 font-mono`} />
      </label>
      {incomplete && <p className="text-[11px] text-destructive">An Application ID is 17 to 20 digits. It will be saved once complete.</p>}
      <ol className="list-decimal space-y-1 pl-4 text-[11px] leading-relaxed text-muted-foreground">
        <li>Open <span className="font-mono text-foreground">{portal}</span> and create a free application. Its name is what friends see first.</li>
        <li>Copy its <strong>Application ID</strong> into the box above.</li>
        <li>Optional: upload an image named <span className="font-mono text-foreground">aether_hub</span> under Rich Presence, Art Assets.</li>
      </ol>
      <div className="flex flex-wrap gap-2">
        <Button variant="outline" className="h-8 rounded-xl text-xs" onClick={copyLink}><Copy size={13} className="mr-2" />Copy link</Button>
        <Button variant="outline" className="h-8 rounded-xl text-xs" disabled={!desktop || testing || !validId(draft)} onClick={test}>{testing && <Loader2 size={13} className="mr-2 animate-spin" />}Test connection</Button>
      </div>
      {desktop && <p className={`text-[11px] ${status?.error ? "text-destructive" : "text-muted-foreground"}`}>{note}</p>}
      {settings.afterLaunch === "close" && <p className="flex gap-2 rounded-lg bg-secondary p-3 text-[11px] leading-relaxed"><AlertTriangle size={14} className="mt-0.5 shrink-0" />“Close Launcher Completely” is selected, so nothing stays running to show your game on Discord. Choose Minimize or Do Nothing on the General tab.</p>}
      {!desktop && <p className="text-[11px] text-muted-foreground">Rich Presence needs the Windows desktop app and the Discord desktop app.</p>}
    </div>}
  </section>;
}
