import test from "node:test";
import assert from "node:assert/strict";
import { KEY_CHOICES, KEY_COLUMNS, LAYOUT_CHOICES, PAD_BUTTONS, PRESETS, assign, choiceOf, choose, editorRows, gridMove, keyLabel, layoutName, sendable, withSpeed, withStick } from "./controls.ts";

const preset = (id: string) => PRESETS.find(p => p.id === id)!.controls;

test("the shooter preset is the layout the owner asked for", () => {
  const c = preset("shooter");
  assert.equal(c.leftStick, "wasd"); assert.equal(c.rightStick, "mouse");
  assert.deepEqual(c.buttons, { r2: "MouseLeft", l2: "MouseRight", cross: "Space", circle: "ControlLeft", square: "KeyR", triangle: "KeyE", l1: "KeyQ", r1: "KeyF", options: "Escape", up: "Digit1", right: "Digit2", down: "Digit3", left: "Digit4" });
});
test("point and click and platformer cover the mouse, Space and Shift", () => {
  assert.equal(preset("pointer").leftStick, "mouse"); assert.equal(preset("pointer").buttons.cross, "MouseLeft");
  const p = preset("platformer");
  assert.equal(p.leftStick, "arrows"); assert.equal(p.buttons.cross, "Space"); assert.equal(p.buttons.square, "ShiftLeft");
});
test("every key a preset or the key grid uses is one the program can send", () => {
  for (const p of PRESETS) for (const [button, code] of Object.entries(p.controls.buttons)) {
    assert.ok(sendable(code), `${p.id} ${button} ${code}`);
    assert.ok(PAD_BUTTONS.some(b => b.id === button), button);
  }
  for (const code of KEY_CHOICES) assert.ok(code === "" || sendable(code), code);
  for (const bad of ["MetaLeft", "KeyAA", "F13", "Power", ""]) assert.ok(!sendable(bad), bad);
});
test("the PS button can never be given a key", () => { assert.ok(!PAD_BUTTONS.some(b => (b.id as string) === "ps")); });
test("choosing a preset gives a copy, so editing it never changes the preset", () => {
  const c = choose("shooter", null)!;
  c.buttons.cross = "KeyZ";
  assert.equal(preset("shooter").buttons.cross, "Space");
  assert.equal(choose("off", c), null);
  assert.equal(choose("custom", null)!.preset, "custom", "custom starts from the shooter layout");
  assert.equal(choose("custom", c)!.buttons.cross, "KeyZ", "or from what the player has");
  assert.deepEqual(LAYOUT_CHOICES, ["off", "shooter", "pointer", "platformer", "custom"]);
});
test("any change makes a layout the player's own", () => {
  const c = choose("platformer", null)!;
  assert.equal(layoutName(c), "Platformer");
  const changed = assign(c, "triangle", "KeyE");
  assert.equal(changed.buttons.triangle, "KeyE"); assert.equal(layoutName(changed), "Custom"); assert.equal(choiceOf(changed), "custom");
  assert.equal(assign(changed, "triangle", "").buttons.triangle, undefined, "Nothing clears the button");
  assert.equal(withStick(c, "leftStick", 1).leftStick, "mouse"); assert.equal(withStick(c, "rightStick", 1).rightStick, "wasd", "wraps around");
  assert.equal(withSpeed(c, 9).mouseSpeed, 10); assert.equal(withSpeed(c, -9).mouseSpeed, 1);
  assert.equal(layoutName(null), "Off"); assert.equal(layoutName(undefined), "Off");
});
test("keys read the way they are printed on the keyboard", () => {
  assert.equal(keyLabel("KeyW"), "W"); assert.equal(keyLabel("Digit3"), "3"); assert.equal(keyLabel("ControlLeft"), "Ctrl");
  assert.equal(keyLabel("MouseLeft"), "Left click"); assert.equal(keyLabel("ArrowUp"), "↑"); assert.equal(keyLabel(undefined), "Nothing");
  assert.equal(keyLabel("Numpad4"), "Num 4"); assert.equal(keyLabel("F5"), "F5");
});
test("the key grid moves like a grid and stops at its edges", () => {
  const n = KEY_CHOICES.length;
  assert.equal(gridMove(0, "left", KEY_COLUMNS, n), 0); assert.equal(gridMove(0, "up", KEY_COLUMNS, n), 0);
  assert.equal(gridMove(0, "right", KEY_COLUMNS, n), 1); assert.equal(gridMove(0, "down", KEY_COLUMNS, n), KEY_COLUMNS);
  assert.equal(gridMove(KEY_COLUMNS - 1, "right", KEY_COLUMNS, n), KEY_COLUMNS - 1, "no wrapping onto the next row");
  assert.equal(gridMove(n - 1, "right", KEY_COLUMNS, n), n - 1); assert.equal(gridMove(n - 1, "down", KEY_COLUMNS, n), n - 1);
});
test("the editor shows only the choice when Off, and every button otherwise", () => {
  assert.deepEqual(editorRows(null), ["layout", "save"]);
  const rows = editorRows(choose("shooter", null));
  assert.equal(rows[0], "layout"); assert.equal(rows.at(-1), "save"); assert.equal(rows.length, 4 + PAD_BUTTONS.length + 1);
});
