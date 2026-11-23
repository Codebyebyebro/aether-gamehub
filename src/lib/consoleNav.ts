/** Big Box layout rules shared by the console home screen, kept pure so they are tested. */

export type ConsoleAction = "play" | "details" | "favorite" | "back";
/** The buttons under the title, left to right, on the home screen and on the details page. */
export const HOME_ACTIONS: readonly ConsoleAction[] = ["play", "details", "favorite"];
export const DETAIL_ACTIONS: readonly ConsoleAction[] = ["play", "favorite", "back"];
/** The top bar, reached with Up from the games: everything that used to need the mouse. */
export type HeaderAction = "search" | "scan" | "settings" | "exit";
export const HEADER_ACTIONS: readonly HeaderAction[] = ["search", "scan", "settings", "exit"];
/** Big Box search: games whose title contains every word typed, ignoring case and accents. */
export function searchGames<T extends { title: string }>(games: readonly T[], query: string): T[] {
  const fold = (s: string) => s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLocaleLowerCase();
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (!words.length) return [...games];
  return games.filter(g => { const title = fold(g.title); return words.every(w => title.includes(w)); });
}

/** Tile sizes in the row (CSS pixels): must match `.console-tile` in globals.css and the row's padding and gap. */
export const TILE = { width: 92, selected: 126, gap: 12, padding: 32 } as const;

/**
 * How far the row scrolls so tile `index` ends up centred. Worked out from the final layout (every tile before the
 * selected one is small), never measured mid-animation, so the scroll position is steady while tiles grow and shrink.
 */
export function rowScroll(index: number, viewWidth: number): number {
  if (index < 0 || !Number.isFinite(viewWidth)) return 0;
  const left = TILE.padding + index * (TILE.width + TILE.gap);
  return Math.max(0, Math.round(left + TILE.selected / 2 - viewWidth / 2));
}

/**
 * Whether the window stays full screen when Big Box closes: yes if it already was before Big Box opened (the player
 * chose it, for example with F11), or if they asked to stay full screen, or Big Box is only stepping aside for a dialog.
 */
export function fullscreenAfterBigBox(wasFullscreenBefore: boolean, keep: boolean): boolean { return wasFullscreenBefore || keep; }
