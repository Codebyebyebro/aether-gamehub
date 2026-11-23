import test from "node:test";
import assert from "node:assert/strict";
import { playedLine, recentlyPlayed, sortGames, spotlight, spotlightLabel, agoWords } from "./recent.ts";
import type { Game } from "./aether.ts";

const game = (id: string, title = id): Game => ({ id, title, platform: "Custom", target: `C:\\${id}.exe`, args: "", workingDir: "", cover: "", banner: "", steamId: "", favorite: false });
const games = [game("a", "Celeste"), game("b", "Alan Wake"), game("c", "Hades"), game("d", "Brotato")];
const times = { b: { seconds: 3600, lastPlayed: 1_000 }, c: { seconds: 60_000, lastPlayed: 3_000 }, d: { seconds: 7200, lastPlayed: 2_000 } };
const ids = (list: Game[]) => list.map(g => g.id).join("");

test("sorting by date added keeps the library's order and never changes the input", () => {
  const copy = [...games];
  assert.equal(ids(sortGames(games, "added", times)), "abcd");
  sortGames(games, "az", times); sortGames(games, "recent", times);
  assert.deepEqual(games, copy);
});
test("A–Z, recently played and most played each order the library", () => {
  assert.equal(ids(sortGames(games, "az", times)), "bdac");
  assert.equal(ids(sortGames(games, "recent", times)), "cdba", "never-played games come last");
  assert.equal(ids(sortGames(games, "played", times)), "cdba");
});
test("most played breaks ties by the most recent", () => {
  const tied = { a: { seconds: 100, lastPlayed: 5 }, b: { seconds: 100, lastPlayed: 9 } };
  assert.equal(ids(sortGames(games.slice(0, 2), "played", tied)), "ba");
});
test("a library nobody has played yet looks the same in every play-time order", () => {
  assert.equal(ids(sortGames(games, "recent", {})), "abcd");
  assert.equal(ids(sortGames(games, "played", {})), "abcd");
});
test("recently played lists only played games, newest first, up to the limit", () => {
  assert.equal(ids(recentlyPlayed(games, times)), "cdb");
  assert.equal(ids(recentlyPlayed(games, times, 2)), "cd");
  assert.equal(recentlyPlayed(games, {}).length, 0);
  assert.equal(ids(recentlyPlayed(games, { a: { seconds: 500, lastPlayed: 0 } })), "", "time imported without a date is not 'recent'");
});
test("the spotlight shows what you are playing, then what you played last, then the first games", () => {
  assert.deepEqual([spotlight(games, times, "a").mode, ids(spotlight(games, times, "a").games)], ["playing", "acd"]);
  assert.deepEqual([spotlight(games, times, "c").mode, ids(spotlight(games, times, "c").games)], ["playing", "cdb"], "no duplicate when it was also played last");
  assert.deepEqual([spotlight(games, times).mode, ids(spotlight(games, times).games)], ["recent", "cdb"]);
  assert.deepEqual([spotlight(games, {}).mode, ids(spotlight(games, {}).games)], ["featured", "abc"]);
  assert.equal(spotlight([], times).games.length, 0);
  assert.equal(spotlight(games, times, "gone").mode, "recent", "a game removed while running is not shown");
});
test("the spotlight label follows why the game is there", () => {
  assert.equal(spotlightLabel.recent, "Jump back in");
  assert.equal(spotlightLabel.playing, "Now playing");
  assert.equal(spotlightLabel.featured, "In the spotlight");
});
test("the played line says how long and when, in plain words", () => {
  const now = 10 * 86_400_000;
  assert.equal(playedLine({ seconds: 9000, lastPlayed: now / 1000 - 86_400 }, now), "2.5 h played · last played yesterday");
  assert.equal(playedLine({ seconds: 0, lastPlayed: now / 1000 - 3 * 86_400 }, now), "last played 3 days ago");
  assert.equal(playedLine({ seconds: 1800, lastPlayed: 0 }, now), "30 min played");
  assert.equal(playedLine(undefined, now), "");
  assert.equal(playedLine({ seconds: 0, lastPlayed: 0 }, now), "");
  assert.equal(agoWords(now / 1000 - 30, now), "just now");
});
