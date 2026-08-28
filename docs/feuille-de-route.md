# Feuille de route

Ce qui est demande et pas encore fait. L'etat courant est dans
`ou-on-en-est.md`, le point d'entree pour reprendre dans `reprise.md`.

## Defauts connus

**Le son de Jade Forest ne s'arretait jamais.** Le tableau des voix y est huit
octets plus loin que sur Water. Il est desormais cherche en memoire au lieu
d'etre code en dur, `Machine::localiser_les_voix`, et une entree n'est retenue
que si elle porte l'horloge du coeur en tete : sans ce controle, une structure
voisine tombant sur le meme pas passait pour une voix et sa valeur figee
s'entendait comme une note sans fin.

Le drapeau `SON_EN_COURS` en `0x18014284`, lui, n'a pas bouge : mesure sur Jade,
il est non nul sur cent pour cent des notes et nul sur cent pour cent des
silences. `drapeau_son_probe` refait cette mesure sur n'importe quelle edition.

A confirmer a l'oreille sur Jade avant de rayer la ligne.

**Un bourdonnement en tete des sons de Jade Forest.** Releve : quarante
secondes de 196 Hz avant que la melodie commence. Le drapeau de son en cours est
leve pendant tout ce temps et une voix porte cette valeur. A trancher : voix de
fond reelle du firmware, ou entree figee que le modele prend pour une note.

**La transparence du mode jeu.** Le fond restait noir au premier essai. La
fenetre nait desormais sans decor, ce qui est en general la condition pour
obtenir une fenetre transparente sur Windows, le decor etant remis a l'execution
pour l'accueil et l'inspection. A verifier.

## A faire

### Mode UART, pour parler au monde exterieur

Le port serie de la console sert a trois choses, par difficulte croissante.

- **Les logiciels PCOM.** Le firmware parle deja un protocole sur ce port ;
  `src/hw_bridge/uart_pcom.rs` en porte le debut. Il faut exposer le port a
  l'exterieur, par un port TCP local ou un port serie virtuel, pour qu'un
  logiciel PCOM existant s'y connecte comme a une vraie console.
- **Deux consoles emulees entre elles.** Une fois le port exposé, relier deux
  instances revient a croiser leurs flux. C'est ce qui permet les visites et les
  echanges entre Tamagotchi.
- **Une console physique et une emulee.** Meme chose, avec un adaptateur USB
  serie en face. C'est le test le plus severe du modele : le vrai firmware juge
  les reponses.

### Explorateur d'assets

Voir tout ce que le firmware porte en images, sprites et sons, dans une vue qui
les liste et les affiche. Puis pouvoir les sortir, les remplacer et les
reinjecter dans la flash. Le decodeur de sprites existe deja,
`src/gui/sprites.rs`, et la carte de la flash est dans
`src/hw_bridge/flash_map.rs`.

### Papiers de personnalisation

Sur la console, un cache plastique transparent entoure l'ecran et recoit un
papier imprime qu'on glisse dessous pour changer l'apparence. C'est prevu par le
fabricant, et chaque edition est livree avec les siens.

Il faut pouvoir importer une image, la glisser sous ce cache, la deplacer, la
zoomer et la recadrer, puis garder le reglage par edition. Le tour d'ecran est
deja dessine comme la plaque octogonale qui porte ce cache, dans
`src/ui/lcd_panel.rs` : c'est la que l'image viendrait se poser, sous le liseré
et derriere la dalle.

### Coques

Le dessin vectoriel est affine edition par edition. Restent le decor imprime
propre a chaque theme, corail et bulles pour Blue Water, feuillages pour Pink
Land, et la forme exacte de la fente de coquille.
