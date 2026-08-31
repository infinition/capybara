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
computer. It is not a reimplementation of the game. It is an ARMv7-M core
written from scratch in Rust, with the undocumented SNC73410 peripherals
modelled from measurement.

The firmware boots, the egg hatches, the clock keeps running while the window is
closed, the gauges fall, the console sleeps and wakes, and your save survives a
reboot of your computer.

### What you need first

Capybara ships no game data, and never will. You supply two things, taken from a
console you own.

- **A flash dump** of your own device, 16 MB, read from its memory chip.
- **The device key**, read over SWD. It is burned into the chip fuses, appears
  in no dump, and can only be read from the device itself.

This is the only lawful and honest way to use the emulator: you emulate the
machine you already own. Without a dump and its key, the application opens and
asks for them.

The dossier at the root of this repository describes the extraction procedures.

### Getting started

1. Download an executable from
   [Releases](https://github.com/infinition/capybara/releases), or build one.
2. Open Capybara and load your dump. It is copied into the data folder, so it
   stays available even if you move the original.
3. Put the key in the data folder as `cle-device.txt`, or beside the dump in a
   file with the same name plus `.key`.
4. Play. The console resumes its last game on its own the next time you open it,
   like a real device switched back on.

The built-in **Help** tab explains the rest in English and French.

### What it does

**Playing.** A or Left arrow, B or Space, C or Right arrow, Up and Down turn the
dial. Held keys combine, which is how you revive a character. Every key and each
mouse button can be remapped, and the mapping is remembered.

**Saves.** A slot is a game: the character, its age, its gauges, its diary. Each
dump keeps its own slots. Starting a new game asks for a name and leaves the
current one untouched on disk. A slot can be deleted, with its recovery points,
after confirmation.

**Recovery points.** Not the same thing as a save. A recovery point freezes the
whole machine, core and peripherals included, and rewinds it to the second. One
is taken every minute and kept for twelve hours. They export and import as
files.

**Appearance.** The shell takes your own images: background, window paper, cap,
and a cut-out mask that replaces the console silhouette. Colours, opacity,
printed word, depth, shadows, layer rotation, screen size, button size and
spacing are all adjustable, per console.

**Serial link.** The console speaks over a UART at 460800 baud, the port through
which a real device receives items or plays with another one. The controller is
modelled and a bidirectional host bridge is in place. Transfers with external
tools are still being worked on.

**Browser view.** A local server publishes the screen and accepts the controls,
so the console can be watched or played from a phone on the same network.

### Where your files go

Everything lives in the system data folder, never next to the executable: a
program you move keeps its games, and a read-only folder blocks nothing.

| System | Folder |
|---|---|
| Windows | `%APPDATA%\Capybara\data` |
| macOS | `~/Library/Application Support/Capybara` |
| Linux | `~/.local/share/capybara` |

It holds imported dumps, saves, recovery points and serial captures. A folder
left over from an earlier name is moved there once, automatically.

### Status

| Area | State |
|---|---|
| ARMv7-M core, Thumb-2 | Runs the factory firmware faster than real time |
| Display, 128 x 128 RGB565 | Complete |
| Buttons, wheel, deep sleep and hardware wake | Complete |
| Real time clock, calendar, ageing | Complete |
| Persistent saves and recovery points | Complete |
| Sound | The notes are the ones the firmware composes; the output peripheral is not identified |
| Serial link, page `0x4000B000` | UART1 modelled, host bridge at 460800 baud, transfers not completing yet |
| Editions | The five run; Water and Jade Forest played end to end |

### Build

```
cargo build --release
```

The binary lands in `target/release/capybara`.

### For contributors

Forty-nine probes live in `examples/`, all taking `<dump.bin> <key hex>`. They
are the instruments the reverse engineering was done with, and they stay in the
repository because the work is not finished.

- `boot_probe`, the general one: regions visited, most executed addresses,
  registers touched with the program counter that touched them.
- `mmio_releve_probe`, one line per hardware register, made to be passed to
  `diff` between two runs. This is what found the serial port.
- `table_scenes_probe`, extracts the scene table of any edition without
  executing anything. Scene numbers differ between editions; never copy one
  across.
- `watch_probe`, stops at the Nth visit of an address or the Nth change of a
  word, and returns the real call stack.
- `veille_probe`, rebuilds the sleep state and replays a wake, without waiting
  for the inactivity timeout.

Tests that need a dump read two environment variables and skip cleanly without
them:

```
export SONIX_DEVICE_KEY=0x........
export SONIX_DUMPS=<folder holding the .bin files>
```

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
votre ordinateur. Ce n'est pas une reimplementation du jeu. C'est un coeur
ARMv7-M ecrit de zero en Rust, avec les peripheriques non documentes du
SNC73410 modelises a la mesure.

Le firmware demarre, l'oeuf eclot, l'horloge continue de tourner fenetre
fermee, les jauges descendent, la console s'endort et se reveille, et votre
sauvegarde survit a l'extinction de l'ordinateur.

### Ce qu'il vous faut d'abord

Capybara ne distribue aucune donnee de jeu, et ne le fera jamais. Vous
fournissez deux choses, tirees d'une console qui vous appartient.

- **Un dump de la flash** de votre propre appareil, 16 Mo, lu sur sa puce
  memoire.
- **La cle de l'appareil**, lue en SWD. Elle est gravee dans les fusibles de la
  puce, ne figure dans aucun dump, et ne se lit que sur l'appareil lui meme.

C'est la seule facon honnete et licite de se servir de l'emulateur : vous
emulez la machine que vous possedez deja. Sans dump ni cle, l'application
s'ouvre et vous les demande.

Le dossier a la racine du depot decrit les procedures d'extraction.

### Premiers pas

1. Prenez un executable dans les
   [releases](https://github.com/infinition/capybara/releases), ou compilez le.
2. Ouvrez Capybara et chargez votre dump. Il est recopie dans le dossier de
   donnees : il reste trouvable meme si vous deplacez l'original.
3. Posez la cle dans le dossier de donnees sous le nom `cle-device.txt`, ou a
   cote du dump dans un fichier portant le meme nom suivi de `.key`.
4. Jouez. La console reprend sa derniere partie toute seule a l'ouverture
   suivante, comme un vrai boitier qu'on rallume.

L'onglet **Aide** integre explique le reste, en francais et en anglais.

### Ce qu'il sait faire

**Jouer.** A ou Fleche gauche, B ou Espace, C ou Fleche droite, Fleche haut et
Fleche bas tournent la molette. Les touches tenues se combinent, c'est ainsi
qu'on ranime un personnage. Chaque touche et chaque bouton de la souris se
remappent, et le reglage est retenu.

**Sauvegardes.** Un emplacement est une partie : le personnage, son age, ses
jauges, son journal. Chaque dump garde les siens. Une nouvelle partie demande
un nom et laisse la precedente intacte sur le disque. Un emplacement s'efface,
avec ses points de reprise, apres confirmation.

**Points de reprise.** A ne pas confondre avec une sauvegarde. Un point fige
toute la machine, coeur et peripheriques compris, et permet de revenir en
arriere a la seconde pres. Un point est pris chaque minute et garde douze
heures. Ils s'exportent et s'importent en fichiers.

**Habillage.** La coque accepte vos images : fond, papier de la fenetre,
calotte, et un masque de decoupe qui remplace la silhouette de la console. Les
couleurs, l'opacite, le mot imprime, le relief, les ombres, la rotation du
calque, la taille de l'ecran, celle des boutons et leur ecartement se reglent,
console par console.

**Liaison serie.** La console parle par un UART a 460800 bauds, le port par
lequel un vrai boitier recoit des objets ou joue a deux. Le controleur est
modelise et un pont bidirectionnel vers l'hote est en place. Les transferts
avec les outils exterieurs sont encore en chantier.

**Vue navigateur.** Un serveur local publie l'ecran et accepte les commandes :
la console se regarde et se joue depuis un telephone sur le meme reseau.

### Ou vont vos fichiers

Tout vit dans le dossier de donnees du systeme, jamais a cote de l'executable :
un programme que vous deplacez garde ses parties, et un dossier en lecture
seule ne bloque rien.

| Systeme | Dossier |
|---|---|
| Windows | `%APPDATA%\Capybara\data` |
| macOS | `~/Library/Application Support/Capybara` |
| Linux | `~/.local/share/capybara` |

On y trouve les dumps importes, les sauvegardes, les points de reprise et les
captures de la liaison. Un dossier reste d'un ancien nom y est deplace une fois,
tout seul.

### Etat

| Domaine | Etat |
|---|---|
| Coeur ARMv7-M, Thumb-2 | Execute le firmware d'usine plus vite que le temps reel |
| Ecran, 128 x 128 RGB565 | Complet |
| Boutons, molette, veille profonde et reveil materiel | Complet |
| Horloge temps reel, calendrier, vieillissement | Complet |
| Sauvegardes persistantes et points de reprise | Complet |
| Son | Les notes sont celles que le firmware compose, la sortie n'est pas identifiee |
| Lien serie, page `0x4000B000` | UART1 modelise, pont hote a 460800 bauds, transferts pas encore aboutis |
| Editions | Les cinq tournent, Water et Jade Forest menees de bout en bout |

### Compiler

```
cargo build --release
```

Le binaire arrive dans `target/release/capybara`.

### Pour contribuer

Quarante-neuf sondes vivent dans `examples/`, toutes en `<dump.bin> <cle hex>`.
Ce sont les instruments avec lesquels la retro-ingenierie a ete faite. Elles
restent dans le depot parce que le travail n'est pas fini.

- `boot_probe`, la generaliste : zones parcourues, adresses les plus executees,
  registres touches avec le compteur de programme qui les touche.
- `mmio_releve_probe`, une ligne par registre materiel, faite pour etre passee a
  `diff` entre deux executions. C'est elle qui a trouve le port serie.
- `table_scenes_probe`, extrait la table des scenes de n'importe quelle edition
  sans rien executer. Les numeros de scene different d'une edition a l'autre :
  ne jamais en recopier un.
- `watch_probe`, s'arrete a la Nieme visite d'une adresse ou a la Nieme
  modification d'un mot, et rend la pile d'appels reelle.
- `veille_probe`, reconstruit l'etat de veille et rejoue un reveil, sans
  attendre le delai d'inactivite.

Les tests qui reclament un dump lisent deux variables d'environnement, et se
sautent proprement sans elles :

```
export SONIX_DEVICE_KEY=0x........
export SONIX_DUMPS=<dossier contenant les .bin>
```

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
