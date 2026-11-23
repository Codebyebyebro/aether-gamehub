/** Puts text on the clipboard. Returns false when Windows or the web view refuses, so the caller can show the text instead. */
export async function copyText(text: string): Promise<boolean> {
  try { if (navigator.clipboard?.writeText) { await navigator.clipboard.writeText(text); return true; } } catch { /* fall back below */ }
  try {
    const box = document.createElement("textarea");
    box.value = text; box.setAttribute("readonly", ""); box.style.cssText = "position:fixed;left:-9999px;top:0";
    document.body.appendChild(box); box.select();
    const ok = document.execCommand("copy"); box.remove();
    return ok;
  } catch { return false; }
}
