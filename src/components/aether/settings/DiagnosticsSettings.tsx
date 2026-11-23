import { useState } from "react";
import { ClipboardCopy, LifeBuoy, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { diagnostics, desktop } from "@/lib/aether";
import { copyText } from "@/lib/clipboard";
import { fail, cardClass } from "./shared";

/** "Copy diagnostics": a short report to paste where help is asked for. The program builds it; this only copies it. */
export default function DiagnosticsSettings() {
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [shown, setShown] = useState("");
  async function copy() {
    setBusy(true); setMessage(""); setShown("");
    try {
      const text = await diagnostics();
      if (await copyText(text)) setMessage(`Copied ${text.split("\n").length} lines. Paste them where you ask for help.`);
      else { setShown(text); setMessage("Windows did not allow copying from here. Select the text below and copy it yourself."); }
    } catch (e) { fail(e); } finally { setBusy(false); }
  }
  return <section className={cardClass}>
    <div className="mb-3 flex items-center justify-between gap-3">
      <h3 className="flex items-center gap-2 text-sm font-semibold"><LifeBuoy size={16} />If something does not work</h3>
      <Button variant="outline" className="h-8 rounded-xl text-xs" disabled={!desktop || busy} onClick={copy}>{busy ? <Loader2 size={13} className="mr-2 animate-spin" /> : <ClipboardCopy size={13} className="mr-2" />}Copy diagnostics</Button>
    </div>
    <p className="text-[11px] leading-relaxed text-muted-foreground">Copies a short report: the version, which background features are on and working, how much memory Aether Hub uses, and the last entries of a log kept only on this PC (<span className="font-mono">aether-hub.log</span> in the Aether Hub data folder). It holds no keys or ids and nothing is sent anywhere, but an error line can name a game's folder, so read it before you share it.</p>
    <p role="status" aria-live="polite" className="mt-2 text-[11px] leading-relaxed">{message}</p>
    {shown && <textarea readOnly aria-label="Diagnostics text" value={shown} rows={8} onFocus={e => e.currentTarget.select()} className="mt-2 w-full rounded-lg border bg-secondary p-2 font-mono text-[10px] leading-snug" />}
    {!desktop && <p className="mt-2 text-[11px] text-muted-foreground">The report is built by the Windows desktop app. This browser preview cannot make one.</p>}
  </section>;
}
