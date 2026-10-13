# Changelog

What changed in each version of the plugin, newest first. The versions follow [semantic versioning](https://semver.org); the headings follow [Keep a Changelog](https://keepachangelog.com). The **Unreleased** section gathers what is on `dev` and not yet released.

## [Unreleased]

## [0.5.0] - 2026-10-13

### Added

- **The keys fill before the mission.** On the loadout screen, the plugin reads your card (your helldiver, with its *Mission Stratagems*) and fills the slot keys with what the stratagem menu will hold, in its order. They show as unavailable until the mission starts, follow your picks while you choose, and are checked against the menu at landing as before.

### Changed

- **A quicker look at the menu when a mission starts.** The plugin closes the menu as soon as it has read it, after about a quarter of a second instead of half a second. After the loadout screen, it opens the menu as soon as the HUD shows Reinforce and tries again every second, so the keys are ready about a second after landing.

### Fixed

- **No stray menu at landing.** When the game ignored the plugin's first press of the menu key during the landing, the plugin pressed it again to close a menu that wasn't open, which could open it for a moment instead.

## [0.4.0] - 2026-10-13

### Changed

- **New icons.** The plugin icon and the category icon are original drawings (four stratagem arrows) instead of the game's Reinforce icon, and the icons of the actions list are pure white on transparent, as the Elgato Marketplace asks.
- **The game's own icons, sharp on the keys.** The plugin now takes the stratagem icons from the game's files instead of the wiki's copies, some of which were out of date: the game has since added markers to the backpacks and the expendable weapons and redrawn a few icons. The keys show them at full size, sharper than the wiki's small pictures, and the screen is read against the same icons. The Tectonic Drill, which the game draws with an icon of its own, is no longer mixed up with the other drills.
- **The settings save themselves.** The Scan key's panel has no Save button any more: each change is saved as you make it. On/off settings are checkboxes, and a value out of range is outlined in red with the reason, while the last good value stays in effect. A *How to use* link opens the guide.

## [0.3.0] - 2026-10-11

### Changed

- **Stream Deck 7.1 or later is needed.** The plugin now declares version 3 of the Stream Deck SDK, which the Elgato Marketplace asks for, and Stream Deck 7.1 as the oldest version it supports.
- **The plugin is one file.** The stratagem list and its pictures are built into the plugin itself, so it reads no file of its own at run time, as the Marketplace's DRM protection requires.

### Fixed

- **Cooldowns on the keys match the game's.** A key could show a cooldown far from the game's: a Gunner FRV with 7:48 of 8 minutes left showed 62%, because the plugin worked out the cooldown's length from how much of the icon looked covered, and the FRV's white car and bullets look the same under the game's fill. The length now comes from a cooldown watched from its start, from one learned earlier in the session, or from the wiki, and the key counts down to the game's timer.
- **The plugin no longer restarts itself in a loop** when it cannot read its own file, which could get it disabled by Stream Deck as unstable.

## [0.2.0] - 2026-10-11

The first version.

### Added

- **Stratagem keys that follow the game.** The plugin reads the stratagem menu from the screen and shows each stratagem on a Stratagem Slot key, in menu order: Reinforce, SOS Beacon, Resupply, the four loadout stratagems, then the ones a mission hands out. It recognises every stratagem of the wiki, including the ones that share an icon (SSSD Delivery and Upload Data), which it tells apart by their arrow code.
- **Calling stratagems in from the deck.** A key press opens the stratagem menu and enters the code with your own key bindings, in hold or toggle mode, and can throw the stratagem too. Nothing is typed when the game isn't the window in front, or when the stratagem isn't ready: a press on a cooling stratagem only goes through in its last moment (0.3 s before the game's timer ends, 1.5 s when the end is estimated).
- **Cooldowns, call-ins and counters on the keys.** A key shows the game's progress fill across the whole icon: rising from the bottom while cooling down, coming down from the top while a call-in is on its way, covering it when the stratagem is unavailable. It also shows the game's counters, like the reinforcements left, even in front of a bright sky. With the menu open, the plugin reads the game's own timer (`T-01:23`) and the key turns ready exactly when it ends; with the menu closed, it counts down from the last timer it read, the length it learned in the session, or the wiki.
- **The loadout is read at mission start.** The plugin opens the stratagem menu when a mission starts, checks that it opened, and tries again every two seconds while you are still landing. It empties the slots after ten seconds back on the ship.
- **Light on the machine.** A scan reads only the column of stratagem icons, about 13 ms every 150 ms in a mission at 2560x1440.
- **A Scan key with a radar.** It shows whether the game is being read (grey: the window can't be seen, yellow: on the ship, green: in a mission), and reads the menu on demand.
- **Standby while the game is closed.** The plugin captures nothing until the game runs, the Scan key becomes a start button that launches the game through Steam, and the other keys turn grey.
- **A Clear Stratagem Slots key** that empties every slot until the next scan.
- **Settings on the Scan key**, shared by every key: the scan period, the inputs, the timings, and the game's resolution for when its window can't be measured (the panel shows the size the plugin found).

[Unreleased]: https://github.com/xiaodoudou/streamdeck-helldivers2-stratagems-plugin/compare/v0.5.0...dev
[0.5.0]: https://github.com/xiaodoudou/streamdeck-helldivers2-stratagems-plugin/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/xiaodoudou/streamdeck-helldivers2-stratagems-plugin/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/xiaodoudou/streamdeck-helldivers2-stratagems-plugin/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/xiaodoudou/streamdeck-helldivers2-stratagems-plugin/releases/tag/v0.2.0
