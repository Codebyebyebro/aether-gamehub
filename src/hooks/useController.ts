import { useEffect, useRef, useSyncExternalStore } from "react";
import { DEFAULT_RULES, DEFAULT_TIMING, PadMachine, pickPad, toFrame, type PadAction, type PadRules } from "./padCore";
import { playCue, type CueKind } from "@/lib/cue";

export type { PadAction, PadRules } from "./padCore";
export type PadHandlers = Partial<Record<PadAction, () => void>>;
export interface PadStatus { connected: boolean; name: string }

/*
 * One shared poller for the whole window, so several components can use the hook without polling several times.
 * Layers: the most recently mounted enabled `useController` receives input (a carousel or dialog on top of the page
 * takes control, and hands it back when it unmounts). Polling runs only while a controller is connected, a layer
 * exists, and the window is visible and focused, so an idle launcher or a game running on top costs nothing.
 */
type Source = () => PadHandlers;
interface Layer { source: Source; rules: PadRules; cues: boolean }
/**
 * The sound each button makes when the screen uses it. Directions are left to the screens, which play the move click
 * only when the selection really moves (pushing against an edge is silent); Triangle is the screen's to choose too.
 */
const ACTION_CUES: Partial<Record<PadAction, CueKind>> = { confirm: "confirm", back: "back", prevTab: "tab", nextTab: "tab", details: "open", menu: "open", home: "open" };
const layers: Layer[] = [];
/** What a button does when the screen on top has no use for it: Options opens the power menu, PS goes home. */
const fallbacks: PadHandlers = {};
/** Sets (or clears, with null) the app-wide action for a button no screen handles itself. */
export function setPadFallback(action: PadAction, handler: (() => void) | null) { if (handler) fallbacks[action] = handler; else delete fallbacks[action]; }
const machine = new PadMachine(DEFAULT_TIMING);
const listeners = new Set<() => void>();
let status: PadStatus = { connected: false, name: "" };
let frameId = 0;
let removeEvents: (() => void) | null = null;

const pads = () => (typeof navigator.getGamepads === "function" ? Array.from(navigator.getGamepads()) : []);
const currentPad = () => pickPad(pads());
const snapshot = () => { const pad = currentPad(); return pad ? toFrame(pad) : null; };
const awake = () => document.visibilityState === "visible" && document.hasFocus();
function friendlyName(id: string) {
  return /dualsense/i.test(id) ? "DualSense" : /xbox|xinput|045e/i.test(id) ? "Xbox controller" : /054c|dualshock|wireless controller/i.test(id) ? "PlayStation controller" : "Controller";
}
function setStatus(next: PadStatus) {
  if (next.connected === status.connected && next.name === status.name) return;
  status = next; listeners.forEach(l => l());
}
function refreshPads() { const pad = currentPad(); setStatus(pad ? { connected: true, name: friendlyName(pad.id) } : { connected: false, name: "" }); sync(); }

function tick(now: number) {
  frameId = requestAnimationFrame(tick);
  const pad = currentPad();
  if (!pad) { machine.step(now, null); refreshPads(); return; }
  const top = layers[layers.length - 1];
  for (const action of machine.step(now, toFrame(pad), top?.rules)) {
    document.documentElement.dataset.input = "pad"; // lets the stylesheet draw a focus ring while the pad is in use
    const handler = top?.source()[action] ?? fallbacks[action];
    if (!handler) continue;
    const cue = top?.cues === false ? undefined : ACTION_CUES[action];
    if (cue) playCue(cue);
    try { handler(); } catch (error) { console.error("Controller action failed", error); }
  }
}
function sync() {
  const wanted = layers.length > 0 && status.connected && awake();
  if (wanted && !frameId) { machine.suppressHeld(snapshot()); frameId = requestAnimationFrame(tick); }
  else if (!wanted && frameId) { cancelAnimationFrame(frameId); frameId = 0; machine.clear(); }
}
function listen() {
  if (removeEvents) return;
  const onMouse = () => { if (document.documentElement.dataset.input) delete document.documentElement.dataset.input; };
  const events: [EventTarget, string, EventListener, AddEventListenerOptions?][] = [
    [window, "gamepadconnected", refreshPads], [window, "gamepaddisconnected", refreshPads],
    [window, "focus", sync], [window, "blur", sync], [document, "visibilitychange", sync],
    [window, "pointerdown", onMouse, { passive: true }], [window, "pointermove", onMouse, { passive: true }], [window, "keydown", onMouse, { passive: true }],
  ];
  events.forEach(([target, name, handler, options]) => target.addEventListener(name, handler, options));
  removeEvents = () => { events.forEach(([target, name, handler, options]) => target.removeEventListener(name, handler, options)); removeEvents = null; };
  refreshPads();
}
function subscribe(layer: Layer) {
  layers.push(layer); listen();
  machine.suppressHeld(snapshot()); // a button still held from opening this layer must not act on it
  sync();
  return () => {
    const at = layers.lastIndexOf(layer); if (at >= 0) layers.splice(at, 1);
    machine.suppressHeld(snapshot()); // ...or on the layer underneath, after closing this one
    if (!layers.length) { removeEvents?.(); if (frameId) { cancelAnimationFrame(frameId); frameId = 0; } machine.clear(); }
    else sync();
  };
}
const onStatus = (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener); }; };
const getStatus = () => status;

/**
 * PS5 DualSense (and Xbox) navigation. D-pad or left stick move, Cross confirms, Circle goes back, L1/R1 switch
 * category, Square opens details. Returns whether a controller is connected, for on-screen hints. `rules` (fixed for
 * the life of the layer) lets a screen such as the on-screen keyboard take fast taps and held-button repeats.
 */
export function useController(handlers: PadHandlers, options: { enabled?: boolean; rules?: PadRules; cues?: boolean } = {}): PadStatus {
  const latest = useRef(handlers);
  useEffect(() => { latest.current = handlers; });
  const enabled = options.enabled ?? true; const rules = useRef(options.rules ?? DEFAULT_RULES).current; const cues = options.cues ?? true;
  useEffect(() => (enabled ? subscribe({ source: () => latest.current, rules, cues }) : undefined), [enabled, rules, cues]);
  return useSyncExternalStore(onStatus, getStatus, getStatus);
}
