import test from "node:test";
import assert from "node:assert/strict";
import { DETAIL_ACTIONS, HEADER_ACTIONS, HOME_ACTIONS, TILE, fullscreenAfterBigBox, rowScroll, searchGames } from "./consoleNav.ts";

test("Down reaches Play, Details and Favorite; the details page has Play, Favorite and Back", () => {
  assert.deepEqual(HOME_ACTIONS, ["play", "details", "favorite"]);
  assert.deepEqual(DETAIL_ACTIONS, ["play", "favorite", "controls", "back"]);
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
test("leaving Big Box only drops full screen when Big Box turned it on and nobody asked to keep it", () => {
  assert.equal(fullscreenAfterBigBox(false, false), false, "Big Box made it full screen: back to a window");
  assert.equal(fullscreenAfterBigBox(true, false), true, "it was already full screen (F11): it stays");
  assert.equal(fullscreenAfterBigBox(false, true), true, "the setting, or stepping aside for Settings: it stays");
});
test("Up from the games reaches Search, Scan, Settings and Exit, in that order", () => {
  assert.deepEqual(HEADER_ACTIONS, ["search", "scan", "settings", "exit"]);
});
test("Big Box search matches every word typed, in any order, ignoring case and accents", () => {
  const games = [{ title: "Hollow Knight" }, { title: "Pokémon Legends" }, { title: "Knights of Honor" }, { title: "Hades" }];
  assert.deepEqual(searchGames(games, "").map(g => g.title), games.map(g => g.title), "empty search shows everything");
  assert.deepEqual(searchGames(games, "  knight ").map(g => g.title), ["Hollow Knight", "Knights of Honor"]);
  assert.deepEqual(searchGames(games, "KNIGHT hollow").map(g => g.title), ["Hollow Knight"]);
  assert.deepEqual(searchGames(games, "pokemon").map(g => g.title), ["Pokémon Legends"]);
  assert.deepEqual(searchGames(games, "zelda"), []);
});
