/** Small key-cap legend ("✕ Play  ○ Back") shown while a controller is connected. */
export default function PadHints({ items, className = "" }: { items: readonly (readonly [string, string])[]; className?: string }) {
  return <span className={`flex flex-wrap items-center gap-x-4 gap-y-1 ${className}`} aria-label="Controller buttons">{items.map(([key, label]) => <span key={key} className="flex items-center gap-1.5"><kbd className="pad-key">{key}</kbd>{label}</span>)}</span>;
}
