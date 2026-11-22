import test from "node:test";
import assert from "node:assert/strict";
import { formatAgo, formatBytes, formatPlaytime, scanCountsLabel, scanFraction, scanStepLabel, shortPath } from "./format.ts";

test("sizes read naturally and match the scanner's wording", () => {
  assert.equal(formatBytes(512), "512 B");
  assert.equal(formatBytes(2 * 1024 * 1024), "2.0 MB");
  assert.equal(formatBytes(88_350_000), "84.3 MB");
  assert.equal(formatBytes(1.5 * 1024 ** 3), "1.5 GB");
  for (const unknown of [null, undefined, -1, NaN]) assert.equal(formatBytes(unknown as number), "—");
});
test("playtime is shown in the unit people think in", () => {
  assert.equal(formatPlaytime(undefined), "Not played yet");
  assert.equal(formatPlaytime(0), "Not played yet");
  assert.equal(formatPlaytime(20), "Under a minute");
  assert.equal(formatPlaytime(45 * 60), "45 min");
  assert.equal(formatPlaytime(12.5 * 3600), "12.5 h");
  assert.equal(formatPlaytime(1234.5 * 3600), "1,235 h");
});
test("relative times", () => {
  const now = 1_700_000_000_000, at = (secondsAgo: number) => now / 1000 - secondsAgo;
  assert.equal(formatAgo(0, now), "never");
  assert.equal(formatAgo(at(5), now), "just now");
  assert.equal(formatAgo(at(5 * 60), now), "5 min ago");
  assert.equal(formatAgo(at(3 * 3600), now), "3 h ago");
  assert.equal(formatAgo(at(86400), now), "1 day ago");
  assert.equal(formatAgo(at(40 * 86400), now), "1 month ago");
  assert.equal(formatAgo(at(800 * 86400), now), "2 y ago");
  assert.equal(formatAgo(at(-500), now), "just now", "a clock slightly in the future never shows a negative time");
});
test("long paths keep the drive and the file name", () => {
  const long = String.raw`D:\Games\Some Publisher\A Very Long Studio Name\Hades II Early Access\Binaries\Win64\Hades2.exe`;
  const short = shortPath(long, 50);
  assert.ok(short.length <= 50 && short.startsWith("D:\\") && short.endsWith("Hades2.exe") && short.includes("…"), short);
  assert.equal(shortPath(String.raw`C:\Games\x.exe`, 50), String.raw`C:\Games\x.exe`);
});
test("scan progress moves one step per folder and never leaves 0–1", () => {
  assert.equal(scanFraction(null), 0);
  assert.equal(scanFraction({ step: 0, steps: 3 }), 0);
  assert.equal(scanFraction({ step: 2, steps: 4 }), 0.5);
  assert.equal(scanFraction({ step: 3, steps: 3 }), 1);
  assert.equal(scanFraction({ step: 9, steps: 3 }), 1);
  assert.equal(scanFraction({ step: 1, steps: 0 }), 0, "a malformed event cannot draw a full or broken bar");
  assert.equal(scanFraction({ step: -1, steps: 2 }), 0);
});
test("scan progress wording", () => {
  assert.equal(scanStepLabel(null), "Reading Steam, Epic and Riot install lists…");
  assert.equal(scanStepLabel({ step: 0, steps: 3 }), "Reading Steam, Epic and Riot install lists…");
  assert.equal(scanStepLabel({ step: 1, steps: 2 }), "Searching your folder…");
  assert.equal(scanStepLabel({ step: 2, steps: 4 }), "Searching folder 2 of 3…");
  assert.equal(scanStepLabel({ step: 4, steps: 4 }), "Preparing your review…");
  assert.equal(scanCountsLabel(null), "Starting…");
  assert.equal(scanCountsLabel({ checked: 1, found: 1 }), "1 item checked · 1 game found");
  assert.equal(scanCountsLabel({ checked: 12345, found: 0 }), "12,345 items checked · 0 games found");
});
