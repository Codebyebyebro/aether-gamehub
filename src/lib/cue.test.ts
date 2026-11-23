import test from "node:test";
import assert from "node:assert/strict";
import { bytesFromBase64, createCueGate, CUE_GAIN, CUE_MIN_GAP_MS, playSelectCue, setCueEnabled, warmCue, BOOT_NOTES, playBootChime } from "./cue.ts";
import { SELECT_SOUND_BASE64 } from "./selectSound.ts";

const tick = () => new Promise(resolve => setTimeout(resolve, 0));

test("the rate limiter lets a cue through, then blocks until the minimum gap has passed", () => {
  const gate = createCueGate();
  assert.equal(gate(1000), true);
  assert.equal(gate(1000 + CUE_MIN_GAP_MS - 1), false, "just inside the gap");
  assert.equal(gate(1000 + CUE_MIN_GAP_MS), true, "exactly the gap later");
  assert.equal(gate(1000 + CUE_MIN_GAP_MS + 5), false, "the gap is measured from the last cue that played, not the last attempt");
  assert.equal(createCueGate(0)(5), true);
});
test("the embedded click is the supplied WAV: 16 ms, stereo, 24-bit, 44.1 kHz, and not silence", () => {
  const bytes = new Uint8Array(bytesFromBase64(SELECT_SOUND_BASE64)); const view = new DataView(bytes.buffer);
  const tag = (at: number) => String.fromCharCode(...bytes.slice(at, at + 4));
  assert.equal(bytes.length, 4912);
  assert.deepEqual([tag(0), tag(8), tag(12)], ["RIFF", "WAVE", "fmt "]);
  assert.deepEqual([view.getUint16(20, true), view.getUint16(22, true), view.getUint32(24, true), view.getUint16(34, true)], [1, 2, 44100, 24], "PCM, channels, rate, bits");
  assert.equal(tag(36), "data");
  const dataBytes = view.getUint32(40, true); assert.equal(dataBytes / (2 * 3), 711, "711 frames is about 16 ms");
  let peak = 0; for (let i = 44; i + 2 < 44 + dataBytes; i += 3) { const v = (bytes[i] | (bytes[i + 1] << 8) | (bytes[i + 2] << 16)) << 8 >> 8; peak = Math.max(peak, Math.abs(v)); }
  assert.ok(peak > 1_000_000, `the click carries real signal, peak ${peak}`);
  assert.ok(CUE_GAIN > 0 && CUE_GAIN < 1, "played below full scale");
});

// A stand-in for the browser's audio engine, so the real player runs without a sound card.
interface Fake { state: string; starts: number; resumed: number; closed: boolean; decoded: number }
let made: Fake[] = []; let failDecode = false; let initialState = "running"; let holdDecode: (() => void) | null = null; let hold = false;
class FakeAudioContext implements Fake {
  state = initialState; starts = 0; resumed = 0; closed = false; decoded = 0; destination = {};
  constructor() { made.push(this); }
  createGain() { return { gain: { value: 1 }, connect() {} }; }
  decodeAudioData(buffer: ArrayBuffer) {
    this.decoded++; if (failDecode) return Promise.reject(new Error("bad audio"));
    return hold ? new Promise(resolve => { holdDecode = () => resolve({ length: buffer.byteLength }); }) : Promise.resolve({ length: buffer.byteLength });
  }
  // Like a real browser, starting a source on a context that is not running is an error.
  createBufferSource() { const owner = this; return { buffer: null, connect() {}, start() { if (owner.state !== "running") throw new Error("InvalidStateError"); owner.starts++; } }; }
  resume() { this.resumed++; this.state = "running"; return Promise.resolve(); }
  close() { this.closed = true; this.state = "closed"; return Promise.resolve(); }
}
const g = globalThis as { AudioContext?: unknown };
function fresh(state = "running", fail = false) { setCueEnabled(false); made = []; initialState = state; failDecode = fail; hold = false; holdDecode = null; g.AudioContext = FakeAudioContext; setCueEnabled(true); }
let clock = 10_000; const next = (by = 100) => (clock += by);

