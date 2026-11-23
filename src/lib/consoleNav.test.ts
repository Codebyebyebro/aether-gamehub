import test from "node:test";
import assert from "node:assert/strict";
import { DETAIL_ACTIONS, HOME_ACTIONS, TILE, rowScroll } from "./consoleNav.ts";

test("Down reaches Play, Details and Favorite; the details page has Play, Favorite and Back", () => {
  assert.deepEqual(HOME_ACTIONS, ["play", "details", "favorite"]);
  assert.deepEqual(DETAIL_ACTIONS, ["play", "favorite", "back"]);
  assert.equal(HOME_ACTIONS[0], "play", "Play is where Down lands first");
});
test("the row scrolls so the selected tile is centred, and never before the start", () => {
  assert.equal(rowScroll(0, 1440), 0);
  assert.equal(rowScroll(1, 1440), 0, "near the start nothing needs to scroll");
  const i = 20, view = 1440;
  const centre = TILE.padding + i * (TILE.width + TILE.gap) + TILE.selected / 2 - rowScroll(i, view);
  assert.equal(centre, view / 2, "tile 20's middle is the screen's middle");
  assert.equal(rowScroll(-1, 1440), 0);
  assert.equal(rowScroll(5, NaN), 0);
});
test("each step along the row scrolls by exactly one tile and gap, so the movement is even", () => {
  const steps = [30, 31, 32, 33].map(i => rowScroll(i, 1280));
  for (let k = 1; k < steps.length; k++) assert.equal(steps[k] - steps[k - 1], TILE.width + TILE.gap);
});
