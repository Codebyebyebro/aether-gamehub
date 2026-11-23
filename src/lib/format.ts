/** Human-readable sizes, paths and times shared by the scan review, the editor and the settings panels. */

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"]; let value = bytes / 1024; let unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit += 1; }
  return `${value.toFixed(1)} ${units[unit]}`;
}
/** "12.5 h", "45 min", "Under a minute", "Not played yet". */
export function formatPlaytime(seconds: number | undefined): string {
  if (!seconds) return "Not played yet";
  if (seconds < 60) return "Under a minute";
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes} min`;
  const hours = minutes / 60;
  return `${hours >= 100 ? Math.round(hours).toLocaleString("en-US") : hours.toFixed(1)} h`;
}
export function formatAgo(unixSeconds: number, nowMs = Date.now()): string {
  if (!unixSeconds) return "never";
  const seconds = Math.max(0, Math.floor(nowMs / 1000 - unixSeconds));
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60); if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60); if (hours < 24) return `${hours} h ago`;
  const days = Math.floor(hours / 24); if (days < 30) return `${days} day${days === 1 ? "" : "s"} ago`;
  const months = Math.floor(days / 30); if (months < 12) return `${months} month${months === 1 ? "" : "s"} ago`;
  return `${Math.floor(days / 365)} y ago`;
}
/** Keeps the drive at the start and the folder and file name at the end, which is what people recognise. */
export function shortPath(path: string, max = 72): string {
  if (path.length <= max) return path;
  const head = Math.ceil((max - 1) * 0.3);
  return `${path.slice(0, head)}…${path.slice(path.length - (max - 1 - head))}`;
}
/** Where the scan progress bar stands, 0–1. Folders are the only steps the backend can count ahead of time. */
export function scanFraction(progress: { step: number; steps: number } | null): number {
  if (!progress || !(progress.steps > 0)) return 0;
  return Math.min(1, Math.max(0, progress.step / progress.steps));
}
/** What the scan is doing now. */
export function scanStepLabel(progress: { step: number; steps: number } | null): string {
  if (!progress || progress.step <= 0) return "Reading Steam, Epic and Riot install lists…";
  if (progress.step >= progress.steps) return "Preparing your review…";
  const folders = progress.steps - 1;
  return folders === 1 ? "Searching your folder…" : `Searching folder ${progress.step} of ${folders}…`;
}
/** "12,345 items checked · 3 games found". */
export function scanCountsLabel(progress: { checked: number; found: number } | null): string {
  if (!progress) return "Starting…";
  const items = `${progress.checked.toLocaleString("en-US")} item${progress.checked === 1 ? "" : "s"} checked`;
  return `${items} · ${progress.found.toLocaleString("en-US")} game${progress.found === 1 ? "" : "s"} found`;
}
