# Code signing

## Where things stand

The installer and `Aether Hub.exe` are **not signed**. Windows SmartScreen therefore shows "unknown publisher" and may warn
before running them. That is the main thing between this project and feeling like a finished product, but it is about
trust, not a fault in the program. Nothing in the repository holds a certificate, key or token, and none were invented.

A self-made ("self-signed") certificate does not help: Windows only trusts certificates that chain to an authority it already
trusts, so players would see the same warning.

## The free route: SignPath Foundation

[SignPath Foundation](https://signpath.org) signs open-source projects at no cost. Its published conditions
([terms](https://signpath.org/terms)) include:

- the project must not contain malware or potentially unwanted programs;
- every component must use an OSI-approved open-source licence, with no commercial dual-licensing;
- the certificate is issued to SignPath Foundation, so the publisher Windows shows is SignPath Foundation, and the project
  must carry the line "Free code signing provided by SignPath.io, certificate by SignPath Foundation".

Guides written by projects that went through it add a public repository with automated builds (this one has: GitHub
Actions), multi-factor sign-in on the GitHub and SignPath accounts, and in one case at least one earlier release. **I could not open the
full terms and application pages in the session that wrote this,** so that list may be incomplete or out of date. Read
<https://signpath.org/terms> and <https://signpath.org/apply> before applying.

Even a signed installer can still trigger SmartScreen for a while: reputation for a new file builds over time and is
separate from the signature.

### How Aether Hub measures up today

| Condition | Status |
| --- | --- |
| Public repository, built by GitHub Actions | Yes |
| Open-source licence | **No `LICENSE` file yet.** Without one the code is public but nobody has permission to use it. Choosing a licence is your decision (MIT is the simplest, GPL protects against closed forks); this is the one thing that blocks an application. |
| No proprietary components | The libraries are open source. **The selection click sound (`src/lib/selectSound.ts`, from `Click1_SP_01.wav`) has an unknown licence.** If it is not yours to redistribute it must be replaced before applying. |
| Maturity of the project | Unknown. New projects with no users may be declined; applying is free. |

### What you would do, and what I can then do

You: choose and add a licence; turn on multi-factor sign-in for GitHub; apply at signpath.org/apply; add a short code
signing policy page naming who builds and who approves a release.

After approval SignPath gives you an organisation, a project and a signing policy. Send me their three names and create a
**GitHub secret** with the API token SignPath issues (never put the token in the repository or in chat). I will then add a
signing step to the build workflow, so the installer is signed before it is attached to a release, and run it to confirm.

## Paid routes

Commercial certificates and Microsoft's own signing service exist and cost money, so none has been set up. Whichever you pick,
the workflow change is the same shape as above.

## Updates that install themselves (not done)

Today an update is a button that opens the download page. Tauri can also download and install updates itself. That needs a
separate update-signing key pair made with Tauri's own tool (free; the private key goes in a GitHub secret and never in the
repository, the public key goes in `tauri.conf.json`), the updater plugin (new Rust libraries, so a `Cargo.lock` update and
a new permission), and an update file published with each release. It protects the integrity of updates but does not remove
the SmartScreen warning, which only a code-signing certificate does. It was left out because each added library and permission
widens what the program can do, and the manual link is enough while installers are unsigned. Check Tauri's updater
documentation for the current steps before starting; I have not tested them here.
