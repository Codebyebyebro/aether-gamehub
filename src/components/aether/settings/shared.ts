import { toast } from "sonner";

/** Backend errors arrive as plain strings, adapter errors as Error objects. */
export const message = (e: unknown) => e instanceof Error ? e.message : String(e);
export const fail = (e: unknown) => toast.error(message(e));
export const fieldClass = "aether-field h-9 w-full border px-3 text-xs outline-none focus:border-[var(--aether-accent)] disabled:opacity-50";
export const cardClass = "rounded-xl border p-4";
