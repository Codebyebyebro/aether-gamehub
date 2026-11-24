/** Controller layouts: the presets, how keys are named on screen, and the layout editor's rules. Kept pure, so tested. */
import type { Controls } from "./aether.ts";

/** The buttons a layout can use, in the editor's order. The PS button is not here: it always opens the quick menu. */
export const PAD_BUTTONS = [
  { id: "cross", label: "✕" }, { id: "circle", label: "○" }, { id: "square", label: "□" }, { id: "triangle", label: "△" },
  { id: "l1", label: "L1" }, { id: "r1", label: "R1" }, { id: "l2", label: "L2" }, { id: "r2", label: "R2" },
  { id: "l3", label: "L3 (press left stick)" }, { id: "r3", label: "R3 (press right stick)" },
  { id: "up", label: "D-pad ▲" }, { id: "down", label: "D-pad ▼" }, { id: "left", label: "D-pad ◀" }, { id: "right", label: "D-pad ▶" },
  { id: "options", label: "Options" }, { id: "create", label: "Create (Share)" },
] as const;
export const STICK_MODES = [
  { id: "wasd", label: "W A S D keys" }, { id: "arrows", label: "Arrow keys" }, { id: "mouse", label: "Mouse" },
  { id: "scroll", label: "Scroll wheel" }, { id: "none", label: "Nothing" },
] as const;

const dpad = (up: string, right: string, down: string, left: string) => ({ up, right, down, left });
export const PRESETS: readonly { id: string; name: string; note: string; controls: Controls }[] = [
  { id: "shooter", name: "Mouse & keyboard shooter", note: "Left stick walks (W A S D), right stick looks around, R2 shoots, L2 aims.", controls: {
    preset: "shooter", leftStick: "wasd", rightStick: "mouse", mouseSpeed: 5,
    buttons: { r2: "MouseLeft", l2: "MouseRight", cross: "Space", circle: "ControlLeft", square: "KeyR", triangle: "KeyE", l1: "KeyQ", r1: "KeyF", options: "Escape", ...dpad("Digit1", "Digit2", "Digit3", "Digit4") },
  } },
  { id: "pointer", name: "Point and click", note: "Left stick moves the pointer, ✕ clicks, ○ right-clicks, right stick scrolls.", controls: {
    preset: "pointer", leftStick: "mouse", rightStick: "scroll", mouseSpeed: 4,
    buttons: { cross: "MouseLeft", circle: "MouseRight", r2: "MouseLeft", l2: "MouseRight", triangle: "Space", options: "Escape", ...dpad("ArrowUp", "ArrowRight", "ArrowDown", "ArrowLeft") },
  } },
  { id: "platformer", name: "Platformer", note: "Left stick and D-pad are the arrow keys (or W A S D), ✕ is Space, □ is Shift.", controls: {
    preset: "platformer", leftStick: "arrows", rightStick: "none", mouseSpeed: 5,
    buttons: { cross: "Space", square: "ShiftLeft", options: "Escape", ...dpad("ArrowUp", "ArrowRight", "ArrowDown", "ArrowLeft") },
  } },
];
/** The choices at the top of the editor, in order. */
export const LAYOUT_CHOICES = ["off", ...PRESETS.map(p => p.id), "custom"] as const;

/** A fresh copy of a preset; "custom" starts from what the player has, or from the shooter layout. */
export function choose(choice: string, current: Controls | null | undefined): Controls | null {
  if (choice === "off") return null;
  const preset = PRESETS.find(p => p.id === choice);
  if (preset) return { ...preset.controls, buttons: { ...preset.controls.buttons } };
  const base = current ?? PRESETS[0].controls;
  return { ...base, buttons: { ...base.buttons }, preset: "custom" };
}
export function choiceOf(c: Controls | null | undefined): string { return !c ? "off" : PRESETS.some(p => p.id === c.preset) ? c.preset : "custom"; }
export function layoutName(c: Controls | null | undefined): string {
  const id = choiceOf(c);
  return id === "off" ? "Off" : id === "custom" ? "Custom" : PRESETS.find(p => p.id === id)!.name;
}

