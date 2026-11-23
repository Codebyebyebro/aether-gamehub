# What's new

Each release has a section here named after its version. The Windows build copies that section into the release notes, and a test fails if the current version has none.

## Not released yet

- **New logo.** A sharper two-tone "A" with a gold shard, on the app icon, the sidebar, Big Box and the start-up screen.
- **A sound for every action.** Moving, selecting, going back, switching category, opening a page or menu, and adding a favorite each have their own short sound. The move click now also plays in Big Box and in the console menus, where it was missing, and it no longer goes silent after Windows pauses the audio.
- **Fixed:** in Big Box, the row of games no longer jumps up and down while you scroll through it.
- **Fixed:** in Big Box, the controller can now reach the Play, Details and Favorite buttons. Press **Down** to go to them, **Left** and **Right** to choose, and **Up** or **Circle** to go back to the games. The details page also has a Back button.
- **Console mode.** With a controller connected, Aether Hub feels like a console:
  - **Starts like one.** Open Aether Hub with your controller connected and it goes straight into full-screen Big Box, with a short start-up animation and chime. Without a controller it opens as usual.
  - **A new home screen.** Big Box is now a console home screen. The selected game's artwork fills the screen, a row of game tiles runs along the top with your recently played games first, and a clock sits in the corner. **Square** opens a full-screen details page with play time, last played and platform, and **Triangle** adds or removes a favorite.
  - **Back home after playing.** When you quit a game, Aether Hub comes back to the front in Big Box with that game selected, instead of leaving you on the desktop.
  - **Power menu.** Press **Options** (Xbox: **Menu**) for a power menu: Exit to desktop, Quit Aether Hub, Sleep, Restart and Shut down. Sleep, Restart and Shut down ask "are you sure?" first, with **Cancel** selected, so a double tap can never turn the PC off.
  - **Launch sound and rumble.** Launching a game with the controller plays a short sound and gives the controller a short rumble. You can turn the rumble off under **Customize UI → Controller rumble**, and the sound with Navigation sounds. Xbox controllers rumble; a PS5 controller may not, depending on how Windows connects it.
  - **PS button during games.** Switch on **Settings → Background → PS / Xbox button opens Aether Hub**, then press the PS button (or Xbox button) during a game. A quick menu opens over it: **Resume game**, **Home** (Big Box, with the game still running), **Close game** (asks first) and **Power**. With no game running, the button brings Aether Hub forward in Big Box. It's off by default because it listens for that one button in the background. Steam and the Xbox Game Bar may react to the same button; the setting explains how to turn theirs off.
  - You can switch it off under **Settings → Background → Console mode**. It does nothing when no controller is connected. If "When a game starts" is set to "Close Launcher Completely", Aether Hub is not running to bring you back.
- **Living colors.** Aether Hub takes on the colours of your games. Point at a game, select one with the controller, or open its details, and the whole app slowly drifts to the main colour of its artwork. In Big Box the screen glows with the selected game's colours as you browse. Text stays on its usual surfaces, so it stays easy to read. You can switch it off in **Customize UI**, and Performance mode turns it off.
- **Jump back in.** The big spotlight at the top now shows the game you played last, with how long you've played it and when ("14.4 h played · last played yesterday"). While a game is running, it says **Now playing**. The dots switch between your three most recent games. A new **Recently played** row sits above your library, and you can sort your library by **Recently played** or **Most played**. A library with nothing played yet looks the same as before.
- **Find missing artwork.** Games without a cover picture, such as Epic games, Riot games and games found in your own folders, can now get their cover and banner automatically. Aether Hub looks each one up by name in Steam's public store, with no account or key needed, and only the game names are sent. A picture is added only when the name matches exactly and just one game fits, so you never get the wrong cover. Anything uncertain is left as it was and listed with the reason. Games not sold on Steam, such as VALORANT, can be looked up on SteamGridDB too if you type your own key, which is used for that search only and not saved. Use **Find missing artwork** above your library. It is also offered after a scan adds games without pictures.

## 0.2.0

- **Missing games.** A game whose file was moved, renamed or uninstalled now shows a **Missing** tag, and a **Missing files** list appears in the sidebar. A game on a drive that is not connected, such as an unplugged USB disk, shows **Drive not connected** instead and is not treated as broken. Pressing Play on a flagged game looks again first, then explains what happened and opens the editor so you can choose where the game is now.
- **If something does not work.** Settings has a new **Copy diagnostics** button, and Aether Hub keeps a small log file on your PC. Nothing is sent anywhere; read the text before you share it.
- **Fixed:** if Aether Hub started up already in Big Box, the controller moved the library behind it instead of Big Box.

## 0.1.0

- First release.
