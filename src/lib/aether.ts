import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";

export type Platform = "Steam" | "Epic" | "Riot" | "Custom";
export type View = "poster" | "banner" | "list" | "carousel";
export interface Game {
  id: string; title: string; platform: Platform; target: string; args: string;
  workingDir: string; cover: string; banner: string; steamId: string; favorite: boolean;
}
export interface Settings {
  theme: "cream" | "navy" | "cyan"; accent: string; view: View; size: number;
  gap: number; radius: number; titles: boolean; performance: boolean;
  wallpaper: string; blur: number; dim: number; afterLaunch: "keep" | "minimize" | "close";
  folders: string[]; onboarded: boolean;
}
export interface Library { games: Game[]; settings: Settings }
export interface ScanResult { games: Game[]; warnings: string[] }
export const desktop = isTauri();
export const defaults: Settings = { theme: "cream", accent: "#5b4bd6", view: "poster", size: 170, gap: 22, radius: 14, titles: true, performance: false, wallpaper: "", blur: 0, dim: 30, afterLaunch: "minimize", folders: [], onboarded: false };
const showcase = [
  ["1245620", "ELDEN RING"], ["1091500", "Cyberpunk 2077"], ["1086940", "Baldur’s Gate 3"],
  ["1593500", "God of War"], ["1151640", "Horizon Zero Dawn"], ["1174180", "Red Dead Redemption 2"],
  ["1145360", "Hades"], ["105600", "Terraria"], ["413150", "Stardew Valley"], ["367520", "Hollow Knight"],
  ["990080", "Hogwarts Legacy"], ["292030", "The Witcher 3"],
];
export const steamImage = (id: string, banner = false) => `https://cdn.akamai.steamstatic.com/steam/apps/${id}/${banner ? "library_hero.jpg" : "library_600x900.jpg"}`;
export const imageUrl = (path: string) => !path ? "" : /^(https:|data:|blob:)/.test(path) ? path : desktop ? convertFileSrc(path) : "";
export function blankGame(): Game { return { id: crypto.randomUUID(), title: "", platform: "Custom", target: "", args: "", workingDir: "", cover: "", banner: "", steamId: "", favorite: false }; }
export const gameIdentity = (game: Game) => `${game.target.split("/").join(String.fromCharCode(92)).toLowerCase()}|${game.args.trim()}`;
const previewKey = "aether-preview-v1";
export async function loadLibrary(): Promise<Library> {
  if (desktop) return invoke("load_library");
  try { const saved = localStorage.getItem(previewKey); if (saved) { const data = JSON.parse(saved); return { games: data.games, settings: { ...defaults, ...data.settings } }; } } catch { /* A fresh isolated preview is safe if browser storage is unavailable. */ }
  return { games: showcase.map(([id, title], i) => ({ ...blankGame(), id: `showcase-${id}`, title, platform: "Steam", target: `steam://rungameid/${id}`, steamId: id, cover: steamImage(id), banner: steamImage(id, true), favorite: i < 3 })), settings: { ...defaults, onboarded: true } };
}
export async function saveLibrary(library: Library): Promise<void> {
  if (desktop) return invoke("save_library", { library });
  localStorage.setItem(previewKey, JSON.stringify(library));
}
export async function pickFile(kind: "exe" | "image" | "folder"): Promise<string | null> {
  if (!desktop) throw new Error("Native file and folder pickers are available in the Windows app. In this preview, enter a path or use the browser artwork picker.");
  return invoke("pick_file", { kind });
}
export async function importArtwork(path: string): Promise<string> { return invoke("import_artwork", { path }); }
export async function scanGames(folders: string[]): Promise<ScanResult> {
  if (!desktop) throw new Error("PC scanning requires the Windows desktop app. This browser cannot read Steam, Epic, or Riot installation files.");
  return invoke("scan_games", { folders });
}
export async function launchGame(id: string): Promise<void> {
  if (!desktop) throw new Error("This is an artwork showcase, not your installed library. Launching is available in the Windows desktop app.");
  return invoke("launch_game", { id });
}
export async function exportShortcuts(): Promise<number | null> {
  if (!desktop) throw new Error("Genuine Windows .lnk files require the desktop app. No shortcuts have been created.");
  return invoke("export_shortcuts");
}
export async function gridArtwork(title: string, key: string): Promise<{ id: number; name: string; url: string }[]> {
  if (!desktop) throw new Error("SteamGridDB lookup is available in the desktop app; your key is not sent from this preview.");
  return invoke("grid_artwork", { title, key });
}
