/**
 * The on-screen keyboard's layout and editing rules, modelled on the PS5 console keyboard: a number row, QWERTY, a
 * symbols page, and a bottom row with cursor, space, backspace and Done keys. Pure, so it is unit-tested; the
 * component (`OnScreenKeyboard`) only draws it and writes the result into the focused field.
 */
export type KeyAction = "char" | "shift" | "symbols" | "left" | "right" | "space" | "backspace" | "done";
export interface KeyDef { id: string; label: string; action: KeyAction; char?: string; /** In key widths; every row adds up to 10. */ width: number }
export type Page = "letters" | "symbols";
/** Shift is one-shot after one press, caps lock after a second, and off after a third (as on the console). */
export type Shift = "off" | "once" | "lock";
export interface Pos { row: number; col: number }
export interface Edit { value: string; cursor: number }
export interface KeyboardState { edit: Edit; shift: Shift; page: Page }

const chars = (text: string): KeyDef[] => [...text].map(c => ({ id: c, label: c, action: "char", char: c, width: 1 }));
const key = (action: Exclude<KeyAction, "char">, label: string, width: number): KeyDef => ({ id: action, label, action, width });
export const SHIFT = key("shift", "⇧", 1), SPACE = key("space", "Space", 3), BACKSPACE = key("backspace", "⌫", 1.5), DONE = key("done", "Done", 1.5);
export const LEFT = key("left", "←", 1), RIGHT = key("right", "→", 1), SYMBOLS = key("symbols", "#+=", 2);
const BOTTOM = [SYMBOLS, LEFT, RIGHT, SPACE, BACKSPACE, DONE];
export const LAYOUT: Record<Page, KeyDef[][]> = {
  letters: [chars("1234567890"), chars("qwertyuiop"), chars("asdfghjkl'"), [SHIFT, ...chars("zxcvbnm,.")], BOTTOM],
  // Same shape as the letters page, so the highlighted key stays put when switching pages.
  symbols: [chars("1234567890"), chars("!@#$%^&*()"), chars("-_=+[]{};:"), [SHIFT, ...chars("\\/|~`\"<>?")], BOTTOM],
};
/** Where the highlight starts: the "q" key, like the console. */
export const START: Pos = { row: 1, col: 0 };

export function keyLabel(def: KeyDef, shift: Shift, page: Page): string {
  if (def.action === "symbols") return page === "letters" ? "#+=" : "ABC";
  if (def.action === "shift") return shift === "lock" ? "⇪" : "⇧";
  return def.action === "char" && page === "letters" && shift !== "off" ? def.label.toUpperCase() : def.label;
}

const centre = (row: readonly KeyDef[], col: number) => row.slice(0, col).reduce((x, k) => x + k.width, 0) + row[col].width / 2;
/** One step of the highlight. Left and right wrap around a row; up and down land on the key nearest straight above or below. */
export function moveKey(rows: readonly (readonly KeyDef[])[], pos: Pos, dir: "up" | "down" | "left" | "right"): Pos {
  const row = rows[pos.row];
  if (dir === "left" || dir === "right") return { row: pos.row, col: (pos.col + (dir === "right" ? 1 : -1) + row.length) % row.length };
  const target = pos.row + (dir === "down" ? 1 : -1);
  if (target < 0 || target >= rows.length) return pos;
  const x = centre(row, pos.col);
  let best = 0;
  rows[target].forEach((_, col) => { if (Math.abs(centre(rows[target], col) - x) < Math.abs(centre(rows[target], best) - x) - 1e-9) best = col; });
  return { row: target, col: best };
}

const clamp = (n: number, max: number) => Math.max(0, Math.min(max, n));
/** Inserts at the cursor; a full field (`maxLength`) is left unchanged. */
export function insert(edit: Edit, text: string, maxLength?: number): Edit {
  const at = clamp(edit.cursor, edit.value.length);
  if (maxLength !== undefined && maxLength >= 0 && edit.value.length + text.length > maxLength) return { value: edit.value, cursor: at };
  return { value: edit.value.slice(0, at) + text + edit.value.slice(at), cursor: at + text.length };
}
export function backspace(edit: Edit): Edit {
  const at = clamp(edit.cursor, edit.value.length);
  return at === 0 ? { value: edit.value, cursor: 0 } : { value: edit.value.slice(0, at - 1) + edit.value.slice(at), cursor: at - 1 };
}
export const moveCursor = (edit: Edit, delta: number): Edit => ({ value: edit.value, cursor: clamp(edit.cursor + delta, edit.value.length) });

/** What one key press does. `done` asks the caller to close the keyboard. */
export function applyKey(state: KeyboardState, def: KeyDef, maxLength?: number): KeyboardState & { done: boolean } {
  const { edit, shift, page } = state;
  const same = { edit, shift, page, done: false };
  switch (def.action) {
    case "char": {
      const upper = page === "letters" && shift !== "off";
      return { edit: insert(edit, upper ? def.char!.toUpperCase() : def.char!, maxLength), shift: shift === "once" && page === "letters" ? "off" : shift, page, done: false };
    }
    case "space": return { ...same, edit: insert(edit, " ", maxLength) };
    case "backspace": return { ...same, edit: backspace(edit) };
    case "left": return { ...same, edit: moveCursor(edit, -1) };
    case "right": return { ...same, edit: moveCursor(edit, 1) };
    case "shift": return page === "letters" ? { ...same, shift: shift === "off" ? "once" : shift === "once" ? "lock" : "off" } : same;
    case "symbols": return { ...same, page: page === "letters" ? "symbols" : "letters" };
    case "done": return { ...same, done: true };
  }
}

/** What the keyboard's preview line shows: password fields stay masked. */
export const previewText = (value: string, type: string | null) => (type?.toLowerCase() === "password" ? "•".repeat(value.length) : value);