test("each allowed cue plays the click once, and cues inside the minimum gap are dropped", async () => {
  fresh(); playSelectCue(next()); await tick();
  assert.equal(made.length, 1); assert.equal(made[0].starts, 1); assert.equal(made[0].decoded, 1);
  playSelectCue(next(CUE_MIN_GAP_MS - 1)); await tick(); assert.equal(made[0].starts, 1, "too soon after the last");
  playSelectCue(next(CUE_MIN_GAP_MS)); await tick(); assert.equal(made[0].starts, 2);
  assert.equal(made[0].decoded, 1, "the click is decoded once and reused");
});
test("switched off, nothing plays, nothing is opened, and the audio device is released", async () => {
  fresh(); playSelectCue(next()); await tick();
  setCueEnabled(false); assert.equal(made[0].closed, true);
  playSelectCue(next()); await tick(); assert.equal(made.length, 1, "no new audio engine is created while off"); assert.equal(made[0].starts, 1);
  setCueEnabled(true); playSelectCue(next()); await tick(); assert.equal(made.length, 2, "switching back on starts afresh"); assert.equal(made[1].starts, 1);
});
test("a suspended audio device is woken and the cue is skipped rather than queued", async () => {
  fresh("suspended"); playSelectCue(next()); await tick();
  assert.equal(made[0].resumed, 1); assert.equal(made[0].starts, 0, "this cue is skipped so a backlog never plays later");
  playSelectCue(next()); await tick(); assert.equal(made[0].starts, 1);
});
test("warming up opens the device and decodes ahead of the first cue, only when enabled", async () => {
  fresh("suspended"); warmCue(); await tick(); assert.equal(made.length, 1); assert.equal(made[0].decoded, 1); assert.equal(made[0].resumed, 1);
  setCueEnabled(false); made = []; warmCue(); await tick(); assert.equal(made.length, 0, "off means no audio device is opened");
});
test("a broken decode or a missing audio engine never throws and never plays", async () => {
  fresh("running", true); playSelectCue(next()); await tick(); assert.equal(made[0].starts, 0);
  setCueEnabled(false); delete g.AudioContext; setCueEnabled(true);
  assert.doesNotThrow(() => { playSelectCue(next()); warmCue(); });
  g.AudioContext = FakeAudioContext; setCueEnabled(false);
});
test("switching off while the click is still decoding plays nothing and does not error", async () => {
  fresh(); hold = true; playSelectCue(next()); await tick();
  assert.equal(made[0].decoded, 1, "decoding has started");
  setCueEnabled(false); (holdDecode as (() => void) | null)?.(); await tick();
  assert.equal(made[0].starts, 0, "the device was closed meanwhile, so the click is dropped instead of throwing");
  hold = false;
});
test("the start-up chime rises, overlaps gently and is over in under two seconds", () => {
  assert.equal(BOOT_NOTES.length, 3);
  for (let i = 1; i < BOOT_NOTES.length; i++) { assert.ok(BOOT_NOTES[i].hz > BOOT_NOTES[i - 1].hz, "each note is higher"); assert.ok(BOOT_NOTES[i].at > BOOT_NOTES[i - 1].at); }
  const end = Math.max(...BOOT_NOTES.map(n => n.at + n.length));
  assert.ok(end < 2, `${end}s`);
});
test("the start-up chime plays its three notes, and is silent when sounds are off", async () => {
  let started = 0; let stopped = 0;
  class ToneContext extends FakeAudioContext {
    currentTime = 0;
    createOscillator() { return { type: "", frequency: { value: 0 }, connect() {}, start() { started++; }, stop() { stopped++; } }; }
    createGain() { return { gain: { value: 1, setValueAtTime() {}, exponentialRampToValueAtTime() {} }, connect() {} }; }
  }
  fresh(); g.AudioContext = ToneContext; setCueEnabled(false); setCueEnabled(true);
  playBootChime(); await tick();
  assert.equal(started, 3); assert.equal(stopped, 3);
  setCueEnabled(false); playBootChime(); await tick();
  assert.equal(started, 3, "switched off: nothing more plays");
});
test("the start-up chime never throws, even on an audio device that cannot make tones", async () => {
  fresh(); assert.doesNotThrow(() => playBootChime()); await tick();
});
