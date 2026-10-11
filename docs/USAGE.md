← [Documentation](README.md)

# How to use

This page assumes the plugin is installed ([INSTALLATION.md](INSTALLATION.md)). It covers setting up the keys, the settings, and what the keys show while you play.

## Setting up the keys

In the Stream Deck app, the plugin adds a category named **Helldivers 2 Stratagems** with three actions. Drag them onto a profile:

| Action | What it is for |
|---|---|
| Scan Now | The Scan key. Its animated radar shows whether the game is being read, and pressing it reads the stratagem menu right away. Its settings panel holds the settings of the whole plugin. |
| Stratagem Slot | One stratagem. Add one per stratagem you want on the deck. |
| Clear Stratagem Slots | Empties every slot. They fill again on the next scan. |

A slot shows the stratagem at the same position in the game's stratagem menu. The menu always starts with Reinforce, SOS Beacon and Resupply, so:

| Slot | Stratagem |
|---|---|
| 1 | Reinforce |
| 2 | SOS Beacon |
| 3 | Resupply |
| 4 to 7 | Your four loadout stratagems, in menu order |
| 8 and up | Stratagems a mission hands out, like the Hellbomb or the SSSD Delivery |

Each new Stratagem Slot key takes the first free slot. To change it, select the key and pick another number in its settings. The same panel has *Show name*, which writes the stratagem's name under its icon (off by default).

## Matching the game's controls

The plugin calls a stratagem in the way you would: it opens the stratagem menu and presses the arrow keys of the code. So its inputs have to match your key bindings in the game. Select the Scan key, set them in its settings panel, and press **Save**:

- *Stratagem menu* is the key that opens the menu (Left Ctrl by default).
- *Menu mode* must match the game's own option for the stratagem menu: *Hold while entering* if you hold the key, *Tap to toggle* if a tap opens the menu and the next one closes it.
- *Up*, *Down*, *Left* and *Right* are the keys of the codes (W, S, A and D by default; the arrow keys work as well if you bound them).
- *Throw* is optional. When it is set, the plugin presses it after the code, which throws the stratagem straight away.

To set an input, click its box and press the key or mouse button, or choose one from the list next to it. Backspace clears the throw key.

## Playing

Start the game, or press the Scan key while the game is closed: it shows a play button then, and starts the game through Steam.

When a mission starts and your loadout isn't known yet, the plugin opens the stratagem menu once to read it (two seconds after the HUD appears, so that the drop has settled; if the menu can't open yet, it tries again a few times). After that, it follows what the HUD shows with the menu closed. Opening the menu yourself, or pressing the Scan key, reads everything again.

Press a slot key to call its stratagem in. The game has to be the window in front; if it isn't, the key shows a warning triangle and nothing is typed into the other window. The plugin also refuses a stratagem the game would refuse (cooling down, unavailable or already on its way), except in the last 5% of a cooldown, where it can't tell the difference reliably.

### What a slot key shows

| The key shows | Meaning |
|---|---|
| A signal icon, filling in | The slot hasn't been read yet |
| The stratagem's icon | Ready |
| A beige fill rising from the bottom, going down as time passes | Cooling down. The fill shows how much of the cooldown is left |
| A fill coming down from the top | On its way (activating, inbound or impact) |
| The icon covered completely | Unavailable |
| A number in the bottom-right corner | The count the game shows, like the reinforcements left or the uses of a stratagem |
| Black | Nothing in that slot this mission |

With the menu closed, the game doesn't show a stratagem's cooldown. The plugin then counts it down itself, from the length it measured earlier in the session or from the [wiki](https://helldivers.wiki.gg/wiki/Stratagems) figure, and corrects it as soon as the game shows the row again.

### What the Scan key shows

| The key shows | Meaning |
|---|---|
| A yellow play button | The game isn't running. Press it to start the game, or to start reading straight away if the game was just started |
| A grey radar, not moving | The game is running, but its window can't be read (minimised, or still starting) |
| A yellow radar | The game is open with no mission HUD: the ship, a menu or a loading screen |
| A green radar | In a mission |

After about ten seconds on yellow, the slots go back to the signal icon, ready for the next loadout.

While the game isn't running, the plugin doesn't capture the screen at all. It only checks every two seconds whether the game has started, and the Clear key and the empty slots turn grey and stop moving.

## All settings

They are on the Scan key's settings panel and apply to every key.

| Setting | Default | What it does |
|---|---|---|
| Auto scan | On | Reads the screen every *Scan period*. Off: only the Scan key reads it |
| Open the menu once at mission start | On | Reads the loadout when a mission starts, as described above |
| Scan period | 0.15 s | Time between two reads (0.1 to 60 s). For a few seconds after you call a stratagem in, the plugin reads every 50 ms to catch the change |
| Confirm after | 2 | How many reads in a row must see a stratagem before its key shows it (1 to 10) |
| Icon scale | 1 | Only for a HUD scale other than the game's default (0.5 to 2) |
| Game resolution | auto | The game's size in pixels. *auto* measures the game's window, and the line under the box shows what was found. Type a size such as `2560x1440` only when that is wrong: when Windows hides the game's window from the plugin, the plugin falls back to the main screen's size, which differs from the game's if the game doesn't run at the screen's resolution |
| Stratagem menu, Menu mode, Up, Down, Left, Right, Throw | Left Ctrl, Hold, W, S, A, D, none | See [Matching the game's controls](#matching-the-games-controls) |
| Menu open delay | 120 ms | Wait after opening the menu, before the first arrow |
| Scan menu wait | 350 ms | Extra wait before the Scan key reads the menu it opened |
| Key hold | 35 ms | How long each key is held down |
| Delay between keys | 40 ms | Pause between two arrows |

If the game misses arrows, raise *Key hold* and *Delay between keys* a little. If the first arrow is lost, raise *Menu open delay*.

## Limits

- The plugin reads English status words only, so with the game in another language the keys can't show cooldowns. [TRANSLATIONS.md](TRANSLATIONS.md) explains how to add a language.
- It reads the game's pixels, so the stratagem panel in the top-left corner must be visible: a window over it hides it from the plugin too.
- It has been tested at 2560x1440, with the game's default HUD.
