import test from "node:test";
import assert from "node:assert/strict";
import { absenceNotice, countMissing, sameAbsent, targetKey, toAbsentMap } from "./absent.ts";

test("the program's answer becomes a lookup by game, and an unknown reason shows nothing", () => {
  const map = toAbsentMap([{ id: "a", reason: "file" }, { id: "b", reason: "drive" }, { id: "c", reason: "corrupt" }, { id: "d", reason: "" }]);
  assert.deepEqual(map, { a: "file", b: "drive" });
  assert.deepEqual(toAbsentMap([]), {});
});
test("only games that are really gone are counted; a disconnected drive is not a problem with the game", () => {
  assert.equal(countMissing({ a: "file", b: "drive", c: "file" }), 2);
  assert.equal(countMissing({ a: "drive" }), 0);
  assert.equal(countMissing({}), 0);
});
test("the re-check key changes when a target changes and not when a game is only favourited or renamed", () => {
  const games = [{ id: "1", target: "C:\\a.exe", title: "A", favorite: false }, { id: "2", target: "steam://rungameid/9", title: "B", favorite: false }];
  const before = targetKey(games);
  assert.equal(targetKey(games.map(g => ({ ...g, favorite: true, title: g.title + "!" }))), before);
  assert.notEqual(targetKey([{ ...games[0], target: "D:\\a.exe" }, games[1]]), before, "pointed somewhere else");
  assert.notEqual(targetKey(games.slice(1)), before, "removed");
  assert.notEqual(targetKey([...games, { id: "3", target: "C:\\c.exe", title: "C", favorite: false }]), before, "added");
  assert.equal(targetKey([]), "");
});
test("the messages name the game and say what to do, and Big Box does not promise a dialog it cannot open", () => {
  const file = absenceNotice("Hollow Knight", "file", false);
  assert.match(file, /^Hollow Knight can't be found\./);
  assert.match(file, /Choose where it is now, or remove it/);
  const big = absenceNotice("Hollow Knight", "file", true);
  assert.match(big, /Leave Big Box/);
  assert.doesNotMatch(big, /Choose where it is now/);
  const drive = absenceNotice("Hollow Knight", "drive", false);
  assert.match(drive, /isn't connected\. Connect it and try again\.$/);
  assert.equal(absenceNotice("X", "drive", true), absenceNotice("X", "drive", false), "the drive message is the same everywhere");
  assert.doesNotMatch(drive, /remove/i, "an unplugged drive is never a reason to remove the game");
});
test("an unchanged answer is recognised as unchanged, whatever order it arrives in", () => {
  assert.equal(sameAbsent({}, {}), true);
  assert.equal(sameAbsent({ a: "file", b: "drive" }, { b: "drive", a: "file" }), true);
  assert.equal(sameAbsent({ a: "file" }, {}), false, "a game stopped being missing");
  assert.equal(sameAbsent({}, { a: "file" }), false, "a game became missing");
  assert.equal(sameAbsent({ a: "file" }, { a: "drive" }), false, "the reason changed");
  assert.equal(sameAbsent({ a: "file" }, { b: "file" }), false, "a different game");
});
