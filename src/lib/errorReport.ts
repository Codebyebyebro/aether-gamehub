/** Sends uncaught interface errors to the local log: a few of them, each once, and never anything but the error text. */

/** After this many reports in one session the rest are dropped, so one error in a loop cannot flood the log. */
export const MAX_REPORTS = 20;
export const MAX_LENGTH = 400;
/** Errors the browser raises about itself and that mean nothing is broken. */
const HARMLESS = [/^ResizeObserver loop/i];

export function createReporter(send: (message: string) => void, limit = MAX_REPORTS): (message: string) => boolean {
  let sent = 0; let last = "";
  return message => {
    const text = message.replace(/\s+/g, " ").trim().slice(0, MAX_LENGTH);
    if (!text || text === last || sent >= limit || HARMLESS.some(rule => rule.test(text))) return false;
    last = text; sent += 1;
    try { send(text); } catch { /* reporting must never become a second error */ }
    return true;
  };
}

/** `Name: message` plus the first line of the stack that has a position, which says where it happened. */
export function describeError(reason: unknown): string {
  if (reason instanceof Error) {
    const frame = (reason.stack ?? "").split("\n").find(line => /:\d+:\d+/.test(line));
    return `${reason.name}: ${reason.message}${frame ? ` ${frame.trim()}` : ""}`;
  }
  if (typeof reason === "string") return reason;
  try { return JSON.stringify(reason) ?? String(reason); } catch { return String(reason); }
}

export function installErrorReporting(target: Window, send: (message: string) => void): void {
  const report = createReporter(send);
  target.addEventListener("error", event => {
    const where = event.filename ? ` (${event.filename.split("/").pop()}:${event.lineno})` : "";
    report(`${describeError(event.error ?? event.message)}${where}`);
  });
  target.addEventListener("unhandledrejection", event => report(`Unhandled promise rejection: ${describeError(event.reason)}`));
}
