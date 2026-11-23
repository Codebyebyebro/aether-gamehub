import { SELECT_SOUND_BASE64 } from "./selectSound.ts";

/** The shortest time between two cues, so the fastest held repeat and any burst of focus changes never machine-guns. */
export const CUE_MIN_GAP_MS = 40;
/** The click is mastered near full scale; this keeps it a quiet tick rather than a bang. */
export const CUE_GAIN = 0.35;

/** A rate limiter: returns true when a cue may play at `now`, and remembers it. Pure, so it is tested without audio. */
export function createCueGate(minGap = CUE_MIN_GAP_MS): (now: number) => boolean {
  let last = -Infinity;
  return now => { if (now - last < minGap) return false; last = now; return true; };
}
export function bytesFromBase64(text: string): ArrayBuffer {
  const binary = atob(text); const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}

type Engine = { context: AudioContext; gain: GainNode; buffer: Promise<AudioBuffer | null> };
let enabled = true;
let engine: Engine | null | undefined;
const gate = createCueGate();

/** Turns the cue on or off (the Settings switch). Turning it off also releases the audio device. */
export function setCueEnabled(on: boolean) {
  enabled = on;
  if (!on) { if (engine) void engine.context.close().catch(() => {}); engine = undefined; }
}
function start(): Engine | null {
  if (engine !== undefined) return engine;
  const Ctor: typeof AudioContext | undefined = (globalThis as { AudioContext?: typeof AudioContext; webkitAudioContext?: typeof AudioContext }).AudioContext
    ?? (globalThis as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Ctor) return (engine = null);
  try {
    const context = new Ctor({ latencyHint: "interactive" }); const gain = context.createGain();
    gain.gain.value = CUE_GAIN; gain.connect(context.destination);
    engine = { context, gain, buffer: context.decodeAudioData(bytesFromBase64(SELECT_SOUND_BASE64)).catch(() => null) };
  } catch { engine = null; }
  return engine;
}
/** Opens the audio device and decodes the click ahead of time, so the first real cue is not late. */
export function warmCue() { if (enabled) { const e = start(); if (e && e.context.state === "suspended") void e.context.resume().catch(() => {}); } }

/** Plays the selection click. Silent when switched off, too soon after the last one, or while the audio device is not running. */
export function playSelectCue(now: number = performance.now()) {
  if (!enabled || !gate(now)) return;
  const e = start(); if (!e) return;
  if (e.context.state === "suspended") { void e.context.resume().catch(() => {}); return; }
  void e.buffer.then(buffer => {
    if (!buffer || e.context.state !== "running") return;
    const source = e.context.createBufferSource(); source.buffer = buffer; source.connect(e.gain); source.start();
  });
}
