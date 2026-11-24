import { useEffect, useRef, useState } from "react";
import { Gamepad2, Info } from "lucide-react";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import type { Controls } from "@/lib/aether";
import { KEY_CHOICES, KEY_COLUMNS, LAYOUT_CHOICES, PAD_BUTTONS, PRESETS, STICK_MODES, assign, choiceOf, choose, editorRows, gridMove, keyLabel, layoutName, sendable, withSpeed, withStick } from "@/lib/controls";
import { useController } from "@/hooks/useController";
import PadHints from "@/components/aether/PadHints";
import { playCue } from "@/lib/cue";

const stickLabel = (id: string) => STICK_MODES.find(m => m.id === id)?.label ?? "Nothing";
const choiceLabel = (id: string) => id === "off" ? "Off" : id === "custom" ? "Custom" : PRESETS.find(p => p.id === id)!.name;

/**
 * Picks and edits a game's controller layout. With the controller: ▲ ▼ choose a row, ◀ ▶ change it, ✕ on a button
 * opens the key grid, Options saves, ○ goes back. With the mouse: click. With the keyboard: while a button waits for
 * its key, the next key pressed is the one it gets.
 */
export default function ControlsEditor({ title, controls, enabled, save, close }: { title: string; controls: Controls | null | undefined; enabled: boolean; save: (c: Controls | null) => void; close: () => void }) {
  const [draft, setDraft] = useState<Controls | null>(controls ?? null);
  const [row, setRow] = useState(0);
  const [picking, setPicking] = useState<string | null>(null);
  const [keyAt, setKeyAt] = useState(0);
  const panel = useRef<HTMLDivElement>(null);
  const rows = editorRows(draft);
  const at = rows[Math.min(row, rows.length - 1)];

  const moveRow = (step: number) => { const next = Math.max(0, Math.min(rows.length - 1, row + step)); if (next !== row) { setRow(next); playCue("move"); } };
  const change = (step: number, id = at) => {
    if (!draft || id === "layout") { const i = LAYOUT_CHOICES.indexOf(choiceOf(draft) as typeof LAYOUT_CHOICES[number]); setDraft(choose(LAYOUT_CHOICES[(i + step + LAYOUT_CHOICES.length) % LAYOUT_CHOICES.length], draft)); }
    else if (id === "leftStick" || id === "rightStick") setDraft(withStick(draft, id, step));
    else if (id === "speed") setDraft(withSpeed(draft, step));
    else return;
    playCue("move");
  };
  const startPicking = (button: string) => { setPicking(button); setKeyAt(Math.max(0, KEY_CHOICES.indexOf(draft?.buttons[button] ?? ""))); };
  const pick = (code: string) => { if (draft && picking) setDraft(assign(draft, picking, code)); setPicking(null); };
  const confirm = () => {
    if (picking) { pick(KEY_CHOICES[keyAt]); return; }
    if (at === "save") save(draft); else if (at === "layout" || at === "leftStick" || at === "rightStick") change(1); else if (at !== "speed") startPicking(at);
  };
  const moveKey = (dir: "up" | "down" | "left" | "right") => { const next = gridMove(keyAt, dir, KEY_COLUMNS, KEY_CHOICES.length); if (next !== keyAt) { setKeyAt(next); playCue("move"); } };

  const pad = useController({
    up: () => picking ? moveKey("up") : moveRow(-1), down: () => picking ? moveKey("down") : moveRow(1),
    left: () => picking ? moveKey("left") : change(-1), right: () => picking ? moveKey("right") : change(1),
    confirm, back: () => picking ? setPicking(null) : close(), menu: () => { if (!picking) save(draft); }, home: close,
  });
  // The focused control follows the controller, so the ring shows and Enter or a screen reader agree with it.
  useEffect(() => { panel.current?.querySelector<HTMLElement>(picking ? `[data-key="${keyAt}"]` : `[data-row="${at}"]`)?.focus({ preventScroll: false }); }, [at, picking, keyAt]);
  // Captured on the window, so keys here never also reach Big Box or the dialog underneath.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (picking) {
        // Every key is a choice here, Esc and Tab included; ○ on the controller or Cancel go back.
        e.preventDefault(); e.stopImmediatePropagation();
        if (!e.repeat && sendable(e.code)) pick(e.code);
        return;
      }
      if (!["Escape", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Enter"].includes(e.key)) return;
      e.preventDefault(); e.stopImmediatePropagation();
      if (e.repeat && e.key === "Enter") return;
      if (e.key === "Escape") close(); else if (e.key === "ArrowUp") moveRow(-1); else if (e.key === "ArrowDown") moveRow(1);
      else if (e.key === "ArrowLeft") change(-1); else if (e.key === "ArrowRight") change(1); else confirm();
    };
    window.addEventListener("keydown", key, { capture: true });
    return () => window.removeEventListener("keydown", key, { capture: true });
  });

  const rowClass = (id: string) => `flex w-full items-center justify-between gap-3 rounded-xl px-3 py-2 text-left text-xs ${at === id ? "bg-secondary ring-2 ring-[var(--aether-accent)]" : "hover:bg-muted"}`;
  const stepper = (id: string, value: string) => <div className={rowClass(id)}>
    <button type="button" data-row={id} onFocus={() => setRow(rows.indexOf(id))} onClick={() => change(1, id)} className="flex-1 text-left font-medium outline-none">{id === "layout" ? "Layout" : id === "leftStick" ? "Left stick" : id === "rightStick" ? "Right stick" : "Mouse speed"}</button>
    <span className="flex items-center gap-1"><button type="button" data-pad-skip aria-label="Previous" onClick={() => { setRow(rows.indexOf(id)); change(-1, id); }} className="h-7 w-7 rounded-lg hover:bg-background">◀</button><span className="min-w-[150px] text-center font-semibold">{value}</span><button type="button" data-pad-skip aria-label="Next" onClick={() => { setRow(rows.indexOf(id)); change(1, id); }} className="h-7 w-7 rounded-lg hover:bg-background">▶</button></span>
  </div>;

  return <Dialog open onOpenChange={open => !open && close()}>
    <DialogContent className="max-h-[90vh] max-w-[620px] overflow-y-auto rounded-3xl bg-card p-7" onEscapeKeyDown={e => e.preventDefault()}>
      <DialogHeader><DialogTitle className="display-font flex items-center gap-2 text-2xl font-extrabold"><Gamepad2 size={22} />Controller layout</DialogTitle><DialogDescription className="text-xs">For {title}: your controller types keys and moves the mouse while this game is in front.</DialogDescription></DialogHeader>
      <div ref={panel} className="mt-2 space-y-4">
        {!enabled && <p className="flex gap-2 rounded-xl bg-secondary p-3 text-[11px] leading-relaxed"><Info size={14} className="mt-0.5 shrink-0" />Switch on “Controller layouts” in Settings, under “Running in the background”, for layouts to work. You can choose one now.</p>}
        {!picking ? <>
          <div className="space-y-1">
            {stepper("layout", choiceLabel(choiceOf(draft)))}
            {draft && <p className="px-3 text-[11px] leading-relaxed text-muted-foreground">{PRESETS.find(p => p.id === draft.preset)?.note ?? "Your own layout. Choose a button to give it a key."}</p>}
          </div>
          {draft && <>
            <div className="space-y-1">{stepper("leftStick", stickLabel(draft.leftStick))}{stepper("rightStick", stickLabel(draft.rightStick))}{stepper("speed", `${draft.mouseSpeed || 5} of 10`)}</div>
            <div className="grid gap-1 sm:grid-cols-2">{PAD_BUTTONS.map(b => <button type="button" key={b.id} data-row={b.id} onFocus={() => setRow(rows.indexOf(b.id))} onClick={() => { setRow(rows.indexOf(b.id)); startPicking(b.id); }} className={rowClass(b.id)}>
              <span className="font-medium">{b.label}</span><span className={draft.buttons[b.id] ? "font-semibold" : "text-muted-foreground"}>{keyLabel(draft.buttons[b.id])}</span>
            </button>)}</div>
          </>}
          <p className="text-[11px] leading-relaxed text-muted-foreground">The PS / Xbox button always opens Aether Hub’s quick menu and never sends a key. Use a layout only for games that ignore your controller; a game that already supports it would get both. Some games only work with Xbox controllers: those need a virtual Xbox controller, which needs a driver, and Aether Hub does not install drivers. For those, use Steam Input (in Steam: Games, “Add a Non-Steam Game to My Library”, then turn on PlayStation controller support) or DS4Windows. Games with anti-cheat may ignore keys sent by another program.</p>
          <div className="flex items-center justify-end gap-2 border-t pt-4">
            {pad.connected && <PadHints className="mr-auto text-[11px] text-muted-foreground" items={[["▲ ▼", "Choose"], ["◀ ▶", "Change"], ["✕", "Set key"], ["Options", "Save"], ["○", "Back"]]} />}
            <Button type="button" variant="ghost" className="rounded-xl" onClick={close}>Cancel</Button>
            <Button type="button" data-row="save" onFocus={() => setRow(rows.length - 1)} className={`aether-primary px-6 ${at === "save" ? "ring-2 ring-[var(--aether-accent)] ring-offset-2" : ""}`} onClick={() => save(draft)}>Save layout ({layoutName(draft)})</Button>
          </div>
        </> : <div role="group" aria-label={`Key for ${PAD_BUTTONS.find(b => b.id === picking)?.label}`}>
          <p className="text-sm font-semibold">Press a key for {PAD_BUTTONS.find(b => b.id === picking)?.label}</p>
          <p className="mt-1 text-[11px] text-muted-foreground">Press it on your keyboard, or choose it below.</p>
          <div className="mt-3 grid gap-1.5" style={{ gridTemplateColumns: `repeat(${KEY_COLUMNS}, minmax(0, 1fr))` }}>{KEY_CHOICES.map((code, i) => <button type="button" key={code || "none"} data-key={i} onClick={() => pick(code)} onMouseEnter={() => setKeyAt(i)}
            className={`h-11 rounded-lg border px-1 text-[11px] font-semibold ${i === keyAt ? "border-[var(--aether-accent)] bg-[var(--aether-accent)] text-white" : "hover:bg-muted"}`}>{keyLabel(code)}</button>)}</div>
          <div className="mt-4 flex items-center justify-end gap-2 border-t pt-4">
            {pad.connected && <PadHints className="mr-auto text-[11px] text-muted-foreground" items={[["✛", "Move"], ["✕", "Choose"], ["○", "Back"]]} />}
            <Button type="button" variant="ghost" className="rounded-xl" onClick={() => setPicking(null)}>Cancel</Button>
          </div>
        </div>}
      </div>
    </DialogContent>
  </Dialog>;
}
