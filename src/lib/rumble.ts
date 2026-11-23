/** A short controller rumble, through the browser's Gamepad vibration (Xbox controllers; PlayStation ones where Windows allows). */

export interface Rumble { duration: number; strong: number; weak: number }
/** Launching a game: a firm, short double motor pulse. */
export const LAUNCH_RUMBLE: Rumble = { duration: 220, strong: 0.7, weak: 0.45 };

type Actuator = { playEffect?: (type: string, params: Record<string, number>) => Promise<unknown> };
type PadLike = { connected: boolean; vibrationActuator?: Actuator | null } | null;

/** The effect parameters, kept inside what the browser accepts (0–1 strength, at most 5 seconds). */
export function rumbleParams(r: Rumble): { duration: number; startDelay: number; strongMagnitude: number; weakMagnitude: number } {
  const unit = (n: number) => (Number.isFinite(n) ? Math.min(1, Math.max(0, n)) : 0);
  return { duration: Math.round(Math.min(5000, Math.max(0, Number.isFinite(r.duration) ? r.duration : 0))), startDelay: 0, strongMagnitude: unit(r.strong), weakMagnitude: unit(r.weak) };
}

/** Rumbles every connected controller that can. Returns how many were asked; never throws. */
export function rumble(pads: readonly PadLike[], r: Rumble = LAUNCH_RUMBLE): number {
  const params = rumbleParams(r);
  if (!params.duration || (!params.strongMagnitude && !params.weakMagnitude)) return 0;
  let asked = 0;
  for (const pad of pads) {
    const play = pad?.connected ? pad.vibrationActuator?.playEffect : undefined;
    if (!play) continue;
    try { void play.call(pad!.vibrationActuator, "dual-rumble", params)?.catch?.(() => {}); asked++; } catch { /* a pad that refuses just stays still */ }
  }
  return asked;
}
/** The window's own controllers (empty where the browser has no Gamepad API). */
export function windowPads(): PadLike[] {
  return typeof navigator !== "undefined" && typeof navigator.getGamepads === "function" ? Array.from(navigator.getGamepads()) as PadLike[] : [];
}
