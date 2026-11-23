/** "Living colors": which picture a game's colour comes from, and how it is handed to the stylesheet. */

export interface Rgb { r: number; g: number; b: number }
type Art = { cover: string; banner: string; steamId: string };

/**
 * The artwork reference to take the colour from: the cover, else the banner, else the Steam cover for a Steam id.
 * Pictures that only live in the browser (data: or blob:) have nothing the program can read, so they give none.
 */
export function tintSource(game: Art | null | undefined, steamCover: (id: string) => string): string {
  if (!game) return "";
  const own = game.cover.trim() || game.banner.trim();
  if (own) return /^(data|blob):/.test(own) ? "" : own;
  return /^\d{1,12}$/.test(game.steamId.trim()) ? steamCover(game.steamId.trim()) : "";
}

/** The value of `--game-tint`: space-separated channels, as `rgb(var(--game-tint) / 0.2)` expects. Rejects anything odd. */
export function tintValue(color: Rgb | null | undefined): string {
  if (!color) return "";
  const parts = [color.r, color.g, color.b];
  if (!parts.every(n => Number.isInteger(n) && n >= 0 && n <= 255)) return "";
  return parts.join(" ");
}

/**
 * Which game the interface should be coloured by. A game shown in a window wins, then the one Big Box has selected,
 * then the one the pointer or controller is on, then the spotlight.
 */
export function tintTarget<T extends { id: string }>(opts: { detail?: T | null; bigBox?: T | null; pointed?: T | null; spotlight?: T | null }): T | null {
  return opts.detail ?? opts.bigBox ?? opts.pointed ?? opts.spotlight ?? null;
}
