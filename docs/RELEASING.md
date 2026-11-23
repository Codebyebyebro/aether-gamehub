# Releasing Aether Hub

A release is a GitHub release with the installer attached. It is also what the in-app **Check now** looks for: the update
check asks GitHub for this repository's latest published release and compares its tag with the version inside the running
program. Nothing here costs money while the repository is public (standard GitHub-hosted runners and release files are free
for public repositories; a private repository would use your plan's allowance instead).

## Steps

1. **Pick the version** and change it in **both** `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` (for example `0.2.0`).
   A test fails if they differ, and every install would think it is out of date if the tag did not match what was built.
2. **Push and wait for the check** (Actions, "Windows build", the *check* job) to go green.
3. **Build the installer and draft the release in one go:** Actions → *Aether Hub Windows build* → *Run workflow* on that
   branch, with **Also create a DRAFT release** ticked. It takes about ten to fifteen minutes. Open the run summary and read
   the start-up checks, including "quiet start in the tray": they report problems without failing the run. The last job then
   attaches this same run's installer, a checksum file and the installation guide to a draft named after the version. It
   stops without creating anything if the two version numbers differ, a release with that tag already exists, or the
   installer no longer matches its checksum. Leave the box unticked for a test build you do not mean to release.
4. **Try the installer yourself** from the draft (see the release checklist) before anyone else gets it. Only people with
   write access to the repository can see or download a draft.
5. **Publish:** Releases → open the draft → read the notes → *Publish release*. Until you press that button the in-app check
   cannot see it. The tag `v0.2.0` is created at the built commit when you publish. To redo a draft, delete it first.

## Things to know

- The tag must look like `v1.2.3`. Pre-releases and drafts are ignored by the update check, so a release marked "pre-release" is a safe way to share a test build.
- The installer is **not code-signed**, so the notes say so. See [Code signing](CODE-SIGNING.md) for the free route and what it needs from you.
- Publishing makes the installer public. Publish only a build you have tried.
- To take a bad release back, unpublish or delete it on the Releases page; the update check then falls back to the previous release.
