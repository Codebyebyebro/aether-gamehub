/**
 * Pure gamepad logic: turns raw button/axis snapshots into debounced, repeating menu actions.
 * It has no DOM and no timers, so it is fully unit-tested (see padCore.test.ts); `useController` feeds it frames.
 */
/** `secondary` is Triangle; `leftTrigger`/`rightTrigger` are L2/R2 (the on-screen keyboard uses them as space, Shift and Done). */
export type PadAction = "up" | "down" | "left" | "right" | "confirm" | "back" | "prevTab" | "nextTab" | "details" | "secondary" | "leftTrigger" | "rightTrigger";
export interface PadFrame { buttons: readonly boolean[]; axes: readonly number[] }

/**
 * Standard-mapping indices. WebView2 (Chromium) reports a PS5 DualSense, and Xbox pads, with this layout:
 * Cross = 0, Circle = 1, Square = 2, Triangle = 3, L1 = 4, R1 = 5, L2 = 6, R2 = 7 (analog), D-pad = 12 to 15.
 */
export const BUTTON = { cross: 0, circle: 1, square: 2, triangle: 3, l1: 4, r1: 5, l2: 6, r2: 7, dpadUp: 12, dpadDown: 13, dpadLeft: 14, dpadRight: 15 } as const;

export interface PadTiming {
  /** Frames closer together than this are skipped, so a 144 Hz display does not poll 144 times a second. */
  pollMs: number;
  /** A held direction first repeats after this long, then every `repeatMs`. */
  repeatDelayMs: number;
  repeatMs: number;
  /** Contact bounce filter for directions (shorter than any deliberate double tap). */
  bounceMs: number;
  /** Minimum gap between two presses of the same one-shot button, so a double tap cannot launch or close twice. */
  cooldownMs: number;
  /** Left-stick thresholds: push past `stickEnter` to start, stay active until it falls below `stickExit`. */
  stickEnter: number;
  stickExit: number;
}
export const DEFAULT_TIMING: PadTiming = { pollMs: 33, repeatDelayMs: 400, repeatMs: 120, bounceMs: 60, cooldownMs: 220, stickEnter: 0.6, stickExit: 0.4 };

const DIRECTIONS: readonly PadAction[] = ["up", "down", "left", "right"];
/**
 * How a screen wants presses handled. `quick` actions only get the short bounce filter (other buttons get the longer
 * cooldown, so a double tap cannot launch or close twice); `repeat` actions fire again while held. Menus use the
 * default (directions only). The on-screen keyboard makes typing keys quick and lets Square and L1/R1 repeat.
 */
export interface PadRules { quick: ReadonlySet<PadAction>; repeat: ReadonlySet<PadAction> }
export const DEFAULT_RULES: PadRules = { quick: new Set(DIRECTIONS), repeat: new Set(DIRECTIONS) };
const BUTTON_ACTIONS: readonly (readonly [number, PadAction])[] = [
  [BUTTON.dpadUp, "up"], [BUTTON.dpadDown, "down"], [BUTTON.dpadLeft, "left"], [BUTTON.dpadRight, "right"],
  [BUTTON.cross, "confirm"], [BUTTON.circle, "back"], [BUTTON.l1, "prevTab"], [BUTTON.r1, "nextTab"], [BUTTON.square, "details"],
  [BUTTON.triangle, "secondary"], [BUTTON.l2, "leftTrigger"], [BUTTON.r2, "rightTrigger"],
];

export class PadMachine {
  private held = new Set<PadAction>();
  private suppressed = new Set<PadAction>();
  private nextRepeat = new Map<PadAction, number>();
  private lastFired = new Map<PadAction, number>();
  private lastStep = -Infinity;
  private stick = false;
  private readonly timing: PadTiming;
  constructor(timing: PadTiming = DEFAULT_TIMING) { this.timing = timing; }

  /** Which actions the frame is holding down right now (D-pad wins over the stick). */
  private read(frame: PadFrame): Set<PadAction> {
    const down = new Set<PadAction>();
    for (const [index, action] of BUTTON_ACTIONS) if (frame.buttons[index]) down.add(action);
    let stick = false;
    if (!DIRECTIONS.some(d => down.has(d))) {
      const x = frame.axes[0] ?? 0, y = frame.axes[1] ?? 0;
      if (Math.max(Math.abs(x), Math.abs(y)) >= (this.stick ? this.timing.stickExit : this.timing.stickEnter)) {
        down.add(Math.abs(x) > Math.abs(y) ? (x > 0 ? "right" : "left") : (y > 0 ? "down" : "up")); stick = true;
      }
    }
    this.stick = stick;
    return down;
  }
  private allowed(action: PadAction, now: number, rules: PadRules) {
    const last = this.lastFired.get(action);
    return last === undefined || now - last >= (rules.quick.has(action) ? this.timing.bounceMs : this.timing.cooldownMs);
  }
  /** The actions to deliver at time `now` (milliseconds). `frame` is null when no controller is available. */
  step(now: number, frame: PadFrame | null, rules: PadRules = DEFAULT_RULES): PadAction[] {
    if (now - this.lastStep < this.timing.pollMs) return [];
    this.lastStep = now;
    const next = frame ? this.read(frame) : new Set<PadAction>();
    const fired: PadAction[] = [];
    for (const action of next) {
      if (this.suppressed.has(action)) continue;
      if (!this.held.has(action)) {
        if (this.allowed(action, now, rules)) { fired.push(action); this.lastFired.set(action, now); }
        if (rules.repeat.has(action)) this.nextRepeat.set(action, now + this.timing.repeatDelayMs);
      } else if (rules.repeat.has(action) && now >= (this.nextRepeat.get(action) ?? Infinity)) {
        // Scheduled from `now`, not from the previous deadline: after a stall this is one step, never a burst.
        fired.push(action); this.lastFired.set(action, now); this.nextRepeat.set(action, now + this.timing.repeatMs);
      }
    }
    for (const action of [...this.suppressed]) if (!next.has(action)) this.suppressed.delete(action);
    for (const action of [...this.nextRepeat.keys()]) if (!next.has(action)) this.nextRepeat.delete(action);
    this.held = next;
    return fired;
  }
  /** Ignore everything that is down right now until it is released (a new layer opened, or the window regained focus). */
  suppressHeld(frame: PadFrame | null) {
    this.suppressed = frame ? this.read(frame) : new Set<PadAction>();
    this.held = new Set(this.suppressed); this.nextRepeat.clear();
  }
  clear() { this.held.clear(); this.suppressed.clear(); this.nextRepeat.clear(); this.lastFired.clear(); this.lastStep = -Infinity; this.stick = false; }
}

/** Builds a frame from a browser Gamepad (a trigger counts as pressed past half travel). */
export function toFrame(pad: { buttons: readonly { pressed: boolean; value: number }[]; axes: readonly number[] }): PadFrame {
  return { buttons: pad.buttons.map(b => b.pressed || b.value > 0.5), axes: pad.axes };
}
/** Prefer a controller that reports the standard layout; DualSense/DualShock and Xbox pads all do. */
export function pickPad<T extends { connected: boolean; mapping: string; id: string }>(pads: readonly (T | null)[]): T | null {
  const live = pads.filter((p): p is T => !!p && p.connected);
  return live.find(p => p.mapping === "standard" && /dualsense|wireless controller|054c/i.test(p.id)) ?? live.find(p => p.mapping === "standard") ?? null;
}
