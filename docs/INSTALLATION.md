← [Documentation](README.md)

# Installation

## What you need

- Windows 10 or later. The plugin is a Windows program.
- The Stream Deck app, version 7.1 or later.
- Helldivers 2 on Steam. The Scan key starts the game through Steam; reading the game works whatever launched it.

## From a release

1. Open the [latest release](https://github.com/xiaodoudou/streamdeck-helldivers2-stratagems-plugin/releases/latest) and download `com.xiaodoudou.helldivers2-stratagems.streamDeckPlugin`.
2. Open the file. The Stream Deck app asks whether to install the plugin.
3. Set up your keys: [USAGE.md](USAGE.md).

To update, open the file of a newer release the same way. To remove the plugin, use the plugins list in the Stream Deck app's preferences.

## From the source

You need [Rust](https://rustup.rs/) (the stable toolchain) and git. PowerShell 5.1, which comes with Windows, runs the scripts. Python 3 is only needed for the tools in `tools/` and the commit check, not to build.

```powershell
git clone https://github.com/xiaodoudou/streamdeck-helldivers2-stratagems-plugin.git
cd streamdeck-helldivers2-stratagems-plugin
.\build.ps1
.\install.ps1
```

`build.ps1` compiles the plugin and assembles it in `plugin\com.xiaodoudou.helldivers2-stratagems.sdPlugin`. `install.ps1` copies that folder into Stream Deck's plugins folder (`%APPDATA%\Elgato\StreamDeck\Plugins`). Restart the Stream Deck app afterwards (quit it from the tray icon, then open it again) so it loads the plugin.

If Windows refuses to run the scripts, allow them for this one run:

```powershell
powershell -ExecutionPolicy Bypass -File .\build.ps1
```

Other options of the scripts:

| Command | What it does |
|---|---|
| `.\build.ps1 -Package` | Also writes `dist\com.xiaodoudou.helldivers2-stratagems.streamDeckPlugin`, the file a release ships |
| `.\install.ps1 -Link` | Links the plugin folder into Stream Deck instead of copying it, so that each build is picked up. For development, see [CONTRIBUTING.md](../CONTRIBUTING.md) |
| `.\install.ps1 -Uninstall` | Removes the plugin from Stream Deck |

## When something goes wrong

The plugin writes a log next to itself, in `%APPDATA%\Elgato\StreamDeck\Plugins\com.xiaodoudou.helldivers2-stratagems.sdPlugin`: `hd2-plugin.log` for the current run and `hd2-plugin.prev.log` for the one before (a restart replaces the log, so the previous one is kept). It says when the game was found, what was read, and why a key press was refused. Attach both files to an issue when you report a problem.

- **The keys stay on the signal icon.** Check that the game shows the stratagem panel in the top-left corner and that nothing covers it, and see [the limits](USAGE.md#limits).
- **A key shows a warning triangle.** The game wasn't the window in front, the stratagem wasn't ready, or the slot is empty. The log says which.
- **The game misses arrows.** See the timing settings in [USAGE.md](USAGE.md#all-settings).
- **Stream Deck reports the plugin as unstable.** It does that after the plugin stopped several times in a row. Restart the Stream Deck app.
- **A renamed action or a new key image doesn't appear.** The Stream Deck app reads the plugin's manifest only when it starts; restart it.
