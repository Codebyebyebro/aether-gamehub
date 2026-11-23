import test from "node:test";
import assert from "node:assert/strict";
import { isTextEntry, pickNeighbor, type Box } from "./spatialNav.ts";

const rect = (left: number, top: number, w: number, h: number): Box => ({ left, top, right: left + w, bottom: top + h });
/** 4 columns x 3 rows of 100x150 cards, 20px apart, to the right of a 180px sidebar with 5 buttons. */
const card = (row: number, col: number) => rect(220 + col * 120, 100 + row * 170, 100, 150);
const side = (i: number) => rect(0, i * 50, 180, 40);
const grid = Array.from({ length: 12 }, (_, i) => card(Math.floor(i / 4), i % 4));
const all = [...Array.from({ length: 5 }, (_, i) => side(i)), ...grid];
const go = (from: Box, dir: Parameters<typeof pickNeighbor>[2], pool = all) => { const others = pool.filter(b => b !== from); const i = pickNeighbor(from, others, dir); return i < 0 ? null : others[i]; };

test("in a grid, arrows move to the neighbouring card", () => {
  assert.equal(go(card(0, 0), "right"), card(0, 1) && grid[1]);
  assert.equal(go(card(0, 1), "down"), grid[5]);
  assert.equal(go(grid[5], "up"), grid[1]);
  assert.equal(go(grid[5], "left"), grid[4]);
});
test("nothing beyond the edge of the grid, so the focus stays put instead of wrapping somewhere surprising", () => {
  assert.equal(go(grid[3], "right", grid), null);
  assert.equal(go(grid[10], "down", grid), null);
  assert.equal(go(grid[1], "up", grid), null);
});
test("when nothing lines up, the nearest control is used so that every control stays reachable", () => {
  // Nothing sits directly above the first row here, but a sidebar button is higher up: still better than being stuck.
  assert.ok(go(grid[1], "up"), "falls back to the nearest control above");
});
test("from the first column, left reaches the sidebar button level with it", () => {
  const target = go(grid[4], "left"); // row 1 spans y 270..420: sidebar buttons only reach y 240
  assert.ok(target && all.indexOf(target) < 5, "a sidebar button");
  const top = go(grid[0], "left"); // row 0 spans y 100..250: buttons 2, 3 and 4 share that height
  assert.ok(top && [side(2), side(3), side(4)].some(s => s.top === top.top), "one of the buttons level with the card");
});
test("from the sidebar, right enters the grid at the card level with the button", () => {
  assert.equal(go(side(3), "right"), grid[0]);
});
test("a short last row does not strand the focus: moving down picks the nearest card", () => {
  const ragged = [...grid.slice(0, 8), ...grid.slice(8, 10)]; // last row has only 2 cards
  assert.equal(go(grid[7], "down", ragged), grid[9]);
});
test("straight ahead beats a nearer diagonal", () => {
  const from = rect(0, 0, 100, 100), straight = rect(300, 0, 100, 100), diagonal = rect(120, 140, 100, 100);
  assert.equal(pickNeighbor(from, [diagonal, straight], "right"), 1);
});
test("empty and single-element cases", () => {
  assert.equal(pickNeighbor(rect(0, 0, 10, 10), [], "up"), -1);
  assert.equal(pickNeighbor(rect(0, 0, 10, 10), [rect(0, 0, 10, 10)], "right"), -1);
});
test("Cross opens the on-screen keyboard only on fields that hold typed text", () => {
  for (const [tag, type] of [["INPUT", null], ["input", "text"], ["INPUT", "search"], ["INPUT", "Password"], ["INPUT", "url"], ["INPUT", "tel"], ["TEXTAREA", null]] as const) assert.ok(isTextEntry(tag, type), `${tag} ${type}`);
  for (const [tag, type] of [["INPUT", "color"], ["INPUT", "range"], ["INPUT", "checkbox"], ["INPUT", "file"], ["INPUT", "number"], ["INPUT", "email"], ["BUTTON", null], ["SELECT", null]] as const) assert.ok(!isTextEntry(tag, type), `${tag} ${type}`);
});
