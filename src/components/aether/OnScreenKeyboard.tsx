import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useController, type PadAction, type PadRules } from "@/hooks/useController";
import { KEYBOARD_EVENT } from "@/lib/spatialNav";
import { BACKSPACE, DONE, LAYOUT, LEFT, RIGHT, SHIFT, SPACE, START, applyKey, keyLabel, moveKey, previewText, type Edit, type KeyDef, type Page, type Pos, type Shift } from "@/lib/keyboard";
import PadHints from "@/components/aether/PadHints";

type Field = HTMLInputElement | HTMLTextAreaElement;
/** Sets the value the way typing does, so a React-controlled field's onChange runs. */
function write(field: Field, value: string) {
  const proto = field instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
  Object.getOwnPropertyDescriptor(proto, "value")?.set?.call(field, value);
  field.dispatchEvent(new Event("input", { bubbles: true }));
}
const cursorOf = (field: Field) => { try { return field.selectionStart ?? field.value.length; } catch { return field.value.length; } };
const placeCursor = (field: Field, at: number) => { try { field.setSelectionRange(at, at); } catch { /* the field type has no text cursor */ } };
const nameOf = (field: Field) => field.getAttribute("aria-label") || field.labels?.[0]?.textContent?.trim() || field.placeholder || "Text";
// Typing keys take fast taps (a double letter, quick deletes); holding Square or L1/R1 repeats, as on the console.
const RULES: PadRules = { quick: new Set<PadAction>(["up", "down", "left", "right", "confirm", "details", "secondary", "prevTab", "nextTab", "leftTrigger"]), repeat: new Set<PadAction>(["up", "down", "left", "right", "details", "prevTab", "nextTab"]) };
const HINTS = [["□", "Delete"], ["△", "Space"], ["L1 R1", "Cursor"], ["L2", "Shift"], ["R2", "Done"], ["○", "Close"]] as const;

/**
 * PS5-style on-screen keyboard. Cross on a text field (see `activate` in spatialNav) opens it for that field; it
 * docks at the bottom of the screen and takes the controller until it closes. The field keeps keyboard focus the
 * whole time (keys never take focus), so dialogs keep their focus trap and a physical keyboard still works.
 */
export default function OnScreenKeyboard() {
  const [field, setField] = useState<Field | null>(null);
  const [pos, setPos] = useState<Pos>(START); const [shift, setShift] = useState<Shift>("off"); const [page, setPage] = useState<Page>("letters");
  const [edit, setEdit] = useState<Edit>({ value: "", cursor: 0 });
  const close = () => setField(null);
  useEffect(() => {
    const open = (e: Event) => {
      const target = e.target;
      if (!(target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement)) return;
      setField(target); setPos(START); setShift("off"); setPage("letters"); setEdit({ value: target.value, cursor: cursorOf(target) });
    };
    document.addEventListener(KEYBOARD_EVENT, open);
    return () => document.removeEventListener(KEYBOARD_EVENT, open);
  }, []);
  useEffect(() => {
    if (!field) return;
    // Follow the field: typing on a physical keyboard updates the preview, and the field going away (its dialog
    // closed) or focus moving elsewhere closes the keyboard. Escape closes only the keyboard, not the dialog under it.
    const sync = () => setEdit({ value: field.value, cursor: cursorOf(field) });
    const check = () => { if (!field.isConnected || document.activeElement !== field) setField(null); };
    const escape = (e: KeyboardEvent) => { if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); setField(null); } };
    field.addEventListener("input", sync); field.addEventListener("keyup", sync); field.addEventListener("blur", check);
    window.addEventListener("keydown", escape, true);
    const timer = setInterval(check, 300);
    return () => { field.removeEventListener("input", sync); field.removeEventListener("keyup", sync); field.removeEventListener("blur", check); window.removeEventListener("keydown", escape, true); clearInterval(timer); };
  }, [field]);
  const rows = LAYOUT[page];
  function press(def: KeyDef) {
    if (!field) return;
    if (!field.isConnected) { close(); return; }
    const current = { value: field.value, cursor: cursorOf(field) };
    const next = applyKey({ edit: current, shift, page }, def, field.maxLength >= 0 ? field.maxLength : undefined);
    if (next.edit.value !== current.value) write(field, next.edit.value);
    // A controlled field can refuse or change the text, so read back what it holds now.
    const value = field.value, cursor = Math.min(next.edit.cursor, value.length);
    placeCursor(field, cursor);
    setEdit({ value, cursor }); setShift(next.shift); setPage(next.page);
    if (next.done) close();
  }
  const move = (dir: "up" | "down" | "left" | "right") => () => setPos(p => moveKey(rows, p, dir));
  useController({
    up: move("up"), down: move("down"), left: move("left"), right: move("right"),
    confirm: () => press(rows[pos.row][pos.col]), back: close,
    details: () => press(BACKSPACE), secondary: () => press(SPACE), prevTab: () => press(LEFT), nextTab: () => press(RIGHT),
    leftTrigger: () => press(SHIFT), rightTrigger: () => press(DONE),
  }, { enabled: !!field, rules: RULES });
  if (!field) return null;
  const shown = previewText(edit.value, field.getAttribute("type"));
  const caret = Math.min(edit.cursor, shown.length);
  // Portalled to <body> so it sits above any open dialog. Its own pointer handling keeps a Radix dialog underneath
  // from treating a key press as a click outside (which would close it) or as a reason to move focus.
  return createPortal(
    <div data-pad-skip role="group" aria-label="On-screen keyboard" className="fixed inset-x-0 bottom-0 flex justify-center px-3 pb-4" style={{ zIndex: 60, pointerEvents: "auto" }} onPointerDown={e => e.stopPropagation()} onMouseDown={e => e.preventDefault()}>
      <div className="w-full max-w-[860px] rounded-3xl border bg-card p-4 shadow-2xl">
        <div className="mb-3 rounded-xl border bg-background px-3 py-2">
          <p className="eyebrow mb-0.5">{nameOf(field)}</p>
          <p className="min-h-6 truncate whitespace-pre text-sm" aria-live="polite">{shown.slice(0, caret)}<span className="mx-px inline-block h-4 w-0.5 translate-y-0.5 bg-[var(--aether-accent)]" />{shown.slice(caret)}</p>
        </div>
        <div className="space-y-1.5">{rows.map((row, r) => <div key={r} className="flex gap-1.5">{row.map((def, c) => {
          const on = r === pos.row && c === pos.col;
          const special = def.action !== "char";
          const active = def.action === "shift" && page === "letters" && shift !== "off";
          return <button key={def.id + c} type="button" tabIndex={-1} aria-label={def.action === "char" ? undefined : def.action} data-highlighted={on || undefined}
            disabled={def.action === "shift" && page === "symbols"}
            className={`h-11 min-w-0 rounded-xl text-sm font-semibold transition-colors disabled:opacity-40 ${on ? "bg-[var(--aether-accent)] text-white" : active ? "border-2 border-[var(--aether-accent)] bg-secondary text-[var(--aether-accent)]" : special ? "bg-muted text-muted-foreground" : "bg-secondary"}`}
            style={{ flex: def.width }} onClick={() => { setPos({ row: r, col: c }); press(def); }}>{keyLabel(def, shift, page)}</button>;
        })}</div>)}</div>
        <PadHints items={HINTS} className="mt-3 justify-center text-[10px] text-muted-foreground" />
      </div>
    </div>, document.body);
}
