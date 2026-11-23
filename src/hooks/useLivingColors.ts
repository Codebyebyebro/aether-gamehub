import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Game, desktop, steamImage } from "@/lib/aether";
import { Rgb, tintSource, tintValue } from "@/lib/tint";

// One answer per picture for the whole session; a picture asked about twice at once is only read once.
const known = new Map<string, Rgb | null>();
const asking = new Map<string, Promise<Rgb | null>>();
/** The colour the program picked for this artwork, or null if it could not read it. Never throws. */
export function colorOf(source: string): Promise<Rgb | null> {
  if (known.has(source)) return Promise.resolve(known.get(source) ?? null);
  let answer = asking.get(source);
  if (!answer) {
    answer = invoke<Rgb>("artwork_color", { reference: source }).catch(() => null).then(color => { known.set(source, color); asking.delete(source); return color; });
    asking.set(source, answer);
  }
  return answer;
}
/** Warms the colours of games likely to be looked at next (the spotlight, Big Box neighbours), a few at a time. */
export function prefetchColors(games: readonly (Game | undefined)[]) {
  if (!desktop) return;
  for (const game of games.slice(0, 6)) { const source = tintSource(game, steamImage); if (source) void colorOf(source); }
}

/**
 * Colours the interface by `game`'s artwork while `enabled`. The previous colour stays until the next one is known,
 * so moving between games drifts instead of flashing; a game with no artwork lets the tint fade out.
 */
export function useLivingColors(enabled: boolean, game: Game | null) {
  const source = enabled && desktop ? tintSource(game, steamImage) : "";
  useEffect(() => {
    const root = document.documentElement;
    if (!source) { root.dataset.tint = "off"; return; }
    let live = true;
    colorOf(source).then(color => {
      if (!live) return;
      const value = tintValue(color);
      if (value) { root.style.setProperty("--game-tint", value); root.dataset.tint = "on"; } else root.dataset.tint = "off";
    });
    return () => { live = false; };
  }, [source]);
  useEffect(() => () => { document.documentElement.dataset.tint = "off"; }, []);
}
