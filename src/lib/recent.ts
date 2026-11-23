/** "Jump back in": the spotlight, the Recently played row and the play-time sorts, from the playtime the program records. */
import type { Game, PlaytimeEntry } from "./aether.ts";
import { formatAgo, formatPlaytime } from "./format.ts";

export type Sort = "added" | "az" | "recent" | "played";
export const sortLabels: Record<Sort, string> = { added: "Date added", az: "Title A–Z", recent: "Recently played", played: "Most played" };
type Times = Readonly<Record<string, PlaytimeEntry | undefined>>;

const last = (times: Times, id: string) => times[id]?.lastPlayed ?? 0;
const spent = (times: Times, id: string) => times[id]?.seconds ?? 0;

/**
 * The library in the chosen order. Games never played keep their place after the played ones, in the order they were
 * added, so a fresh library sorted by play time looks the same as by date added. The input is not changed.
 */
export function sortGames(games: readonly Game[], sort: Sort, times: Times): Game[] {
  const list = [...games];
  if (sort === "az") return list.sort((a, b) => a.title.localeCompare(b.title));
  if (sort === "recent") return list.sort((a, b) => last(times, b.id) - last(times, a.id));
  if (sort === "played") return list.sort((a, b) => spent(times, b.id) - spent(times, a.id) || last(times, b.id) - last(times, a.id));
  return list;
}

/** Games played at least once, most recent first. */
export function recentlyPlayed(games: readonly Game[], times: Times, limit = 10): Game[] {
  return games.filter(g => last(times, g.id) > 0).sort((a, b) => last(times, b.id) - last(times, a.id)).slice(0, limit);
}

export type SpotlightMode = "playing" | "recent" | "featured";
export interface Spotlight { games: Game[]; mode: SpotlightMode }
/**
 * Up to three games for the spotlight, with why they are there. A game running right now comes first; otherwise the
 * most recently played; a library with nothing played yet shows its first games as before.
 */
export function spotlight(games: readonly Game[], times: Times, playingId?: string | null): Spotlight {
  const playing = playingId ? games.find(g => g.id === playingId) : undefined;
  const recent = recentlyPlayed(games, times, 3);
  if (playing) return { mode: "playing", games: [playing, ...recent.filter(g => g.id !== playing.id)].slice(0, 3) };
  if (recent.length) return { mode: "recent", games: recent };
  return { mode: "featured", games: games.slice(0, 3) };
}

export const spotlightLabel: Record<SpotlightMode, string> = { playing: "Now playing", recent: "Jump back in", featured: "In the spotlight" };

/** "2.5 h played · last played yesterday" for a spotlight or row tile; empty for a game never played. */
export function playedLine(entry: PlaytimeEntry | undefined, nowMs = Date.now()): string {
  if (!entry?.lastPlayed && !entry?.seconds) return "";
  const time = entry.seconds ? `${formatPlaytime(entry.seconds)} played` : "";
  const when = entry.lastPlayed ? `last played ${agoWords(entry.lastPlayed, nowMs)}` : "";
  return [time, when].filter(Boolean).join(" · ");
}
/** formatAgo, but "yesterday" reads better than "1 day ago" in a sentence. */
export function agoWords(unixSeconds: number, nowMs = Date.now()): string {
  const text = formatAgo(unixSeconds, nowMs);
  return text === "1 day ago" ? "yesterday" : text;
}
