import test from "node:test";
import assert from "node:assert/strict";
import { POWER_ITEMS, nativeAction, quickItems, stepIndex } from "./power.ts";
import { LAUNCH_RUMBLE, rumble, rumbleParams } from "./rumble.ts";

test("the power menu offers the five choices in console order", () => {
  assert.deepEqual(POWER_ITEMS.map(i => i.id), ["desktop", "quit", "sleep", "restart", "shutdown"]);
});
test("only sleep, restart and shut down ask first, and only they reach the program", () => {
  for (const item of POWER_ITEMS) {
    const pc = item.id === "sleep" || item.id === "restart" || item.id === "shutdown";
    assert.equal(!!item.confirm, pc, item.id);
    assert.equal(nativeAction(item.id), pc ? item.id : null, item.id);
  }
  assert.match(POWER_ITEMS.find(i => i.id === "shutdown")!.confirm!.body, /unsaved/i, "the warning says what can be lost");
});
test("moving through the menu wraps around both ways", () => {
  assert.equal(stepIndex(0, -1, 5), 4);
  assert.equal(stepIndex(4, 1, 5), 0);
  assert.equal(stepIndex(2, 1, 5), 3);
  assert.equal(stepIndex(0, 1, 0), 0);
});

test("rumble strength and length stay inside what controllers accept", () => {
  assert.deepEqual(rumbleParams({ duration: 220, strong: 0.7, weak: 0.45 }), { duration: 220, startDelay: 0, strongMagnitude: 0.7, weakMagnitude: 0.45 });
  assert.deepEqual(rumbleParams({ duration: 99_999, strong: 3, weak: -1 }), { duration: 5000, startDelay: 0, strongMagnitude: 1, weakMagnitude: 0 });
  assert.deepEqual(rumbleParams({ duration: NaN, strong: NaN, weak: 0.2 }), { duration: 0, startDelay: 0, strongMagnitude: 0, weakMagnitude: 0.2 });
});
test("every connected controller that can rumble is asked once; others are skipped without errors", () => {
  const calls: [string, Record<string, number>][] = [];
  const able = { connected: true, vibrationActuator: { playEffect: (type: string, p: Record<string, number>) => { calls.push([type, p]); return Promise.resolve("complete"); } } };
  const refusing = { connected: true, vibrationActuator: { playEffect: () => { throw new Error("NotSupported"); } } };
  const rejecting = { connected: true, vibrationActuator: { playEffect: () => Promise.reject(new Error("busy")) } };
  const unplugged = { connected: false, vibrationActuator: able.vibrationActuator };
  const plain = { connected: true };
  assert.equal(rumble([able, refusing, rejecting, unplugged, plain, null]), 2, "the refusing one throws, the rejecting one is still asked");
  assert.equal(calls.length, 1);
  assert.equal(calls[0][0], "dual-rumble");
  assert.equal(calls[0][1].duration, LAUNCH_RUMBLE.duration);
  assert.equal(rumble([able], { duration: 0, strong: 1, weak: 1 }), 0, "a zero-length rumble is not sent");
  assert.equal(rumble([]), 0);
});
test("the quick menu resumes first, and only closing the game asks before acting", () => {
  const items = quickItems("Hades");
  assert.deepEqual(items.map(i => i.id), ["resume", "home", "close", "power"]);
  assert.deepEqual(items.filter(i => i.confirm).map(i => i.id), ["close"]);
  assert.equal(items.find(i => i.id === "close")!.confirm!.title, "Close Hades?");
  assert.match(items.find(i => i.id === "close")!.confirm!.body, /saved/);
});
