import test from "node:test";
import assert from "node:assert/strict";
import { BUTTON, DEFAULT_TIMING, PadMachine, pickPad, toFrame, type PadAction, type PadFrame, type PadRules } from "./padCore.ts";

const frame = (down: number[] = [], axes: number[] = [0, 0, 0, 0]): PadFrame => ({ buttons: Array.from({ length: 17 }, (_, i) => down.includes(i)), axes });
const run = (m: PadMachine, steps: [number, PadFrame | null][]) => steps.flatMap(([t, f]) => m.step(t, f));

test("the DualSense buttons from the brief map to the right actions", () => {
  for (const [button, action] of [[BUTTON.dpadUp, "up"], [BUTTON.dpadDown, "down"], [BUTTON.dpadLeft, "left"], [BUTTON.dpadRight, "right"], [BUTTON.cross, "confirm"], [BUTTON.circle, "back"], [BUTTON.l1, "prevTab"], [BUTTON.r1, "nextTab"]] as const) {
    assert.deepEqual(new PadMachine().step(1000, frame([button])), [action], `button ${button}`);
  }
  assert.deepEqual([BUTTON.cross, BUTTON.circle, BUTTON.l1, BUTTON.r1, BUTTON.dpadUp, BUTTON.dpadDown, BUTTON.dpadLeft, BUTTON.dpadRight], [0, 1, 4, 5, 12, 13, 14, 15]);
});
test("Square, Triangle, L2 and R2 map to their actions (the on-screen keyboard's delete, space, Shift and Done)", () => {
  for (const [button, action] of [[BUTTON.square, "details"], [BUTTON.triangle, "secondary"], [BUTTON.l2, "leftTrigger"], [BUTTON.r2, "rightTrigger"]] as const) {
    assert.deepEqual(new PadMachine().step(1000, frame([button])), [action], `button ${button}`);
  }
  assert.deepEqual([BUTTON.square, BUTTON.triangle, BUTTON.l2, BUTTON.r2], [2, 3, 6, 7]);
  const m = new PadMachine(); const pull = (value: number) => toFrame({ buttons: Array.from({ length: 17 }, (_, i) => ({ pressed: false, value: i === BUTTON.r2 ? value : 0 })), axes: [0, 0, 0, 0] });
  assert.deepEqual(m.step(0, pull(0.3)), [], "a light touch on the trigger does nothing");
  assert.deepEqual(m.step(40, pull(0.8)), ["rightTrigger"], "past half travel it fires once");
  assert.deepEqual(m.step(400, pull(1)), [], "holding the trigger does not repeat");
});
test("a press fires once, and holding does not spam a one-shot button", () => {
  const m = new PadMachine();
  const fired = run(m, [[0, frame([BUTTON.cross])], [40, frame([BUTTON.cross])], [500, frame([BUTTON.cross])], [2000, frame([BUTTON.cross])]]);
  assert.deepEqual(fired, ["confirm"]);
});
test("a held direction waits, then repeats at a steady pace, and stops on release", () => {
  const m = new PadMachine(); const right = frame([BUTTON.dpadRight]);
  const times: number[] = [];
  for (let t = 0; t <= 1000; t += 33) if (m.step(t, right).length) times.push(t);
  assert.equal(times[0], 0);
  assert.ok(times[1] >= DEFAULT_TIMING.repeatDelayMs && times[1] < DEFAULT_TIMING.repeatDelayMs + 40, `first repeat at ${times[1]}`);
  const gaps = times.slice(2).map((t, i) => t - times[i + 1]);
  assert.ok(gaps.length >= 3 && gaps.every(g => g >= DEFAULT_TIMING.repeatMs - 1 && g <= DEFAULT_TIMING.repeatMs + 40), `gaps ${gaps}`);
  assert.deepEqual(m.step(1100, frame()), []);
  assert.deepEqual(m.step(1300, frame([BUTTON.dpadRight])), ["right"], "a fresh press fires immediately again");
});
test("frames faster than the poll interval are ignored", () => {
  const m = new PadMachine();
  assert.deepEqual(m.step(0, frame()), []);
  assert.deepEqual(m.step(10, frame([BUTTON.cross])), [], "too soon: skipped, not queued");
  assert.deepEqual(m.step(40, frame([BUTTON.cross])), ["confirm"]);
});
test("double taps cannot launch or close twice, but quick direction taps still work", () => {
  const m = new PadMachine(); const tap = (t: number, button: number) => [m.step(t, frame([button])), m.step(t + 40, frame())].flat();
  assert.deepEqual([...tap(0, BUTTON.cross), ...tap(100, BUTTON.cross)], ["confirm"], "second tap inside the cooldown is dropped");
  assert.deepEqual(tap(1000, BUTTON.cross), ["confirm"]);
  assert.deepEqual([...tap(2000, BUTTON.dpadDown), ...tap(2130, BUTTON.dpadDown), ...tap(2260, BUTTON.dpadDown)], ["down", "down", "down"]);
});
test("a stall while a direction is held produces one step, not a burst", () => {
  const m = new PadMachine(); const down = frame([BUTTON.dpadDown]);
  assert.deepEqual(m.step(0, down), ["down"]);
  assert.deepEqual(m.step(5000, down), ["down"]);
  assert.deepEqual(m.step(5040, down), []);
});
test("the left stick navigates with hysteresis and the D-pad takes priority", () => {
  const m = new PadMachine(); const stick = (x: number, y: number) => frame([], [x, y, 0, 0]);
  assert.deepEqual(m.step(0, stick(0.5, 0)), [], "below the entry threshold");
  assert.deepEqual(m.step(40, stick(0.7, 0.1)), ["right"]);
  assert.deepEqual(m.step(80, stick(0.45, 0)), [], "still held: no new press while above the exit threshold");
  assert.deepEqual(m.step(120, stick(0.3, 0)), []);
  assert.deepEqual(m.step(160, stick(0.5, 0)), [], "released below the exit threshold, so 0.5 is not enough to re-enter");
  assert.deepEqual(m.step(200, stick(0, -0.9)), ["up"], "dominant axis wins");
  assert.deepEqual(new PadMachine().step(0, frame([BUTTON.dpadLeft], [0.9, 0, 0, 0])), ["left"]);
});
test("a button that was already down when a new layer opened is ignored until released", () => {
  const m = new PadMachine(); const cross = frame([BUTTON.cross]);
  m.suppressHeld(cross);
  assert.deepEqual(run(m, [[0, cross], [100, cross], [400, cross]]), []);
  assert.deepEqual(run(m, [[500, frame()], [800, cross]]), ["confirm"]);
});
test("losing the controller clears everything", () => {
  const m = new PadMachine();
  assert.deepEqual(m.step(0, frame([BUTTON.dpadUp])), ["up"]);
  assert.deepEqual(m.step(100, null), []);
  assert.deepEqual(m.step(200, frame([BUTTON.dpadUp])), ["up"], "pressed again after reconnecting");
});
test("triggers count as pressed past half travel and the right pad is chosen", () => {
  assert.deepEqual(toFrame({ buttons: [{ pressed: false, value: 0.6 }, { pressed: false, value: 0.2 }, { pressed: true, value: 1 }], axes: [0.1] }).buttons, [true, false, true]);
  const pads = [null, { connected: true, mapping: "", id: "Generic Pad" }, { connected: true, mapping: "standard", id: "Xbox 360 Controller" }, { connected: true, mapping: "standard", id: "DualSense Wireless Controller (STANDARD GAMEPAD Vendor: 054c Product: 0ce6)" }];
  assert.equal(pickPad(pads)?.id.startsWith("DualSense"), true);
  assert.equal(pickPad(pads.slice(0, 3))?.id, "Xbox 360 Controller");
  assert.equal(pickPad([{ connected: true, mapping: "", id: "x" }]), null);
  assert.equal(pickPad([null, null]), null);
});
test("a screen can make buttons quick and repeating (the on-screen keyboard); the default rules stay strict", () => {
  const keyboard: PadRules = { quick: new Set<PadAction>(["up", "down", "left", "right", "confirm", "details"]), repeat: new Set<PadAction>(["up", "down", "left", "right", "details"]) };
  const tapTwice = (rules?: PadRules) => { const m = new PadMachine(); return [m.step(0, frame([BUTTON.cross]), rules), m.step(40, frame(), rules), m.step(100, frame([BUTTON.cross]), rules)].flat(); };
  assert.deepEqual(tapTwice(), ["confirm"], "menus: a double tap of Cross is still one press");
  assert.deepEqual(tapTwice(keyboard), ["confirm", "confirm"], "keyboard: a double letter types twice");
  const hold = (rules?: PadRules) => { const m = new PadMachine(); const square = frame([BUTTON.square]); let n = 0; for (let t = 0; t <= 1000; t += 33) n += m.step(t, square, rules).length; return n; };
  assert.equal(hold(), 1, "menus: holding Square opens details once");
  assert.ok(hold(keyboard) >= 5, `keyboard: holding Square keeps deleting (${hold(keyboard)} in 1 s)`);
  const m = new PadMachine(); const circle = frame([BUTTON.circle]);
  assert.deepEqual([m.step(0, circle, keyboard), m.step(40, frame(), keyboard), m.step(100, circle, keyboard)].flat(), ["back"], "buttons a screen leaves out keep the cooldown");
});
