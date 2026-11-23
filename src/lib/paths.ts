/** Path helpers that must agree with `storage::identity` and `util::strip_verbatim` in the Rust backend. */
const BS = String.fromCharCode(92);

/** Windows' canonical paths carry a `\\?\` prefix that is noise to people and to duplicate checks. */
export function stripVerbatim(path: string): string {
  const unc = `${BS}${BS}?${BS}UNC${BS}`, plain = `${BS}${BS}?${BS}`;
  if (path.startsWith(unc)) return `${BS}${BS}${path.slice(unc.length)}`;
  if (path.startsWith(plain)) return path.slice(plain.length);
  return path;
}
/** Two games with the same identity launch the same thing. */
export const launchIdentity = (target: string, args: string) => `${stripVerbatim(target).split("/").join(BS).toLowerCase()}|${args.trim()}`;
