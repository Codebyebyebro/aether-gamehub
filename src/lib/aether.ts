import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { launchIdentity } from "@/lib/paths";

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
  /** Automatic backups of the library, playtime and cover references. An empty folder means the app's own. */
  backupEnabled: boolean; backupDir: string; backupKeep: number;
  /** Discord Rich Presence needs the player's own Discord application id. */
  discordEnabled: boolean; discordAppId: string;
  /** Steam playtime import from a public profile. The API key is optional. */
  steamSyncEnabled: boolean; steamProfile: string; steamApiKey: string; steamSyncHours: number;
}
export interface Library { games: Game[]; settings: Settings }
/** What the scan review shows about a candidate; never saved with the game. */
export interface ScanDetail { sizeBytes: number | null; path: string; source: "Steam" | "Epic" | "Riot" | "Folder" }
export interface FilteredItem { game: Game; detail: ScanDetail; reason: string }
export interface ScanStats { executablesSeen: number; tooSmall: number; blacklisted: number; knownTools: number; alreadyManaged: number; duplicates: number; installers: number; programs: number; noGameFiles: number }
/** `cancelled` means the player stopped the scan; everything else is then empty. */
export interface ScanResult { games: Game[]; warnings: string[]; details: Record<string, ScanDetail>; filtered: FilteredItem[]; stats: ScanStats; cancelled: boolean }
/** The `aether:scan-progress` event. `step` is 0 while store lists are read, then one per custom folder, and `steps` when done. */
export interface ScanProgress { scanId: string; step: number; steps: number; checked: number; found: number; folder: string }
export const desktop = isTauri();
export const defaults: Settings = { theme: "cream", accent: "#5b4bd6", view: "poster", size: 170, gap: 22, radius: 14, titles: true, performance: false, wallpaper: "", blur: 0, dim: 30, afterLaunch: "minimize", folders: [], onboarded: false, backupEnabled: true, backupDir: "", backupKeep: 10, discordEnabled: false, discordAppId: "", steamSyncEnabled: false, steamProfile: "", steamApiKey: "", steamSyncHours: 6 };
const showcase = [
  ["1245620", "ELDEN RING"], ["1091500", "Cyberpunk 2077"], ["1086940", "Baldur’s Gate 3"],
  ["1593500", "God of War"], ["1151640", "Horizon Zero Dawn"], ["1174180", "Red Dead Redemption 2"],
  ["1145360", "Hades"], ["105600", "Terraria"], ["413150", "Stardew Valley"], ["367520", "Hollow Knight"],
  ["990080", "Hogwarts Legacy"], ["292030", "The Witcher 3"],
];
export const steamImage = (id: string, banner = false) => `https://cdn.akamai.steamstatic.com/steam/apps/${id}/${banner ? "library_hero.jpg" : "library_600x900.jpg"}`;
export const imageUrl = (path: string) => !path ? "" : /^(https:|data:|blob:)/.test(path) ? path : desktop ? convertFileSrc(path) : "";
export function blankGame(): Game { return { id: crypto.randomUUID(), title: "", platform: "Custom", target: "", args: "", workingDir: "", cover: "", banner: "", steamId: "", favorite: false }; }
export const gameIdentity = (game: Game) => launchIdentity(game.target, game.args);
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
/** `scanId` tags the progress events and is what `cancelScan` stops; use a fresh one for every scan. */
export async function scanGames(folders: string[], scanId: string): Promise<ScanResult> {
  if (!desktop) throw new Error("PC scanning requires the Windows desktop app. This browser cannot read Steam, Epic, or Riot installation files.");
  return invoke("scan_games", { folders, scanId });
}
/** Asks a running scan to stop; its `scanGames` call then resolves with `cancelled: true`. The preview never scans. */
export async function cancelScan(scanId: string): Promise<void> {
  if (!desktop) return;
  return invoke("cancel_scan", { scanId });
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

/* ---------- Playtime, backups, Discord and Steam sync (desktop app only) ---------- */
export interface PlaytimeEntry { seconds: number; lastPlayed: number }
export interface SteamSyncState { syncedAt: number; attemptedAt: number; matched: number; owned: number; error: string }
export interface PlaytimeStore { version: number; games: Record<string, PlaytimeEntry>; steam: SteamSyncState }
export const emptyPlaytime: PlaytimeStore = { version: 1, games: {}, steam: { syncedAt: 0, attemptedAt: 0, matched: 0, owned: 0, error: "" } };
export interface BackupInfo { name: string; path: string; sizeBytes: number; createdAt: number }
export interface BackupOverview { dir: string; backups: BackupInfo[]; lastError: string }
export interface BackupMade { info: BackupInfo; warnings: string[] }
export interface BackupExported { path: string; warnings: string[] }
export interface Restored { library: Library; playtime: PlaytimeStore; games: number; restoredCovers: number; missingCovers: number }
export interface DiscordStatus { enabled: boolean; connected: boolean; playing: string; error: string }
export interface SteamSyncSummary { owned: number; matched: number; raised: number; usedApi: boolean }
export interface SessionEvent { id: string; title: string; startedAt: number }
export interface PlaytimeEvent { id: string; seconds: number; lastPlayed: number }

function desktopOnly(what: string): never { throw new Error(`${what} needs the Windows desktop app. This browser preview cannot read or write your PC.`); }
/** Reading is passive, so the preview simply has nothing recorded rather than inventing numbers. */
export async function loadPlaytime(): Promise<PlaytimeStore> { return desktop ? invoke("load_playtime") : emptyPlaytime; }
export async function startupNotice(): Promise<string | null> { return desktop ? invoke("startup_notice") : null; }
export async function backupOverview(dir: string): Promise<BackupOverview> { if (!desktop) desktopOnly("Backups"); return invoke("backup_overview", { dir }); }
export async function backupNow(dir: string, keep: number, includeCovers: boolean): Promise<BackupMade> { if (!desktop) desktopOnly("Backups"); return invoke("backup_now", { dir, keep, includeCovers }); }
export async function backupExport(): Promise<BackupExported | null> { if (!desktop) desktopOnly("Exporting a backup"); return invoke("backup_export"); }
/** `path` restores one of the listed backups; without it the player picks any backup file. Null means they cancelled. */
export async function backupRestore(path?: string): Promise<Restored | null> { if (!desktop) desktopOnly("Restoring a backup"); return invoke("backup_restore", { path: path ?? null }); }
export async function openBackupFolder(dir: string): Promise<void> { if (!desktop) desktopOnly("Opening the backup folder"); return invoke("open_backup_folder", { dir }); }
export async function discordStatus(): Promise<DiscordStatus> { return desktop ? invoke("discord_status") : { enabled: false, connected: false, playing: "", error: "" }; }
export async function discordTest(appId: string): Promise<void> { if (!desktop) desktopOnly("The Discord test"); return invoke("discord_test", { appId }); }
export async function steamSyncNow(profile: string, apiKey: string): Promise<SteamSyncSummary> { if (!desktop) desktopOnly("Steam sync"); return invoke("steam_sync_now", { profile, apiKey }); }
/** Subscribe to a backend event. Resolves to the function that unsubscribes; in the browser preview it does nothing. */
export async function listenDesktop<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
  if (!desktop) return () => {};
  return listen<T>(event, e => handler(e.payload));
}
