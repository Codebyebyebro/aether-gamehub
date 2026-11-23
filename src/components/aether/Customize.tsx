import { LayoutGrid, GalleryHorizontalEnd, List, MonitorPlay, RotateCcw, ImagePlus, Check, SlidersHorizontal } from "lucide-react";
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription } from "@/components/ui/sheet";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { Settings, defaults, desktop, pickFile, importArtwork } from "@/lib/aether";
import { toast } from "sonner";

const views = [
  { id: "poster", label: "Poster grid", icon: LayoutGrid },
  { id: "banner", label: "Wide banners", icon: GalleryHorizontalEnd },
  { id: "list", label: "Compact list", icon: List },
  { id: "carousel", label: "Big Box", icon: MonitorPlay },
] as const;
const palettes = [
  { id: "cream", name: "Daylight", bg: "#f4f1eb", accent: "#5b4bd6" },
  { id: "navy", name: "Dusk", bg: "#182037", accent: "#7155ce" },
  { id: "cyan", name: "Orbit", bg: "#142438", accent: "#087f96" },
] as const;
function Range({ label, value, min, max, unit = "px", onChange }: { label: string; value: number; min: number; max: number; unit?: string; onChange: (v: number) => void }) {
  return <div className="space-y-3">
    <div className="flex justify-between text-xs"><span>{label}</span><span className="tabular-nums text-muted-foreground">{value}{unit}</span></div>
    <Slider className="[&_[role=slider]]:border-[var(--aether-accent)]" min={min} max={max} step={1} value={[value]} onValueChange={([v]) => onChange(v)} />
  </div>;
}
export default function Customize({ open, onOpenChange, settings, update }: { open: boolean; onOpenChange: (v: boolean) => void; settings: Settings; update: (patch: Partial<Settings>) => void }) {
  async function wallpaper() {
    try { const path = await pickFile("image"); if (path) update({ wallpaper: await importArtwork(path) }); }
    catch (e) { toast.error(String(e)); }
  }
  function accent(value: string) {
    if (!/^#[0-9a-f]{6}$/i.test(value)) { toast.error("Enter a six-digit hex color, such as #5B4BD6."); return; }
    const rgb = [1, 3, 5].map(i => parseInt(value.slice(i, i + 2), 16) / 255).map(c => c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4);
    const luminance = rgb[0] * .2126 + rgb[1] * .7152 + rgb[2] * .0722;
    if (1.05 / (luminance + .05) < 4.5) { toast.error("Choose a darker accent so white button labels stay readable."); return; }
    update({ accent: value });
  }
  return <Sheet open={open} onOpenChange={onOpenChange}>
    <SheetContent className="w-[390px] max-w-full overflow-y-auto bg-card p-7 sm:max-w-[390px]">
      <SheetHeader className="text-left">
        <div className="mb-3 flex h-11 w-11 items-center justify-center rounded-2xl bg-secondary text-[var(--aether-accent)]"><SlidersHorizontal size={20} /></div>
        <SheetTitle className="display-font text-2xl font-extrabold">Make it yours.</SheetTitle>
        <SheetDescription className="text-xs leading-relaxed">Your library, your way. Changes appear instantly.</SheetDescription>
      </SheetHeader>
      <div className="mt-8 space-y-8">
        <section>
          <h3 className="eyebrow mb-4">Library layout</h3>
          <div className="grid grid-cols-2 gap-2">{views.map(({ id, label, icon: Icon }) =>
            <button key={id} onClick={() => { update({ view: id }); if (id === "carousel") onOpenChange(false); }} className={`flex flex-col gap-3 rounded-xl border p-4 text-left text-xs ${settings.view === id ? "border-[var(--aether-accent)] bg-secondary text-[var(--aether-accent)]" : "hover:bg-muted"}`}><Icon size={22} />{label}</button>
          )}</div>
        </section>
        <section>
          <h3 className="eyebrow mb-4">Color palette</h3>
          <div className="grid grid-cols-3 gap-2">{palettes.map(p =>
            <button key={p.id} onClick={() => update({ theme: p.id, accent: p.accent })} className={`rounded-xl border p-2 text-xs ${settings.theme === p.id ? "border-[var(--aether-accent)]" : ""}`}>
              <span className="mb-2 flex h-12 items-center justify-center rounded-lg" style={{ background: p.bg }}><span className="h-5 w-5 rounded-full" style={{ background: p.accent }}>{settings.theme === p.id && <Check size={14} className="m-[3px] text-white" />}</span></span>{p.name}
            </button>
          )}</div>
          <div className="mt-4 flex items-center justify-between gap-3 text-xs">
            <label htmlFor="accent">Accent color</label>
            <div className="flex items-center gap-2">
              <input aria-label="Pick accent color" type="color" value={settings.accent} onChange={e => accent(e.target.value)} className="h-8 w-8 cursor-pointer rounded-lg border bg-transparent p-1" />
              <Input id="accent" key={settings.accent} defaultValue={settings.accent.toUpperCase()} className="aether-field h-8 w-24 text-xs" maxLength={7} onBlur={e => accent(e.target.value)} />
            </div>
          </div>
        </section>
        <section className="space-y-5">
          <h3 className="eyebrow">Card details</h3>
          <Range label="Card size" value={settings.size} min={130} max={280} onChange={size => update({ size })} />
          <Range label="Space between cards" value={settings.gap} min={8} max={40} onChange={gap => update({ gap })} />
          <Range label="Rounded corners" value={settings.radius} min={0} max={30} onChange={radius => update({ radius })} />
          <div className="flex items-center justify-between text-xs"><span>Show game titles</span><Switch checked={settings.titles} onCheckedChange={titles => update({ titles })} /></div>
        </section>
        <section className="space-y-5">
          <h3 className="eyebrow">Wallpaper</h3>
          <div className="flex gap-2">
            <Button variant="outline" className="flex-1 rounded-xl text-xs" onClick={() => update({ wallpaper: settings.theme === "cream" ? "/orbit-wallpaper.svg" : `/orbit-${settings.theme}-wallpaper.svg` })}>Orbital preset</Button>
            <Button variant="outline" className="flex-1 rounded-xl text-xs" onClick={wallpaper}><ImagePlus size={14} className="mr-2" />Local image</Button>
          </div>
          {!desktop && <p className="text-[11px] text-muted-foreground">Local wallpaper pickers require the desktop app.</p>}
          <Range label="Blur" value={settings.blur} min={0} max={20} onChange={blur => update({ blur })} />
          <Range label="Dim" value={settings.dim} min={0} max={85} unit="%" onChange={dim => update({ dim })} />
          {settings.wallpaper && <button className="text-xs text-muted-foreground underline" onClick={() => update({ wallpaper: "" })}>Remove wallpaper</button>}
        </section>
        <div className="rounded-xl bg-secondary p-4">
          <div className="flex items-center justify-between text-sm font-semibold"><span>Performance mode</span><Switch checked={settings.performance} onCheckedChange={performance => update({ performance })} /></div>
          <p className="mt-2 text-xs leading-relaxed text-muted-foreground">Less motion. No blur or wallpaper. Smaller artwork requests. More room for your games.</p>
        </div>
        <div className="rounded-xl bg-secondary p-4">
          <div className="flex items-center justify-between text-sm font-semibold"><span>Navigation sounds</span><Switch checked={settings.sounds} onCheckedChange={sounds => update({ sounds })} aria-label="Navigation sounds" /></div>
          <p className="mt-2 text-xs leading-relaxed text-muted-foreground">A soft click when the controller moves the selection.</p>
        </div>
        <Button variant="ghost" className="w-full rounded-xl text-xs text-muted-foreground" onClick={() => update({ ...defaults, onboarded: settings.onboarded, folders: settings.folders, afterLaunch: settings.afterLaunch })}><RotateCcw size={14} className="mr-2" />Reset appearance</Button>
      </div>
    </SheetContent>
  </Sheet>;
}
