/**
 * Moves keyboard focus around the page the way a console menu does, so a controller can reach every button,
 * card and setting without each component needing to know about gamepads. The geometry is pure and unit-tested.
 */
export type Direction = "up" | "down" | "left" | "right";
export interface Box { left: number; top: number; right: number; bottom: number }

const EPS = 1;
/** Index of the best box to move to from `from` in `dir`, or -1 when nothing lies that way. */
export function pickNeighbor(from: Box, boxes: readonly Box[], dir: Direction): number {
  const horizontal = dir === "left" || dir === "right";
  const cx = (b: Box) => (b.left + b.right) / 2, cy = (b: Box) => (b.top + b.bottom) / 2;
  let best = -1, bestScore = Infinity;
  boxes.forEach((box, i) => {
    const dx = cx(box) - cx(from), dy = cy(box) - cy(from);
    const ahead = dir === "right" ? dx > EPS : dir === "left" ? dx < -EPS : dir === "down" ? dy > EPS : dy < -EPS;
    if (!ahead) return;
    const gap = dir === "right" ? box.left - from.right : dir === "left" ? from.left - box.right : dir === "down" ? box.top - from.bottom : from.top - box.bottom;
    const overlap = horizontal ? Math.min(from.bottom, box.bottom) - Math.max(from.top, box.top) : Math.min(from.right, box.right) - Math.max(from.left, box.left);
    const across = Math.abs(horizontal ? dy : dx);
    // Straight ahead beats diagonal: sharing a row or column costs nothing, missing it costs twice the distance.
    const score = Math.max(0, gap) + (overlap > 0 ? 0 : across * 2) + across * 0.01;
    if (score < bestScore - 1e-6) { bestScore = score; best = i; }
  });
  return best;
}

const FOCUSABLE = 'a[href], button, input:not([type="hidden"]), select, textarea, summary, [role="button"], [role="tab"], [role="menuitem"], [role="option"], [role="checkbox"], [role="switch"], [role="radio"], [role="slider"], [tabindex]';
const OVERLAYS = '[role="listbox"], [role="menu"], [role="alertdialog"], [role="dialog"]';
const ALWAYS_REACHABLE = new Set(["tab", "option", "menuitem", "radio"]); // Radix gives most of these tabindex -1

const shown = (el: HTMLElement) => el.getClientRects().length > 0 && getComputedStyle(el).visibility !== "hidden";
function usable(el: HTMLElement): boolean {
  if (el.closest("[data-pad-skip], [data-sonner-toaster], [inert]") || el.matches(":disabled") || el.getAttribute("aria-disabled") === "true") return false;
  if (el.getAttribute("tabindex") === "-1" && !ALWAYS_REACHABLE.has(el.getAttribute("role") ?? "")) return false;
  return shown(el);
}
const box = (el: HTMLElement): Box => { const r = el.getBoundingClientRect(); return { left: r.left, top: r.top, right: r.right, bottom: r.bottom }; };

/** The screen layer that currently owns focus: an open menu, list, or dialog (the last in the DOM is on top), else the page. */
export function topLayer(): HTMLElement {
  const open = Array.from(document.querySelectorAll<HTMLElement>(OVERLAYS)).filter(shown);
  return open[open.length - 1] ?? document.body;
}
export const overlayOpen = () => topLayer() !== document.body;
export const focusables = (layer: HTMLElement) => Array.from(layer.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(usable);

function focusItem(el: HTMLElement) {
  el.focus({ preventScroll: true });
  el.scrollIntoView({ block: "nearest", inline: "nearest" });
}
function focusEntry(items: HTMLElement[]) {
  const onScreen = (el: HTMLElement) => { const r = el.getBoundingClientRect(); return r.bottom > 0 && r.top < innerHeight && r.right > 0 && r.left < innerWidth; };
  focusItem(items.find(i => i.hasAttribute("data-pad-default") && onScreen(i)) ?? items.find(onScreen) ?? items[0]);
}
function current(items: HTMLElement[]): HTMLElement | null {
  const active = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  return active ? items.find(i => i === active || i.contains(active)) ?? null : null;
}
/** Move focus one step. Returns false when there was nothing to move to (the edge of the screen). */
export function moveFocus(direction: Direction): boolean {
  const items = focusables(topLayer());
  if (!items.length) return false;
  const from = current(items);
  if (!from) { focusEntry(items); return true; }
  const others = items.filter(i => i !== from);
  const next = pickNeighbor(box(from), others.map(box), direction);
  if (next < 0) return false;
  focusItem(others[next]); return true;
}
/** Sent (bubbling) from a text field when Cross is pressed on it; `OnScreenKeyboard` listens for it and types into it. */
export const KEYBOARD_EVENT = "aether:keyboard";
const TEXT_TYPES = ["text", "search", "password", "url", "tel"]; // the input types that support a text cursor
/** Fields the on-screen keyboard can type into (`type` is the attribute, so a missing one means text). */
export const isTextEntry = (tag: string, type: string | null) => tag.toLowerCase() === "textarea" || (tag.toLowerCase() === "input" && TEXT_TYPES.includes((type || "text").toLowerCase()));
/** Press the focused control; with nothing focused yet, the first press just picks a starting point. */
export function activate(): boolean {
  const items = focusables(topLayer());
  const from = current(items);
  if (!from) { if (!items.length) return false; focusEntry(items); return true; }
  if ((from instanceof HTMLInputElement || from instanceof HTMLTextAreaElement) && isTextEntry(from.tagName, from.getAttribute("type")) && !from.readOnly) {
    from.dispatchEvent(new CustomEvent(KEYBOARD_EVENT, { bubbles: true })); return true; // the field keeps focus while the keyboard types
  }
  if (from instanceof HTMLInputElement && !["checkbox", "radio", "button", "submit"].includes(from.type)) return true; // a colour or range input keeps focus
  from.click(); return true;
}
/** Close the open dialog or menu the way Escape would. Returns false when nothing is open. */
export function dismiss(): boolean {
  if (!overlayOpen()) return false;
  (document.activeElement ?? document.body).dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true, cancelable: true }));
  return true;
}
/** Switch to the previous or next tab of the open dialog. Radix activates a tab when it takes focus. */
export function switchTab(delta: 1 | -1): boolean {
  const list = topLayer().querySelector<HTMLElement>('[role="tablist"]');
  const tabs = list ? Array.from(list.querySelectorAll<HTMLElement>('[role="tab"]:not([disabled])')) : [];
  if (tabs.length < 2) return false;
  const at = Math.max(0, tabs.findIndex(t => t.getAttribute("aria-selected") === "true"));
  focusItem(tabs[(at + delta + tabs.length) % tabs.length]); return true;
}
