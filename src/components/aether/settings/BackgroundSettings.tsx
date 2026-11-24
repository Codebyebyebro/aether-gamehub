import { useCallback, useEffect, useState } from "react";
import { AlertTriangle, Download, Loader2, PanelBottom, RefreshCw } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Settings, BackgroundStatus, UpdateInfo, backgroundStatus, checkForUpdate, openReleasesPage, desktop } from "@/lib/aether";
import { fail, cardClass } from "./shared";

interface Props { settings: Settings; update: (patch: Partial<Settings>) => void; flush: () => Promise<void> }

function Row({ label, on, onChange, children, problem }: { label: string; on: boolean; onChange: (on: boolean) => void; children: React.ReactNode; problem?: string }) {
  return <div className="py-3 first:pt-0 last:pb-0">
    <div className="flex items-center justify-between gap-3"><span className="text-xs font-medium">{label}</span><Switch aria-label={label} checked={on} disabled={!desktop} onCheckedChange={onChange} /></div>
    <p className="mt-1.5 text-[11px] leading-relaxed text-muted-foreground">{children}</p>
    {on && problem && <p role="alert" className="mt-1.5 text-[11px] leading-relaxed text-destructive">{problem}</p>}
  </div>;
}

/** Tray, shortcut, start with Windows, controller wake-up and the update check. Everything here is off until switched on. */
export default function BackgroundSettings({ settings, update, flush }: Props) {
  const [status, setStatus] = useState<BackgroundStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [found, setFound] = useState<UpdateInfo | null>(null);
  const refresh = useCallback(() => { backgroundStatus().then(setStatus).catch(() => {}); }, []);
  useEffect(refresh, [refresh]);
  // Controller layouts change state as games start and come to the front, so their line is kept current.
  useEffect(() => { if (!settings.controllerLayouts || !desktop) return; const timer = setInterval(refresh, 2000); return () => clearInterval(timer); }, [settings.controllerLayouts, refresh]);
  // The program applies a change when it is saved, so save first and then read back what it is really doing.
  async function toggle(patch: Partial<Settings>) { update(patch); try { await flush(); } catch { /* the page reports a failed save itself */ } refresh(); }
  async function check() {
    setChecking(true);
    try { setFound(await checkForUpdate()); } catch (e) { setFound(null); fail(e); } finally { setChecking(false); }
  }
  const label = status?.shortcutLabel ?? "Ctrl+Alt+Space";
  return <>
    <section className={cardClass}>
      <h3 className="mb-3 flex items-center gap-2 text-sm font-semibold"><PanelBottom size={16} />Running in the background</h3>
      <div className="divide-y">
        <Row label="Keep running in the system tray" on={settings.keepInTray} onChange={keepInTray => toggle({ keepInTray })} problem={status?.trayError}>
          Closing the window hides Aether Hub next to the clock instead of quitting. Click its icon to bring it back, or right-click it for Big Box and Quit. Windows may tuck new icons under the ^ arrow: drag it out to keep it in view. Opening Aether Hub again always shows the window.
        </Row>
        <Row label={`Global shortcut (${label})`} on={settings.shortcut} onChange={shortcut => toggle({ shortcut })} problem={status?.shortcutError}>
          Press {label} from anywhere to bring Aether Hub forward. Only that one key combination is reserved; no other key is read.
        </Row>
        <Row label="Start with Windows" on={settings.startWithWindows} onChange={startWithWindows => toggle({ startWithWindows })} problem={status?.startupError}>
          Opens Aether Hub when you sign in: quietly in the tray if the tray is on, otherwise as a normal window. It adds one entry for your account only, and switching this off removes it.
        </Row>
        <Row label="Console mode" on={settings.consoleMode} onChange={consoleMode => toggle({ consoleMode })}>
          With a controller connected, Aether Hub starts straight in Big Box, and when you quit a game it comes back to Big Box instead of leaving you on the desktop. Without a controller nothing changes. It works while Aether Hub is open or minimized.
        </Row>
        <Row label="PS / Xbox button opens Aether Hub" on={settings.homeButton} onChange={homeButton => toggle({ homeButton })} problem={status?.homeError}>
          Press the PS button (or the Xbox button) during a game to open a quick menu over it: resume, go home, close the game, or the power menu. With no game running it brings Aether Hub forward in Big Box. While this is on, Aether Hub listens for that one button in the background and ignores every other button. Steam may also open its own overlay on the PS button, and the Xbox Game Bar on the Xbox button: you can turn those off in Steam (Settings, Controller) or Windows (Settings, Gaming, Xbox Game Bar).
        </Row>
        <Row label="Controller layouts" on={settings.controllerLayouts} onChange={controllerLayouts => toggle({ controllerLayouts })} problem={status?.layoutsError}>
          For games that ignore controllers: while such a game is in front, your controller types keys and moves the mouse, using the layout you chose for that game (Edit game, or Details in Big Box). Nothing is sent to Aether Hub, the desktop or other programs, and the PS / Xbox button still opens the quick menu. Games that work with Xbox controllers but not PlayStation ones need a virtual Xbox controller, which needs a driver; Aether Hub does not install drivers, so use Steam Input (“Add a Non-Steam Game”) or DS4Windows for those. Games with anti-cheat may ignore keys sent this way.
          {settings.controllerLayouts && status && <span role="status" className="mt-1.5 block font-medium text-foreground">{status.layoutsState}</span>}
        </Row>
        <Row label="Open Big Box when a controller connects" on={settings.padWake} onChange={padWake => toggle({ padWake })} problem={status?.padError}>
          Turn your controller on and Aether Hub comes forward in Big Box. It needs Aether Hub running (in the tray or minimized), and it stays out of the way while a game launched from here is running. Steam can hide a physical controller from Windows while it is running; if that happens this will not notice it.
        </Row>
      </div>
      {settings.afterLaunch === "close" && (settings.keepInTray || settings.shortcut || settings.padWake || settings.consoleMode || settings.homeButton || settings.controllerLayouts) && <p className="mt-3 flex gap-2 rounded-lg bg-secondary p-3 text-[11px] leading-relaxed"><AlertTriangle size={14} className="mt-0.5 shrink-0" />“Close Launcher Completely” is selected under “When a game starts”, so launching a game quits Aether Hub, including its tray icon, shortcut, controller wake-up, the PS / Xbox button, controller layouts and Console mode’s return to Big Box. Choose Minimize or Do nothing to keep them running.</p>}
      {!desktop && <p className="mt-3 text-[11px] text-muted-foreground">These need the Windows desktop app. This browser preview cannot change them.</p>}
    </section>
    <section className={cardClass}>
      <div className="mb-3 flex items-center justify-between gap-3">
        <h3 className="flex items-center gap-2 text-sm font-semibold"><Download size={16} />Updates</h3>
        <div className="flex items-center gap-2">
          {found?.available && <Button className="h-8 rounded-xl text-xs" onClick={() => openReleasesPage().catch(fail)}>Open download page</Button>}
          <Button variant="outline" className="h-8 rounded-xl text-xs" disabled={!desktop || checking} onClick={check}>{checking ? <Loader2 size={13} className="mr-2 animate-spin" /> : <RefreshCw size={13} className="mr-2" />}Check now</Button>
        </div>
      </div>
      <Row label="Check for updates when Aether Hub starts" on={settings.checkUpdates} onChange={checkUpdates => update({ checkUpdates })}>
        Asks GitHub once at start whether a newer release exists. It is the only thing Aether Hub contacts GitHub for, and nothing is downloaded or installed for you: you get a link to the download page. Off by default.
      </Row>
      {found && <p role="status" className="mt-2 text-[11px] leading-relaxed text-muted-foreground">{found.available ? `Version ${found.latest} is available. You have ${found.current}.` : found.latest ? `${found.note} (${found.current})` : found.note}</p>}
    </section>
  </>;
}
