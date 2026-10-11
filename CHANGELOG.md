# Changelog

What changed in each version of the plugin, newest first. The versions follow [semantic versioning](https://semver.org); the headings follow [Keep a Changelog](https://keepachangelog.com). The **Unreleased** section gathers what is on `dev` and not yet released.

## [Unreleased]

The first version.

### Added

- **Stratagem keys that follow the game.** The plugin reads the stratagem menu from the screen and shows each stratagem on a Stratagem Slot key, in menu order: Reinforce, SOS Beacon, Resupply, the four loadout stratagems, then the ones a mission hands out. It recognises every stratagem of the wiki, including the ones that share an icon (SSSD Delivery and Upload Data), which it tells apart by their arrow code.
- **Calling stratagems in from the deck.** A key press opens the stratagem menu and enters the code with your own key bindings, in hold or toggle mode, and can throw the stratagem too. Nothing is typed when the game isn't the window in front, or when the stratagem isn't ready.
- **Cooldowns, call-ins and counters on the keys.** A key shows the game's progress fill across the whole icon: rising from the bottom while cooling down, coming down from the top while a call-in is on its way, covering it when the stratagem is unavailable. It also shows the game's counters, like the reinforcements left. With the menu closed, the plugin counts the cooldown down itself, from the length it measured earlier in the session or from the wiki.
- **The loadout is read at mission start.** The plugin opens the stratagem menu once when a mission starts, and empties the slots after ten seconds back on the ship.
- **A Scan key with a radar.** It shows whether the game is being read (grey: the window can't be seen, yellow: on the ship, green: in a mission), and reads the menu on demand.
- **Standby while the game is closed.** The plugin captures nothing until the game runs, the Scan key becomes a start button that launches the game through Steam, and the other keys turn grey.
- **A Clear Stratagem Slots key** that empties every slot until the next scan.
- **Settings on the Scan key**, shared by every key: the scan period, the inputs, the timings, and the game's resolution for when its window can't be measured (the panel shows the size the plugin found).

[Unreleased]: https://github.com/xiaodoudou/streamdeck-helldivers2-stratagems-plugin/commits/dev
