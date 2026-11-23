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

/** Set while a woken audio device is resuming, so only one click waits for it (never a backlog). */
let waking = false;
/**
 * Plays the selection click. Silent when switched off or too soon after the last one. If Windows paused the audio
 * device, it is woken and this one click plays as soon as it is running; clicks meanwhile are dropped, not queued.
 */
export function playSelectCue(now: number = performance.now()) {
  if (!enabled || !gate(now)) return;
  const e = start(); if (!e) return;
  const play = () => void e.buffer.then(buffer => {
    if (!buffer || e.context.state !== "running") return;
    try { const source = e.context.createBufferSource(); source.buffer = buffer; source.connect(e.gain); source.start(); } catch { /* silent */ }
  });
  if (e.context.state !== "suspended") { play(); return; }
  if (waking) return;
  waking = true;
  void e.context.resume().then(() => { waking = false; play(); }, () => { waking = false; });
}

/** One synthesised tone: a sine (or `type`) from `hz` (gliding to `to`), starting `at` seconds in, fading over `length`. */
export interface Tone { hz: number; at: number; length: number; to?: number; type?: OscillatorType; level?: number }

/** Console mode's start-up chime: three soft rising notes (C5, G5, C6), each fading out, about 1.4 s in all. */
export const BOOT_NOTES: readonly Tone[] = [
  { hz: 523.25, at: 0, length: 0.9 }, { hz: 783.99, at: 0.16, length: 0.9 }, { hz: 1046.5, at: 0.32, length: 1.1 },
];
/** Launching a game: a quick bright rise, under a second. */
export const LAUNCH_NOTES: readonly Tone[] = [
  { hz: 659.25, at: 0, length: 0.35 }, { hz: 987.77, at: 0.07, length: 0.45 }, { hz: 1318.51, at: 0.14, length: 0.6 },
];

/**
 * The console's sound set. `move` is the recorded click; the others are short synthesised tones, each with its own
 * character, so you can hear what happened without looking: select rises, back falls, a category switch ticks twice,
 * opening a page or menu pops upward, and a favorite sparkles.
 */
export type CueKind = "move" | "confirm" | "back" | "tab" | "open" | "favorite";
export const CUE_TONES: Readonly<Record<Exclude<CueKind, "move">, readonly Tone[]>> = {
  confirm: [{ hz: 880, at: 0, length: 0.09, level: 0.32 }, { hz: 1318.51, at: 0.045, length: 0.14, level: 0.32 }],
  back: [{ hz: 1046.5, at: 0, length: 0.08, level: 0.26 }, { hz: 659.25, at: 0.045, length: 0.14, level: 0.26 }],
  tab: [{ hz: 1567.98, at: 0, length: 0.05, type: "triangle", level: 0.22 }, { hz: 2093, at: 0.05, length: 0.06, type: "triangle", level: 0.22 }],
  open: [{ hz: 440, to: 990, at: 0, length: 0.16, level: 0.28 }],
  favorite: [{ hz: 2093, at: 0, length: 0.08, level: 0.18 }, { hz: 2637, at: 0.05, length: 0.08, level: 0.18 }, { hz: 3136, at: 0.1, length: 0.16, level: 0.18 }],
};
const gates = new Map<CueKind, (now: number) => boolean>();
/** Plays one sound of the set. Each kind has its own rate limit, so a quick "select" right after a move is still heard. */
export function playCue(kind: CueKind, now: number = performance.now()) {
  if (kind === "move") { playSelectCue(now); return; }
  if (!enabled) return;
  let gateFor = gates.get(kind); if (!gateFor) { gateFor = createCueGate(); gates.set(kind, gateFor); }
  if (!gateFor(now)) return;
  playTones(CUE_TONES[kind], 0.005);
}

/** Plays the start-up chime, synthesised (no sound file). Silent when navigation sounds are switched off. */
export function playBootChime() { playTones(BOOT_NOTES); }
/** Plays the launch sound when a game is started with the controller. Silent when navigation sounds are switched off. */
export function playLaunchCue() { playTones(LAUNCH_NOTES); }
function playTones(tones: readonly Tone[], lead = 0.05) {
  if (!enabled) return;
  const e = start(); if (!e) return;
  // An audio device that cannot make tones just stays silent; a sound is never worth an error.
  const play = () => { try {
    const t0 = e.context.currentTime + lead;
    for (const tone of tones) {
      const osc = e.context.createOscillator(); const env = e.context.createGain();
      const begin = t0 + tone.at, end = begin + tone.length, peak = tone.level ?? 0.5;
      osc.type = tone.type ?? "sine"; osc.frequency.setValueAtTime(tone.hz, begin);
      if (tone.to) osc.frequency.exponentialRampToValueAtTime(tone.to, end);
      env.gain.setValueAtTime(0.0001, begin);
      env.gain.exponentialRampToValueAtTime(peak, begin + Math.min(0.02, tone.length / 4));
      env.gain.exponentialRampToValueAtTime(0.0001, end);
      osc.connect(env); env.connect(e.gain); osc.start(begin); osc.stop(end + 0.05);
    }
  } catch { /* silent */ } };
  if (e.context.state === "suspended") void e.context.resume().then(play).catch(() => {}); else play();
}
