# Stratagem symbols

The SVG symbols in this folder come from [nvigneux/Helldivers-2-Stratagems-icons-svg](https://github.com/nvigneux/Helldivers-2-Stratagems-icons-svg) (commit `e7b06c6`), made for the [Stratagem Hero Trainer](https://stratagem-hero-trainer.vercel.app/), whose README invites their use in other projects. They are vector redrawings of the stratagem icons of Helldivers 2, whose designs belong to Sony Interactive Entertainment and Arrowhead Game Studios. They are not covered by this repository's Apache 2.0 license.

`tools/make_key_icons.py` frames each symbol the way the game does (a border in the colour of the stratagem's type, a dark inside) and writes the key pictures to `data/keys_144/`, which the plugin shows on its keys. The files keep their original names; the script matches them to the plugin's stratagem names.
