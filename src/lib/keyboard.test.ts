import test from "node:test";
import assert from "node:assert/strict";
import { BACKSPACE, DONE, LAYOUT, LEFT, RIGHT, SHIFT, SPACE, START, SYMBOLS, applyKey, insert, keyLabel, moveKey, previewText, type KeyDef, type KeyboardState } from "./keyboard.ts";

const at = (row: number, col: number) => LAYOUT.letters[row][col];
const charKey = (c: string) => LAYOUT.letters.flat().find(k => k.char === c)!;
const type = (state: KeyboardState, keys: KeyDef[], maxLength?: number) => keys.reduce((s, k) => applyKey(s, k, maxLength), { ...state, done: false });
const empty: KeyboardState = { edit: { value: "", cursor: 0 }, shift: "off", page: "letters" };

test("both pages have the same shape, every row is 10 keys wide, and the highlight starts on q", () => {
  for (const page of ["letters", "symbols"] as const) {
    assert.deepEqual(LAYOUT[page].map(r => r.reduce((w, k) => w + k.width, 0)), [10, 10, 10, 10, 10], page);
  }
  assert.deepEqual(LAYOUT.letters.map(r => r.length), LAYOUT.symbols.map(r => r.length));
  assert.equal(at(START.row, START.col).char, "q");
  assert.deepEqual(LAYOUT.letters[4].map(k => k.action), ["symbols", "left", "right", "space", "backspace", "done"]);
});
test("left and right wrap around a row; up and down land on the key straight above or below", () => {
  const rows = LAYOUT.letters;
  assert.deepEqual(moveKey(rows, { row: 1, col: 0 }, "left"), { row: 1, col: 9 });
  assert.deepEqual(moveKey(rows, { row: 1, col: 9 }, "right"), { row: 1, col: 0 });
  assert.deepEqual(moveKey(rows, { row: 1, col: 3 }, "up"), { row: 0, col: 3 }, "r is under 4");
  assert.deepEqual(moveKey(rows, { row: 0, col: 3 }, "up"), { row: 0, col: 3 }, "the top edge stops");
  assert.equal(rows[3][moveKey(rows, { row: 2, col: 2 }, "down").col].char, "x", "d sits over x (shift takes the first slot)");
  assert.equal(rows[4][moveKey(rows, { row: 3, col: 5 }, "down").col].action, "space", "b drops onto the space bar");
  assert.equal(rows[4][moveKey(rows, { row: 3, col: 9 }, "down").col].action, "done", ". drops onto Done");
  assert.equal(rows[3][moveKey(rows, { row: 4, col: 3 }, "up").col].char, "b", "space goes back up to the middle");
  assert.deepEqual(moveKey(rows, { row: 4, col: 0 }, "down"), { row: 4, col: 0 }, "the bottom edge stops");
});
test("typing, space, cursor moves and backspace edit at the cursor", () => {
  let s = type(empty, [charKey("h"), charKey("d"), LEFT, charKey("a"), RIGHT, SPACE, charKey("2")]);
  assert.deepEqual(s.edit, { value: "had 2", cursor: 5 });
  s = type(s, [LEFT, LEFT, BACKSPACE]);
  assert.deepEqual(s.edit, { value: "ha 2", cursor: 2 });
  s = type({ ...s, edit: { value: "ha 2", cursor: 0 } }, [BACKSPACE, LEFT]);
  assert.deepEqual(s.edit, { value: "ha 2", cursor: 0 }, "nothing to delete and nowhere to go at the start");
  assert.deepEqual(type({ ...s, edit: { value: "ab", cursor: 2 } }, [RIGHT]).edit.cursor, 2, "nor past the end");
  assert.equal(type(s, [DONE]).done, true);
});
test("Shift is one-shot, a second press locks caps, a third turns it off", () => {
  let s = type(empty, [SHIFT, charKey("e"), charKey("l")]);
  assert.equal(s.edit.value, "El"); assert.equal(s.shift, "off");
  s = type(s, [SHIFT, SHIFT, charKey("d"), charKey("e"), charKey("n")]);
  assert.equal(s.edit.value, "ElDEN"); assert.equal(s.shift, "lock");
  s = type(s, [SHIFT, charKey("x")]);
  assert.equal(s.edit.value, "ElDENx"); assert.equal(s.shift, "off");
  assert.equal(keyLabel(charKey("q"), "once", "letters"), "Q");
  assert.equal(keyLabel(SHIFT, "lock", "letters"), "⇪");
});
test("the symbols page types symbols, ignores Shift, and switches back", () => {
  let s = type(empty, [SYMBOLS]);
  assert.equal(s.page, "symbols"); assert.equal(keyLabel(SYMBOLS, s.shift, s.page), "ABC");
  const backslash = LAYOUT.symbols[3][1], colon = LAYOUT.symbols[2][9];
  s = type(s, [SHIFT, { ...colon }, { ...backslash }]);
  assert.equal(s.edit.value, ":\\"); assert.equal(s.shift, "off", "Shift does nothing on the symbols page");
  assert.equal(type(s, [SYMBOLS]).page, "letters");
});
test("maxLength is respected, including for spaces", () => {
  const full = { ...empty, edit: { value: "abc", cursor: 3 } };
  assert.equal(type(full, [charKey("d"), SPACE], 3).edit.value, "abc");
  assert.equal(type(full, [charKey("d")], 4).edit.value, "abcd");
  assert.deepEqual(insert({ value: "ab", cursor: 99 }, "c"), { value: "abc", cursor: 3 }, "a stale cursor is clamped");
});
test("password previews stay masked", () => {
  assert.equal(previewText("hunter2", "password"), "•••••••");
  assert.equal(previewText("Hades", "text"), "Hades");
  assert.equal(previewText("Hades", null), "Hades");
});
