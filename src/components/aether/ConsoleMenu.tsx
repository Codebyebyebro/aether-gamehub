import { useEffect, useRef, useState } from "react";
import { useController } from "@/hooks/useController";
import PadHints from "@/components/aether/PadHints";
import { stepIndex } from "@/lib/power";
import { playCue } from "@/lib/cue";

export interface MenuItem { id: string; label: string; hint?: string; confirm?: { title: string; body: string; action: string } }

/**
 * A console-style menu over everything (Big Box included): Up/Down move, Cross chooses, Circle goes back. A choice
 * with `confirm` asks first, with Cancel selected, so a double tap can never restart the PC. Keyboard and mouse work
 * the same way. `onMenu` and `onHome` say what Options and the PS button do while it is open.
 */
export default function ConsoleMenu({ label, heading, sub, items, choose, close, onMenu, onHome, busy = false }: {
  label: string; heading: React.ReactNode; sub?: React.ReactNode; items: readonly MenuItem[];
  choose: (id: string) => void; close: () => void; onMenu?: () => void; onHome?: () => void; busy?: boolean;
}) {
  const [index, setIndex] = useState(0);
  const [asking, setAsking] = useState<MenuItem | null>(null);
  const [yes, setYes] = useState(false); // in the confirm step: false = Cancel selected
  const list = useRef<HTMLDivElement>(null);

  const pick = (item: MenuItem) => { if (busy) return; if (item.confirm) { setAsking(item); setYes(false); } else choose(item.id); };
  const back = () => { if (asking) setAsking(null); else close(); };
  const accept = () => { if (busy) return; if (asking) { if (yes) choose(asking.id); else setAsking(null); } else if (items[index]) pick(items[index]); };
  const move = (step: number) => { playCue("move"); if (asking) setYes(y => !y); else setIndex(i => stepIndex(i, step, items.length)); };

  useController({ up: () => move(-1), down: () => move(1), left: () => { if (asking && yes) { setYes(false); playCue("move"); } }, right: () => { if (asking && !yes) { setYes(true); playCue("move"); } }, confirm: accept, back, menu: onMenu ?? close, home: onHome ?? close });
  // Focus follows the selection, so Enter and the mouse agree with the controller, and screen readers hear it.
  useEffect(() => { list.current?.querySelector<HTMLElement>(asking ? `[data-answer="${yes ? "yes" : "no"}"]` : `[data-item="${index}"]`)?.focus({ preventScroll: true }); }, [index, asking, yes]);
  // Captured on the window so Esc or Enter here never also reaches Big Box underneath.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const handled = ["Escape", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Enter", "Tab"].includes(e.key);
      if (!handled) return;
      e.preventDefault(); e.stopImmediatePropagation();
      if (e.repeat && e.key === "Enter") return;
      if (e.key === "Escape") back();
      else if (e.key === "ArrowUp" || (e.key === "Tab" && e.shiftKey)) move(-1);
      else if (e.key === "ArrowDown" || e.key === "Tab") move(1);
      else if (e.key === "ArrowLeft" && asking) setYes(false);
      else if (e.key === "ArrowRight" && asking) setYes(true);
      else if (e.key === "Enter") accept();
    };
    window.addEventListener("keydown", key, { capture: true });
    return () => window.removeEventListener("keydown", key, { capture: true });
  });

  return <div role="dialog" aria-modal="true" aria-label={label} className="console-menu fixed inset-0 z-[70] flex items-center justify-center p-6 text-white" onClick={e => { if (e.target === e.currentTarget) back(); }}>
    <div ref={list} className="console-menu-panel w-full max-w-md rounded-3xl p-6">
      {!asking ? <>
        <div className="mb-5">{typeof heading === "string" ? <h2 className="display-font text-2xl font-extrabold">{heading}</h2> : heading}{sub && <div className="mt-1 text-[13px] text-white/70">{sub}</div>}</div>
        <div className="space-y-1.5">{items.map((item, i) => <button key={item.id} data-item={i} disabled={busy} onClick={() => { setIndex(i); pick(item); }} onMouseEnter={() => setIndex(i)} className={`console-menu-item ${i === index ? "is-on" : ""}`}>
          <span className="block text-[15px] font-semibold">{item.label}</span>{item.hint && <span className="block text-[12px] text-white/60">{item.hint}</span>}
        </button>)}</div>
        <div className="mt-5 text-[11px] text-white/60"><PadHints items={[["▲ ▼", "Move"], ["✕", "Choose"], ["○", "Back"]]} /></div>
      </> : <>
        <h2 className="display-font text-2xl font-extrabold">{asking.confirm!.title}</h2>
        <p className="mt-2 text-[13px] leading-relaxed text-white/75">{asking.confirm!.body}</p>
        <div className="mt-6 flex gap-3">
          <button data-answer="no" onClick={() => setAsking(null)} onMouseEnter={() => setYes(false)} className={`console-menu-answer ${!yes ? "is-on" : ""}`}>Cancel</button>
          <button data-answer="yes" disabled={busy} onClick={() => choose(asking.id)} onMouseEnter={() => setYes(true)} className={`console-menu-answer ${yes ? "is-on" : ""}`}>{busy ? "One moment…" : asking.confirm!.action}</button>
        </div>
        <div className="mt-5 text-[11px] text-white/60"><PadHints items={[["◀ ▶", "Choose"], ["✕", "OK"], ["○", "Back"]]} /></div>
      </>}
    </div>
  </div>;
}
