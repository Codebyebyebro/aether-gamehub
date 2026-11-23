/** Games whose file cannot be found. The program decides; this only names the answer and words the messages. */

/** `file`: the drive is there but the file is not. `drive`: the drive itself is not connected. */
export type Absence = "file" | "drive";
export interface Absent { id: string; reason: Absence }
export type AbsentMap = Readonly<Record<string, Absence>>;

/** Keeps only reasons this version understands, so an unknown one from a newer program shows nothing instead of a wrong badge. */
export function toAbsentMap(list: readonly { id: string; reason: string }[]): AbsentMap {
  const map: Record<string, Absence> = {};
  for (const item of list) if (item.reason === "file" || item.reason === "drive") map[item.id] = item.reason;
  return map;
}

/** True when two answers say the same thing, so an unchanged re-check does not redraw the library. */
export function sameAbsent(a: AbsentMap, b: AbsentMap): boolean {
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every(id => a[id] === b[id]);
}

/** Games that are really gone. A disconnected drive is not counted: that game is fine, it only cannot be reached now. */
export function countMissing(map: AbsentMap): number { return Object.values(map).filter(reason => reason === "file").length; }

/** Changes whenever a game is added, removed or pointed somewhere else, and not when it is only favourited or retitled. */
export function targetKey(games: readonly { id: string; target: string }[]): string { return games.map(g => `${g.id}\t${g.target}`).join("\n"); }

export function absenceNotice(title: string, reason: Absence, bigBox: boolean): string {
  if (reason === "drive") return `${title} is on a drive that isn't connected. Connect it and try again.`;
  return bigBox
    ? `${title} can't be found. Leave Big Box to choose where it moved to, or to remove it.`
    : `${title} can't be found. Its file was moved, renamed or uninstalled. Choose where it is now, or remove it from your library.`;
}
