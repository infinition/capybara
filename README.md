<div align="center">

<img src=".github/capybara.png" alt="Capybara" width="160">

# Capybara

**A bare-metal emulator for the Sonix SNC73410, compatible with Tamagotchi Paradise firmware.**

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/infinition/capybara?include_prereleases)](https://github.com/infinition/capybara/releases)

[English](#english) | [Français](#francais)

</div>

---

<a name="english"></a>

## English

Capybara runs the real factory firmware of a Tamagotchi Paradise on your
computer. It is not a reimplementation of the game: it is an ARMv7-M core
written from scratch in Rust, with the undocumented SNC73410 peripherals
modelled from measurement. The firmware boots, the egg hatches, the clock
advances, the gauges fall, the sound comes out, and the save survives a reboot
of your computer.

### Status

| Area | State |
|---|---|
| ARMv7-M core, Thumb-2 | Runs the factory firmware, 1.40 times real time |
| Display, 128 x 128 RGB565 | Complete |
| Buttons, wheel, deep sleep and hardware wake | Complete |
| Real time clock, calendar, ageing | Complete |
| Persistent saves per dump | Complete |
| Sound | Notes are the ones the firmware composes, output peripheral not identified |
| Serial link, page `0x4000B000` | Located, not yet modelled |
| Editions | Water and Jade Forest played end to end, three others partially |

### What you need

Capybara ships no copyrighted data. You supply two things, taken from a console
you own.

- **A flash dump** of your own device, 16 MB.
- **The device key**, read over SWD. It is burned in the chip fuses, appears in
  no dump, and is read from the device itself.

Neither is distributed here, and neither will be. Without them the application
opens and asks for them.

### Build

```
cargo build --release
```

The binary lands in `target/release/capybara`. Prebuilt Windows executables are
in [Releases](https://github.com/infinition/capybara/releases).

### Use

Load your dump from the interface. For the tests and the probes, two environment
variables:

```
export SONIX_DEVICE_KEY=0x........
export SONIX_DUMPS=<folder holding the .bin files>
```

Without them, the tests that depend on a dump skip cleanly.

### Probes

Twenty-five programs in `examples/`, all taking `<dump.bin> <key hex>`. These are
the instruments the reverse engineering was done with. They stay in the
repository because the work is not finished.

- `boot_probe`, the general one: regions visited, most executed addresses,
  registers touched with the program counter that touched them.
- `mmio_releve_probe`, one line per hardware register, made to be passed to
  `diff` between two runs. This is what found the serial port.
- `table_scenes_probe`, extracts the scene table of any edition without
  executing anything.
- `watch_probe`, stops at the Nth visit of an address or the Nth change of a
  word, and returns the real call stack.

### Documentation

The research dossier is `index.html` at the root: pinout, memory map, Sonix load
table format, AES key derivation, extraction procedures, and the sixteen ARMv7-M
decoding faults found by running real code.

### Support

If this work is useful to you:

<a href="https://www.buymeacoffee.com/infinition"><img src="https://img.shields.io/badge/Buy%20me%20a%20coffee-infinition-yellow" alt="Buy me a coffee"></a>

### Legal

Capybara is an independent work of reverse engineering for interoperability. It
is not affiliated with, endorsed by, or connected to Bandai. Tamagotchi and
Tamagotchi Paradise are trademarks of Bandai. No firmware, no ROM, no key and no
graphic or sound asset from the console is contained in this repository or in
the published executables. The extraction procedures described in the dossier
apply to a device you own.

Distributed under the GNU General Public License v3.0. See [LICENSE](LICENSE).

---

<a name="francais"></a>

## Français

Capybara fait tourner le vrai firmware d'usine d'un Tamagotchi Paradise sur
votre ordinateur. Ce n'est pas une reimplementation du jeu : c'est un coeur
ARMv7-M ecrit de zero en Rust, avec les peripheriques non documentes du
SNC73410 modelises a la mesure. Le firmware demarre, l'oeuf eclot, l'horloge
avance, les jauges descendent, le son sort, et la sauvegarde survit a
l'extinction de l'ordinateur.

### Etat

| Domaine | Etat |
|---|---|
| Coeur ARMv7-M, Thumb-2 | Execute le firmware d'usine, 1,40 fois le temps reel |
| Ecran, 128 x 128 RGB565 | Complet |
| Boutons, molette, veille profonde et reveil materiel | Complet |
| Horloge temps reel, calendrier, vieillissement | Complet |
| Sauvegardes persistantes par dump | Complet |
| Son | Les notes sont celles que le firmware compose, la sortie n'est pas identifiee |
| Lien serie, page `0x4000B000` | Trouve, pas encore modelise |
| Editions | Water et Jade Forest menees de bout en bout, trois autres partiellement |

### Ce qu'il vous faut

Capybara ne distribue aucune donnee sous droit d'auteur. Vous fournissez deux
choses, tirees d'une console qui vous appartient.

- **Un dump de la flash** de votre propre appareil, 16 Mo.
- **La cle de l'appareil**, lue en SWD. Elle est gravee dans les fusibles de la
  puce, ne figure dans aucun dump, et se lit sur l'appareil.

Ni l'un ni l'autre n'est distribue ici, et ne le sera. Sans eux l'application
s'ouvre et vous les demande.

### Compiler

```
cargo build --release
```

Le binaire arrive dans `target/release/capybara`. Les executables Windows
precompiles sont dans les [releases](https://github.com/infinition/capybara/releases).

### Utiliser

Le dump se charge depuis l'interface. Pour les tests et les sondes, deux
variables d'environnement :

```
export SONIX_DEVICE_KEY=0x........
export SONIX_DUMPS=<dossier contenant les .bin>
```

Sans elles, les tests qui dependent d'un dump se sautent proprement.

### Les sondes

Vingt-cinq programmes dans `examples/`, tous en `<dump.bin> <cle hex>`. Ce sont
les instruments avec lesquels la retro-ingenierie a ete faite. Ils restent dans
le depot parce que le travail n'est pas fini.

- `boot_probe`, la generaliste : zones parcourues, adresses les plus executees,
  registres touches avec le compteur de programme qui les touche.
- `mmio_releve_probe`, une ligne par registre materiel, faite pour etre passee a
  `diff` entre deux executions. C'est elle qui a trouve le port serie.
- `table_scenes_probe`, extrait la table des scenes de n'importe quelle edition
  sans rien executer.
- `watch_probe`, s'arrete a la Nieme visite d'une adresse ou a la Nieme
  modification d'un mot, et rend la pile d'appels reelle.

### Documentation

Le dossier de recherche est la page `index.html` a la racine : le brochage, la
carte memoire, le format des load tables Sonix, la derivation de cle AES, les
procedures d'extraction, et les seize defauts de decodage ARMv7-M trouves en
faisant tourner du vrai code.

### Soutenir

Si ce travail vous est utile :

<a href="https://www.buymeacoffee.com/infinition"><img src="https://img.shields.io/badge/Buy%20me%20a%20coffee-infinition-yellow" alt="Buy me a coffee"></a>

### Mentions legales

Capybara est un travail independant de retro-ingenierie a des fins
d'interoperabilite. Il n'est ni affilie, ni approuve, ni lie a Bandai.
Tamagotchi et Tamagotchi Paradise sont des marques de Bandai. Aucun firmware,
aucune ROM, aucune cle et aucun element graphique ou sonore de la console n'est
contenu dans ce depot ni dans les executables publies. Les procedures
d'extraction decrites dans le dossier s'appliquent a un appareil dont vous etes
proprietaire.

Distribue sous licence GNU General Public License v3.0. Voir [LICENSE](LICENSE).
