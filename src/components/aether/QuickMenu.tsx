import { useEffect, useState } from "react";
import ConsoleMenu from "@/components/aether/ConsoleMenu";
import { formatPlaytime } from "@/lib/format";
import { quickItems, type QuickChoice } from "@/lib/power";

/** The quick menu over a running game: how long this session has been going, and what to do next. */
export default function QuickMenu({ game, choose, close, busy }: { game: { id: string; title: string; startedAt: number }; choose: (choice: QuickChoice) => void; close: () => void; busy?: boolean }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => { const t = setInterval(() => setNow(Date.now()), 15_000); return () => clearInterval(t); }, []);
  const played = Math.max(0, Math.floor(now / 1000 - game.startedAt));
  return <ConsoleMenu label="Quick menu" busy={busy}
    heading={<><p className="console-eyebrow">Now playing</p><h2 className="display-font mt-1 text-2xl font-extrabold">{game.title}</h2></>}
    sub={`This session: ${played < 60 ? "just started" : formatPlaytime(played)}`}
    items={quickItems(game.title)} choose={id => choose(id as QuickChoice)} close={close}
    onHome={() => choose("resume")} onMenu={() => choose("power")} />;
}
