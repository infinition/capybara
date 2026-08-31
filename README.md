<div align="center">

<img src=".github/capybara.png" alt="Capybara" width="160">

# Capybara

**A bare-metal emulator for the Sonix SNC73410, compatible with Tamagotchi Paradise firmware.**

https://infinition.github.io/capybara/

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

<div align="center">
<img src="assets/screenshots/console.png" alt="Capybara running a Land edition dump" width="880">
</div>

<p align="center"><i>The console on the left, everything that drives it on the right. The shell here wears a paper of the owner's choosing.</i></p>

### What you need first

Capybara ships no game data, and never will. You supply two things, taken from a
console you own.

- **A flash dump** of your own device, 16 MB, read from its memory chip.
- **The device key**. It is burned into the chip fuses and appears in no dump,
  but Capybara works it out from your dump in about a minute: the AES key sits
  in the load table in clear, and only a thirty two bit value is missing. You
  can also read it over SWD if you prefer.

**Why it works this way.** Tamagotchi Paradise is a product still on sale, not
an abandoned one. Nothing here replaces buying it, and nothing here lets you
play without it: you must physically own a console, open it, and read its own
memory. What Capybara gives back is what you already paid for, on a screen that
does not run on a coin cell. That is the honest line, and the project holds it
even where it makes the software harder to start using.

Without a dump and its key, the application opens and asks for them.

### Getting started

