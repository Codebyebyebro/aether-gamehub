import test from "node:test";
import assert from "node:assert/strict";
import { launchIdentity, stripVerbatim } from "./paths.ts";

const BS = "\\";
test("the verbatim prefix is removed the same way the backend removes it", () => {
  assert.equal(stripVerbatim(String.raw`\\?\C:\Games\Hades\Hades.exe`), String.raw`C:\Games\Hades\Hades.exe`);
  assert.equal(stripVerbatim(String.raw`\\?\UNC\nas\games\x.exe`), String.raw`\\nas\games\x.exe`);
  assert.equal(stripVerbatim(String.raw`D:\Games\x.exe`), String.raw`D:\Games\x.exe`);
  assert.equal(stripVerbatim("steam://rungameid/1"), "steam://rungameid/1");
});
test("a game found by an older scan and the same game found again are recognised as one", () => {
  assert.equal(launchIdentity(String.raw`\\?\C:\Games\Hades.exe`, " -x "), launchIdentity("c:/games/hades.exe", "-x"));
  assert.notEqual(launchIdentity("a.exe", "-x"), launchIdentity("a.exe", "-y"));
  assert.equal(launchIdentity("C:/A/b.exe", ""), `c:${BS}a${BS}b.exe|`);
});
