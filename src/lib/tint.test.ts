import test from "node:test";
import assert from "node:assert/strict";
import { tintSource, tintTarget, tintValue } from "./tint.ts";

const steam = (id: string) => `https://cdn.akamai.steamstatic.com/steam/apps/${id}/library_600x900.jpg`;
const art = (over: Partial<{ cover: string; banner: string; steamId: string }>) => ({ cover: "", banner: "", steamId: "", ...over });

test("the colour comes from the cover, then the banner, then the Steam cover", () => {
  assert.equal(tintSource(art({ cover: "C:\\covers\\a.webp", banner: "C:\\covers\\b.webp" }), steam), "C:\\covers\\a.webp");
  assert.equal(tintSource(art({ banner: " C:\\covers\\b.webp " }), steam), "C:\\covers\\b.webp");
  assert.equal(tintSource(art({ steamId: "367520" }), steam), steam("367520"));
  assert.equal(tintSource(art({}), steam), "");
  assert.equal(tintSource(null, steam), "");
});
test("pictures that only exist in the browser, and odd Steam ids, give no colour", () => {
  assert.equal(tintSource(art({ cover: "data:image/png;base64,AAA" }), steam), "");
  assert.equal(tintSource(art({ cover: "blob:abc" }), steam), "");
  assert.equal(tintSource(art({ steamId: "12a" }), steam), "");
});
test("the stylesheet value is three whole channels, or nothing", () => {
  assert.equal(tintValue({ r: 200, g: 30, b: 40 }), "200 30 40");
  assert.equal(tintValue({ r: 0, g: 0, b: 0 }), "0 0 0");
  assert.equal(tintValue(null), "");
  assert.equal(tintValue({ r: 256, g: 0, b: 0 }), "");
  assert.equal(tintValue({ r: 1.5, g: 0, b: 0 }), "");
  assert.equal(tintValue({ r: -1, g: 0, b: 0 }), "");
});
test("a window beats Big Box, which beats the pointer, which beats the spotlight", () => {
  const [d, b, p, s] = ["detail", "bigbox", "pointed", "spot"].map(id => ({ id }));
  assert.equal(tintTarget({ detail: d, bigBox: b, pointed: p, spotlight: s }), d);
  assert.equal(tintTarget({ bigBox: b, pointed: p, spotlight: s }), b);
  assert.equal(tintTarget({ pointed: p, spotlight: s }), p);
  assert.equal(tintTarget({ spotlight: s }), s);
  assert.equal(tintTarget({}), null);
});
