# What's new

Each release has a section here named after its version. The Windows build copies that section into the release notes, and a test fails if the current version has none.

## Not released yet

- **Living colors.** Aether Hub takes on the colours of your games. Point at a game, select one with the controller, or open its details, and the whole app slowly drifts to the main colour of its artwork. In Big Box the screen glows with the selected game's colours as you browse. Text stays on its usual surfaces, so it stays easy to read. You can switch it off in **Customize UI**, and Performance mode turns it off.
- **Jump back in.** The big spotlight at the top now shows the game you played last, with how long you've played it and when ("14.4 h played · last played yesterday"). While a game is running, it says **Now playing**. The dots switch between your three most recent games. A new **Recently played** row sits above your library, and you can sort your library by **Recently played** or **Most played**. A library with nothing played yet looks the same as before.
- **Find missing artwork.** Games without a cover picture, such as Epic games, Riot games and games found in your own folders, can now get their cover and banner automatically. Aether Hub looks each one up by name in Steam's public store, with no account or key needed, and only the game names are sent. A picture is added only when the name matches exactly and just one game fits, so you never get the wrong cover. Anything uncertain is left as it was and listed with the reason. Games not sold on Steam, such as VALORANT, can be looked up on SteamGridDB too if you type your own key, which is used for that search only and not saved. Use **Find missing artwork** above your library. It is also offered after a scan adds games without pictures.

## 0.2.0

- **Missing games.** A game whose file was moved, renamed or uninstalled now shows a **Missing** tag, and a **Missing files** list appears in the sidebar. A game on a drive that is not connected, such as an unplugged USB disk, shows **Drive not connected** instead and is not treated as broken. Pressing Play on a flagged game looks again first, then explains what happened and opens the editor so you can choose where the game is now.
- **If something does not work.** Settings has a new **Copy diagnostics** button, and Aether Hub keeps a small log file on your PC. Nothing is sent anywhere; read the text before you share it.
- **Fixed:** if Aether Hub started up already in Big Box, the controller moved the library behind it instead of Big Box.

## 0.1.0

- First release.
