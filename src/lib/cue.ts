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

/** Console mode's start-up chime: three soft rising notes (C5, G5, C6), each fading out, about 1.4 s in all. */
export const BOOT_NOTES: readonly { hz: number; at: number; length: number }[] = [
  { hz: 523.25, at: 0, length: 0.9 }, { hz: 783.99, at: 0.16, length: 0.9 }, { hz: 1046.5, at: 0.32, length: 1.1 },
];
/** Launching a game: a quick bright rise, under a second. */
export const LAUNCH_NOTES: readonly { hz: number; at: number; length: number }[] = [
  { hz: 659.25, at: 0, length: 0.35 }, { hz: 987.77, at: 0.07, length: 0.45 }, { hz: 1318.51, at: 0.14, length: 0.6 },
];
/** Plays the start-up chime, synthesised (no sound file). Silent when navigation sounds are switched off. */
export function playBootChime() { playNotes(BOOT_NOTES); }
/** Plays the launch sound when a game is started with the controller. Silent when navigation sounds are switched off. */
export function playLaunchCue() { playNotes(LAUNCH_NOTES); }
function playNotes(notes: readonly { hz: number; at: number; length: number }[]) {
  if (!enabled) return;
  const e = start(); if (!e) return;
  // An audio device that cannot make tones just stays silent; a chime is never worth an error.
  const play = () => { try {
    const t0 = e.context.currentTime + 0.05;
    for (const note of notes) {
      const osc = e.context.createOscillator(); const env = e.context.createGain();
      osc.type = "sine"; osc.frequency.value = note.hz;
      env.gain.setValueAtTime(0.0001, t0 + note.at);
      env.gain.exponentialRampToValueAtTime(0.5, t0 + note.at + 0.03);
      env.gain.exponentialRampToValueAtTime(0.0001, t0 + note.at + note.length);
      osc.connect(env); env.connect(e.gain); osc.start(t0 + note.at); osc.stop(t0 + note.at + note.length + 0.05);
    }
  } catch { /* silent */ } };
  if (e.context.state === "suspended") void e.context.resume().then(play).catch(() => {}); else play();
}
