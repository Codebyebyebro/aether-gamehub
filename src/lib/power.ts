/** The console power menu (Options button): what it offers and which choices ask "are you sure?" first. */

export type PowerChoice = "desktop" | "quit" | "sleep" | "restart" | "shutdown";
export interface PowerItem { id: PowerChoice; label: string; hint: string; confirm?: { title: string; body: string; action: string } }

export const POWER_ITEMS: readonly PowerItem[] = [
  { id: "desktop", label: "Exit to desktop", hint: "Leave Big Box and minimize Aether Hub" },
  { id: "quit", label: "Quit Aether Hub", hint: "Close the launcher completely" },
  { id: "sleep", label: "Sleep", hint: "Put the PC to sleep", confirm: { title: "Put your PC to sleep?", body: "Games and apps stay open and continue where you left them.", action: "Sleep" } },
  { id: "restart", label: "Restart", hint: "Restart the PC", confirm: { title: "Restart your PC?", body: "Anything unsaved in other apps may be lost. Running games are closed by Windows.", action: "Restart" } },
  { id: "shutdown", label: "Shut down", hint: "Turn the PC off", confirm: { title: "Shut down your PC?", body: "Anything unsaved in other apps may be lost. Running games are closed by Windows.", action: "Shut down" } },
];

/** What the program is asked to do for a choice: one of its three fixed power actions, or nothing (handled in the app). */
export function nativeAction(choice: PowerChoice): "sleep" | "restart" | "shutdown" | null {
  return choice === "sleep" || choice === "restart" || choice === "shutdown" ? choice : null;
}

export type QuickChoice = "resume" | "close" | "home" | "power";
/** What the PS button opens while a game is running. Closing the game asks first; it can lose unsaved progress. */
export function quickItems(title: string): (Omit<PowerItem, "id"> & { id: QuickChoice })[] {
  return [
    { id: "resume", label: "Resume game", hint: `Back to ${title}` },
    { id: "home", label: "Home", hint: "Big Box, with the game still running" },
    { id: "close", label: "Close game", hint: "Quit the game", confirm: { title: `Close ${title}?`, body: "Anything you haven't saved in the game will be lost.", action: "Close game" } },
    { id: "power", label: "Power", hint: "Sleep, restart or shut down" },
  ];
}

/** Moving up and down the menu wraps around, like a console menu. */
export function stepIndex(index: number, step: number, count: number): number {
  if (count <= 0) return 0;
  return ((index + step) % count + count) % count;
}
