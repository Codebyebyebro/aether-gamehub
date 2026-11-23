import test from "node:test";
import assert from "node:assert/strict";
import { applyArtwork, artworkPercent, artworkSummary, needsArtwork, wantedFrom } from "./artwork.ts";
import type { Game } from "./aether.ts";

const game = (over: Partial<Game>): Game => ({ id: "x", title: "Game", platform: "Custom", target: "C:\\Games\\g.exe", args: "", workingDir: "", cover: "", banner: "", steamId: "", favorite: false, ...over });
const found = (id: string, cover = `https://cdn.akamai.steamstatic.com/steam/apps/${id}/library_600x900.jpg`, banner = "") => ({ id, cover, banner, source: "Steam", matched: id });

test("only games that would show a placeholder are looked up", () => {
  assert.equal(needsArtwork(game({})), true);
  assert.equal(needsArtwork(game({ cover: "C:\\covers\\a.webp" })), false, "already has a cover");
  assert.equal(needsArtwork(game({ steamId: "367520" })), false, "a Steam id already gives the Steam pictures");
  assert.equal(needsArtwork(game({ title: "  " })), false, "nothing to search for");
  assert.equal(needsArtwork(game({ cover: "  " })), true, "a blank cover is no cover");
});
test("only the id and title are sent, never the install path", () => {
  const sent = wantedFrom([game({ id: "a", title: " Hades " }), game({ id: "b", cover: "x" }), game({ id: "c", title: "Celeste" })]);
  assert.deepEqual(sent, [{ id: "a", title: "Hades" }, { id: "c", title: "Celeste" }]);
  assert.ok(!JSON.stringify(sent).includes("Games"));
});
test("found covers are applied, with the banner when there is none", () => {
  const games = [game({ id: "a" }), game({ id: "b", banner: "C:\\mine.webp" }), game({ id: "c" })];
  const { games: next, applied } = applyArtwork(games, [found("a", "https://cdn.akamai.steamstatic.com/a.jpg", "https://cdn.akamai.steamstatic.com/a-hero.jpg"), found("b", "https://cdn.akamai.steamstatic.com/b.jpg", "https://cdn.akamai.steamstatic.com/b-hero.jpg")]);
  assert.equal(applied, 2);
  assert.equal(next[0].cover, "https://cdn.akamai.steamstatic.com/a.jpg");
  assert.equal(next[0].banner, "https://cdn.akamai.steamstatic.com/a-hero.jpg");
  assert.equal(next[1].banner, "C:\\mine.webp", "the player's own banner stays");
  assert.equal(next[2], games[2], "a game with nothing found is the same object");
});
test("a cover the player picked during the lookup is never overwritten", () => {
  const games = [game({ id: "a", cover: "C:\\covers\\picked.webp" })];
  const { games: next, applied } = applyArtwork(games, [found("a")]);
  assert.equal(applied, 0);
  assert.equal(next[0].cover, "C:\\covers\\picked.webp");
});
test("answers for removed games or without a cover change nothing", () => {
  const games = [game({ id: "a" })];
  assert.equal(applyArtwork(games, [found("gone")]).applied, 0);
  assert.equal(applyArtwork(games, [found("a", "")]).applied, 0);
  assert.equal(applyArtwork(games, []).applied, 0);
});
test("the summary says what happened in plain words", () => {
  assert.equal(artworkSummary({ cancelled: false }, 3, 3), "Done! Found covers for all 3 games.");
  assert.equal(artworkSummary({ cancelled: false }, 1, 1), "Done! Found a cover.");
  assert.equal(artworkSummary({ cancelled: false }, 2, 5), "Found covers for 2 of 5 games.");
  assert.equal(artworkSummary({ cancelled: false }, 1, 5), "Found a cover for 1 of 5 games.");
  assert.equal(artworkSummary({ cancelled: false }, 0, 4), "No covers could be found for certain.");
  assert.equal(artworkSummary({ cancelled: true }, 2, 9), "Stopped. 2 covers added before stopping.");
  assert.equal(artworkSummary({ cancelled: true }, 0, 9), "Stopped. No covers were added.");
  assert.equal(artworkSummary({ cancelled: false }, 0, 0), "Every game already has artwork.");
});
test("the progress bar stays between 0 and 100", () => {
  assert.equal(artworkPercent(null), 0);
  assert.equal(artworkPercent({ done: 0, total: 0, title: "" }), 0);
  assert.equal(artworkPercent({ done: 1, total: 4, title: "a" }), 25);
  assert.equal(artworkPercent({ done: 9, total: 4, title: "" }), 100);
});
