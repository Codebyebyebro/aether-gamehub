/** "Find missing artwork": which games to look up, and how the program's answer is put onto the library. */
import type { Game } from "./aether.ts";

/** A cover the program found and already cached. `matched` is the store's name for the game. */
export interface ArtFound { id: string; cover: string; banner: string; source: string; matched: string }
/** A game the program left alone, with a plain reason. */
export interface ArtUnmatched { id: string; title: string; reason: string }
export interface ArtOutcome { found: ArtFound[]; unmatched: ArtUnmatched[]; cancelled: boolean }
/** The `aether:artwork-progress` event: `done` of `total` games looked up, `title` is the one being looked up now. */
export interface ArtProgress { done: number; total: number; title: string }

/** A game shows a plain placeholder when it has no cover and no Steam app id to fall back on. */
export function needsArtwork(game: Pick<Game, "cover" | "steamId" | "title">): boolean {
  return !game.cover.trim() && !game.steamId.trim() && !!game.title.trim();
}

/** What is sent to the program: only the id and title, nothing about where the game is installed. */
export function wantedFrom(games: readonly Game[]): { id: string; title: string }[] {
  return games.filter(needsArtwork).map(g => ({ id: g.id, title: g.title.trim() }));
}

/**
 * Puts found covers onto the games that still have none. A cover the player chose while the lookup ran is kept,
 * and an existing banner is never replaced. Returns the new list and how many games changed.
 */
export function applyArtwork(games: readonly Game[], found: readonly ArtFound[]): { games: Game[]; applied: number } {
  const byId = new Map(found.filter(f => f.cover).map(f => [f.id, f]));
  let applied = 0;
  const next = games.map(game => {
    const hit = byId.get(game.id);
    if (!hit || game.cover.trim()) return game;
    applied++;
    return { ...game, cover: hit.cover, banner: game.banner.trim() ? game.banner : hit.banner };
  });
  return { games: applied ? next : [...games], applied };
}

/** The headline after a run, in plain words. */
export function artworkSummary(outcome: Pick<ArtOutcome, "cancelled">, applied: number, lookedUp: number): string {
  const covers = `${applied} cover${applied === 1 ? "" : "s"}`;
  if (outcome.cancelled) return applied ? `Stopped. ${covers} added before stopping.` : "Stopped. No covers were added.";
  if (!lookedUp) return "Every game already has artwork.";
  if (applied === lookedUp) return `Done! Found ${applied === 1 ? "a cover" : `covers for all ${applied} games`}.`;
  if (!applied) return "No covers could be found for certain.";
  return `Found ${applied === 1 ? "a cover" : "covers"} for ${applied} of ${lookedUp} games.`;
}

/** 0–100 for the progress bar. */
export function artworkPercent(progress: ArtProgress | null): number {
  if (!progress || progress.total <= 0) return 0;
  return Math.max(0, Math.min(100, Math.round((progress.done / progress.total) * 100)));
}