1. Download an executable from
   [Releases](https://github.com/infinition/capybara/releases), or build one.
2. Open Capybara and load your dump. It is copied into the data folder, so it
   stays available even if you move the original.
3. Press **Find the key in the dump**. The key is worked out from your own
   dump in about a minute, and saved. Nothing is downloaded and nothing is
   supplied with the software. If you already know your key you can paste it in
   the field instead.
4. Play. The console resumes its last game on its own the next time you open it,
   like a real device switched back on.

The built-in **Help** tab explains the rest in English and French.

### What it does

**Playing.** A or Left arrow, B or Space, C or Right arrow, Up and Down turn the
dial. Held keys combine, which is how you revive a character. Every key and each
mouse button can be remapped, and the mapping is remembered.

Game mode drops the window frame and leaves the console alone on the desktop,
cut to its own shape. It can be dragged anywhere, kept above other windows, and
right-clicking it opens everything without going back to the panel.

<div align="center">
<img src="assets/screenshots/game-mode.png" alt="Game mode: the console alone on the desktop" width="360">
</div>

**Saves.** A slot is a game: the character, its age, its gauges, its diary. Each
dump keeps its own slots. Starting a new game asks for a name and leaves the
current one untouched on disk. A slot can be deleted, with its recovery points,
after confirmation.

**Recovery points.** Not the same thing as a save. A recovery point freezes the
whole machine, core and peripherals included, and rewinds it to the second. One
is taken every minute and kept for twelve hours. They export and import as
files.

<div align="center">
<img src="assets/screenshots/recovery-points.png" alt="Right-click menu listing recovery points by time" width="560">
</div>

<p align="center"><i>Right-click the console and pick an hour. Twelve hours of your game, minute by minute.</i></p>

**Appearance.** The shell takes your own images: background, window paper, cap,
and a cut-out mask that replaces the console silhouette. Colours, opacity,
printed word, depth, shadows, layer rotation, screen size, button size and
spacing are all adjustable, per console. Nothing is global: each console keeps
its own look.

<div align="center">
<img src="assets/screenshots/appearance-paper.png" alt="Appearance tab, with a photo used as the window paper" width="880">
</div>

<p align="center"><i>Every slider lands on the shell as you drag it. The screenshot shows a photo slid under the transparent window.</i></p>

The same tab holds the controls: which keys drive which button, and what the
three mouse buttons do when you click the screen instead of aiming at the small
buttons.

<div align="center">
<img src="assets/screenshots/appearance-controls.png" alt="Appearance tab, controls and button geometry" width="880">
</div>

**The mask** is worth a word. It is a black and white image that decides the
shape of the paper: black shows it, white hides it, and anything outside the
image is hidden too. Load this star as a mask and the paper appears as a star,
whatever the console's own silhouette is.

<div align="center">
<img src="assets/screenshots/mask-example.png" alt="A black star on white, used as a cut-out mask" width="150">
</div>

**Serial link.** The console speaks over a UART at 460800 baud, the port through
which a real device receives items or plays with another one. The controller is
modelled and a bidirectional host bridge is in place.

> **For now the link needs an intermediary.** Two programs cannot open the same
> COM port, and Capybara has no wire. You need a virtual serial port pairing
> tool, such as Virtual Serial Port Driver or com0com. This is a temporary
> requirement: a built-in transport that needs no driver at all is the next step.

It creates two COM ports wired back to back: Capybara
opens one end, the transfer tool opens the other. Both sides use 460800 baud, 8
data bits, no parity, 1 stop bit, and the same port cannot be opened twice.
Paired that way, a transfer tool talks to your console as if it were plugged in.

<div align="center">
<img src="assets/screenshots/uart-connecting.png" alt="The console connecting over the serial link" width="300">
&nbsp;&nbsp;
<img src="assets/screenshots/uart-item-received.png" alt="The console unlocking an item received over the link" width="380">
</div>

<p align="center"><i>The console asks for the link, then unlocks what arrived. This baobab was sent from the computer.</i></p>

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
| Sound | Complete |
| Serial link, page `0x4000B000` | UART1 modelled, host bridge at 460800 baud, needs a virtual COM pairing tool |
| Editions | Land, Water, Sky and Jade Forest tested. White Glacier and Orange Tropical are untested and may not run yet |

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

Everything the emulator relies on was measured on the hardware, not assumed:
the pinout, the real memory map, the Sonix load table format, the AES key
derivation, and the sixteen ARMv7-M decoding faults found by running real code.

### Support

If this work is useful to you:

<a href="https://www.buymeacoffee.com/infinition"><img src="https://img.shields.io/badge/Buy%20me%20a%20coffee-infinition-yellow" alt="Buy me a coffee"></a>

### Legal

Capybara is an independent work of reverse engineering for interoperability. It
is not affiliated with, endorsed by, or connected to Bandai. Tamagotchi and
Tamagotchi Paradise are trademarks of Bandai. No firmware, no ROM, no key and no
graphic or sound asset from the console is contained in this repository or in
the published executables. Everything it needs is read from a device you own.

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

<div align="center">
<img src="assets/screenshots/console.png" alt="Capybara faisant tourner un dump de l'edition Land" width="880">
</div>

<p align="center"><i>La console a gauche, tout ce qui la commande a droite. Ici la coque porte un papier choisi par son proprietaire.</i></p>

### Ce qu'il vous faut d'abord

Capybara ne distribue aucune donnee de jeu, et ne le fera jamais. Vous
fournissez deux choses, tirees d'une console qui vous appartient.

- **Un dump de la flash** de votre propre appareil, 16 Mo, lu sur sa puce
  memoire.
- **La cle de l'appareil**. Elle est gravee dans les fusibles de la puce et ne
  figure dans aucun dump, mais Capybara la retrouve depuis votre dump en une
  minute environ : la cle AES est en clair dans la table de chargement, et il ne
  manque qu'une valeur de trente deux bits. Vous pouvez aussi la lire en SWD.

**Pourquoi c'est fait ainsi.** Tamagotchi Paradise est un produit toujours en
vente, pas une console abandonnee. Rien ici ne remplace son achat, et rien ici
ne permet d'y jouer sans elle : il faut posseder physiquement un boitier,
l'ouvrir, et lire sa propre memoire. Ce que Capybara vous rend, c'est ce que
vous avez deja paye, sur un ecran qui ne tient pas sur une pile bouton. C'est
la ligne honnete, et le projet s'y tient meme quand elle rend le logiciel plus
difficile a prendre en main.

Sans dump ni cle, l'application s'ouvre et vous les demande.

### Premiers pas

1. Prenez un executable dans les
   [releases](https://github.com/infinition/capybara/releases), ou compilez le.
2. Ouvrez Capybara et chargez votre dump. Il est recopie dans le dossier de
   donnees : il reste trouvable meme si vous deplacez l'original.
3. Appuyez sur **Chercher la cle dans le dump**. Elle se deduit de votre propre
   dump en une minute environ, et elle est enregistree. Rien n'est telecharge et
   rien n'est fourni avec le logiciel. Si vous connaissez deja votre cle, collez
   la dans le champ a la place.
4. Jouez. La console reprend sa derniere partie toute seule a l'ouverture
   suivante, comme un vrai boitier qu'on rallume.

L'onglet **Aide** integre explique le reste, en francais et en anglais.

### Ce qu'il sait faire

**Jouer.** A ou Fleche gauche, B ou Espace, C ou Fleche droite, Fleche haut et
Fleche bas tournent la molette. Les touches tenues se combinent, c'est ainsi
qu'on ranime un personnage. Chaque touche et chaque bouton de la souris se
remappent, et le reglage est retenu.

Le mode jeu retire le cadre de la fenetre et laisse la console seule sur le
bureau, decoupee a sa forme. Elle se deplace ou l'on veut, se maintient au
dessus des autres fenetres, et un clic droit dessus ouvre tout sans repasser par
le panneau.

<div align="center">
<img src="assets/screenshots/game-mode.png" alt="Mode jeu : la console seule sur le bureau" width="360">
</div>

**Sauvegardes.** Un emplacement est une partie : le personnage, son age, ses
jauges, son journal. Chaque dump garde les siens. Une nouvelle partie demande
un nom et laisse la precedente intacte sur le disque. Un emplacement s'efface,
avec ses points de reprise, apres confirmation.

**Points de reprise.** A ne pas confondre avec une sauvegarde. Un point fige
toute la machine, coeur et peripheriques compris, et permet de revenir en
arriere a la seconde pres. Un point est pris chaque minute et garde douze
heures. Ils s'exportent et s'importent en fichiers.

<div align="center">
<img src="assets/screenshots/recovery-points.png" alt="Menu du clic droit listant les points de reprise par heure" width="560">
</div>

<p align="center"><i>Clic droit sur la console, puis une heure. Douze heures de partie, minute par minute.</i></p>

**Habillage.** La coque accepte vos images : fond, papier de la fenetre,
calotte, et un masque de decoupe qui remplace la silhouette de la console. Les
couleurs, l'opacite, le mot imprime, le relief, les ombres, la rotation du
calque, la taille de l'ecran, celle des boutons et leur ecartement se reglent,
console par console. Rien n'est global : chaque console garde son allure.

<div align="center">
<img src="assets/screenshots/appearance-paper.png" alt="Onglet Personnalisation, avec une photo posee en papier de fenetre" width="880">
</div>

<p align="center"><i>Chaque curseur se voit sur la coque pendant qu'on le tire. Ici une photo glissee sous la fenetre transparente.</i></p>

Le meme onglet porte les commandes : quelles touches tiennent quel bouton, et ce
que font les trois boutons de la souris quand on clique sur l'ecran au lieu de
viser les petites pastilles.

<div align="center">
<img src="assets/screenshots/appearance-controls.png" alt="Onglet Personnalisation, commandes et geometrie des boutons" width="880">
</div>

**Le masque** merite un mot. C'est une image en noir et blanc qui decide de la
forme du papier : le noir le laisse voir, le blanc le cache, et ce qui tombe
hors de l'image est cache aussi. Chargez cette etoile comme masque et le papier
apparait en etoile, quelle que soit la silhouette de la console.

<div align="center">
<img src="assets/screenshots/mask-example.png" alt="Une etoile noire sur fond blanc, utilisee comme masque de decoupe" width="150">
</div>

**Liaison serie.** La console parle par un UART a 460800 bauds, le port par
lequel un vrai boitier recoit des objets ou joue a deux. Le controleur est
modelise et un pont bidirectionnel vers l'hote est en place.

> **Pour l'instant la liaison reclame un intermediaire.** Deux programmes ne
> peuvent pas ouvrir le meme port COM, et Capybara n'a pas de fil. Il faut donc
> un logiciel d'appairage de ports serie virtuels, comme Virtual Serial Port
> Driver ou com0com. C'est une contrainte provisoire : un transport interne, qui
> ne demande aucun pilote, est la prochaine etape.

Il cree deux ports COM relies dos a dos :
Capybara ouvre un bout, l'outil de transfert ouvre l'autre. Les deux cotes
utilisent 460800 bauds, 8 bits de donnees, aucune parite, 1 bit d'arret, et un
meme port ne s'ouvre pas deux fois. Appairee ainsi, un outil de transfert parle
a votre console comme si elle etait branchee.

<div align="center">
<img src="assets/screenshots/uart-connecting.png" alt="La console en train d'etablir la liaison serie" width="300">
&nbsp;&nbsp;
<img src="assets/screenshots/uart-item-received.png" alt="La console debloquant un objet recu par la liaison" width="380">
</div>

<p align="center"><i>La console demande la liaison, puis debloque ce qui est arrive. Ce baobab a ete envoye depuis l'ordinateur.</i></p>

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
| Son | Complet |
| Lien serie, page `0x4000B000` | UART1 modelise, pont hote a 460800 bauds, demande un logiciel d'appairage de ports |
| Editions | Land, Water, Sky et Jade Forest eprouvees. White Glacier et Orange Tropical ne sont pas testees et peuvent ne pas fonctionner |

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

Tout ce sur quoi l'emulateur repose a ete mesure sur le materiel, pas suppose :
le brochage, la vraie carte memoire, le format des load tables Sonix, la
derivation de cle AES, et les seize defauts de decodage ARMv7-M trouves en
faisant tourner du vrai code.

### Soutenir

Si ce travail vous est utile :

<a href="https://www.buymeacoffee.com/infinition"><img src="https://img.shields.io/badge/Buy%20me%20a%20coffee-infinition-yellow" alt="Buy me a coffee"></a>

### Mentions legales

Capybara est un travail independant de retro-ingenierie a des fins
d'interoperabilite. Il n'est ni affilie, ni approuve, ni lie a Bandai.
Tamagotchi et Tamagotchi Paradise sont des marques de Bandai. Aucun firmware,
aucune ROM, aucune cle et aucun element graphique ou sonore de la console n'est
contenu dans ce depot ni dans les executables publies. Tout ce dont il a besoin
se lit sur un appareil dont vous etes proprietaire.

Distribue sous licence GNU General Public License v3.0. Voir [LICENSE](LICENSE).
