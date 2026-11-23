/** Big Box layout rules shared by the console home screen, kept pure so they are tested. */

export type ConsoleAction = "play" | "details" | "favorite" | "back";
/** The buttons under the title, left to right, on the home screen and on the details page. */
export const HOME_ACTIONS: readonly ConsoleAction[] = ["play", "details", "favorite"];
export const DETAIL_ACTIONS: readonly ConsoleAction[] = ["play", "favorite", "back"];

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