/** Changing anything makes the layout the player's own. `code` "" leaves the button unused. */
export function assign(c: Controls, button: string, code: string): Controls {
  const buttons = { ...c.buttons };
  if (code) buttons[button] = code; else delete buttons[button];
  return { ...c, buttons, preset: "custom" };
}
export function withStick(c: Controls, which: "leftStick" | "rightStick", step: number): Controls {
  const ids = STICK_MODES.map(m => m.id as string);
  const at = Math.max(0, ids.indexOf(c[which]));
  return { ...c, [which]: ids[(at + step + ids.length) % ids.length], preset: "custom" };
}
export function withSpeed(c: Controls, step: number): Controls { return { ...c, mouseSpeed: Math.max(1, Math.min(10, (c.mouseSpeed || 5) + step)), preset: "custom" }; }

/** Keys the program can send: browser `KeyboardEvent.code` values it knows, and the mouse. Must match `vk` in mapper.rs. */
const SENDABLE = /^(Key[A-Z]|Digit[0-9]|Numpad[0-9]|F([1-9]|1[0-2])|Space|Enter|Escape|Tab|Backspace|CapsLock|(Shift|Control|Alt)(Left|Right)|Arrow(Up|Down|Left|Right)|Insert|Delete|Home|End|PageUp|PageDown|Minus|Equal|BracketLeft|BracketRight|Backslash|Semicolon|Quote|Backquote|Comma|Period|Slash|Mouse(Left|Right|Middle)|Wheel(Up|Down))$/;
export const sendable = (code: string) => SENDABLE.test(code);
/** The on-screen key choices, for picking a key with the controller (or the mouse). "" is "Nothing". */
export const KEY_CHOICES: readonly string[] = [
  "MouseLeft", "MouseRight", "MouseMiddle", "WheelUp", "WheelDown", "Space", "ShiftLeft", "ControlLeft",
  "AltLeft", "Tab", "Enter", "Escape", "KeyE", "KeyF", "KeyQ", "KeyR",
  "KeyW", "KeyA", "KeyS", "KeyD", "KeyC", "KeyV", "KeyX", "KeyZ",
  "KeyI", "KeyM", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "F5",
  "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Backspace", "F9", "",
];
export const KEY_COLUMNS = 8;
const NAMES: Record<string, string> = {
  "": "Nothing", MouseLeft: "Left click", MouseRight: "Right click", MouseMiddle: "Middle click", WheelUp: "Scroll up", WheelDown: "Scroll down",
  ShiftLeft: "Shift", ShiftRight: "Right Shift", ControlLeft: "Ctrl", ControlRight: "Right Ctrl", AltLeft: "Alt", AltRight: "Right Alt",
  Escape: "Esc", ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→", CapsLock: "Caps Lock", PageUp: "Page Up", PageDown: "Page Down",
  Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]", Backslash: "\\", Semicolon: ";", Quote: "'", Backquote: "`", Comma: ",", Period: ".", Slash: "/",
};
export function keyLabel(code: string | undefined): string {
  const c = code ?? "";
  return NAMES[c] ?? c.replace(/^Key/, "").replace(/^Digit/, "").replace(/^Numpad/, "Num ");
}

/** One step in the key grid: left and right along a row, up and down a column; edges stop. */
export function gridMove(index: number, dir: "up" | "down" | "left" | "right", columns: number, count: number): number {
  const next = dir === "left" ? (index % columns ? index - 1 : index) : dir === "right" ? ((index + 1) % columns && index + 1 < count ? index + 1 : index)
    : dir === "up" ? index - columns : index + columns;
  return next >= 0 && next < count ? next : index;
}
/** The editor's rows, top to bottom: the layout choice, then (unless Off) the sticks, mouse speed and every button. */
export function editorRows(c: Controls | null): string[] {
  return c ? ["layout", "leftStick", "rightStick", "speed", ...PAD_BUTTONS.map(b => b.id), "save"] : ["layout", "save"];
}
