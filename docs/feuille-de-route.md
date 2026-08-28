# Feuille de route

Ce qui reste a faire. L'etat courant est dans `ou-on-en-est.md`, le point
d'entree pour reprendre dans `reprise.md`.

## Defauts connus

**Un bourdonnement en tete des sons de Jade Forest.** Releve a la sonde :
quarante secondes de 196 Hz, un Sol3 juste, avant que la melodie commence. Le
drapeau de son en cours est leve pendant tout ce temps et une voix porte cette
valeur. A trancher : voix de fond reelle du firmware, ou entree figee que le
modele prend pour une note. `melodie_probe` le montre en une commande.

**Le peripherique de sortie du son reste introuvable.** Le moteur du firmware
calcule ses notes mais n'ecrit sur aucun peripherique connu. Voir la section
« Le son » de `ou-on-en-est.md` pour ce qui a ete mesure.

La lecture en periode change la recherche, et c'est ce qu'il faut essayer en
premier : le registre attendu ne recoit pas un diviseur de 96 MHz par la
frequence, mais le compte lui meme, 568 ou 1516, sur une base a 1,5 MHz. C'est
ce nombre la qu'il faut chercher dans toutes les ecritures materielles pendant
une note, sans filtre de PC, avec un prediviseur a 64 pose quelque part.
Trancher cela reglerait aussi la question du volume : si la console le regle en
modulant le rapport cyclique, notre carre a cinquante pour cent est faux en
nature et pas seulement en timbre.

**Les quatre autres editions ne sont pas menees jusqu'au bout.** Seule Water a
ete jouee de bout en bout. Jade Forest affiche et sonne. Earth, Sky et Land
n'ont pas ete suivies jusqu'a l'image.

## A faire

### Mode UART, pour parler au monde exterieur

Le port serie de la console sert a trois choses, par difficulte croissante.

- **Les logiciels PCOM.** Le firmware parle deja un protocole sur ce port ;
  `src/hw_bridge/uart_pcom.rs` en porte le debut. Il faut exposer le port a
  l'exterieur, par un port TCP local ou un port serie virtuel, pour qu'un
  logiciel PCOM existant s'y connecte comme a une vraie console.
- **Deux consoles emulees entre elles.** Une fois le port expose, relier deux
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

### Timbre du buzzer

Le vrai Tamagotchi a un piezo dans une coque plastique : un transducteur tres
resonant, un pic entre deux et quatre kilohertz, presque rien en dessous d'un
kilohertz. Un haut parleur d'ordinateur restitue le fondamental en entier, d'ou
un son plus plein et plus grave que la console. Le signal electrique est le
meme, c'est le haut parleur qui differe.

Trois facons de s'en rapprocher, par cout croissant : un passe haut vers un
kilohertz avec une resonance, une vingtaine de lignes dans `audio.rs` ; une
reponse impulsionnelle relevee au micro sur la vraie console, puis convolution ;
ou l'echantillonnage des quatre vingt sept sons du banc, ce qui n'est plus de
l'emulation.

### Coques

Le dessin suit la vraie console : oeuf au rapport 6,5 sur 7,5, fente de
coquille, fenetre transparente decoupee sans trait, molette cannelee, boutons
d'une couleur qui tranche. Reste le decor imprime propre a chaque theme, corail
et bulles pour Blue Water, feuillages pour Pink Land. C'est moins urgent depuis
qu'on peut glisser son propre papier sous la fenetre.

## Fait

Pour memoire, et pour ne pas le refaire.

- La vitesse, de 0,73 a 1,00 fois le temps de la console.
- Le son : le champ de hauteur est une periode et non une frequence, base
  750 000. Les melodies ne sont plus a l'envers, ne trainent plus, ne gresillent
  plus, et ne jouent plus la note heritee de la precedente.
- Le tableau des voix est cherche en memoire : son adresse change d'une edition
  a l'autre, et c'est ce qui rendait Jade Forest muette.
- Trois modes, dont un mode jeu sans cadre, transparent, deplacable, avec tout
  au clic droit. Il a fallu passer au moteur wgpu : sur Windows le compositeur
  du bureau ignore la couche alpha des fenetres OpenGL.
- Points de reprise horodates, par sauvegarde, avec pose, import et export.
- Papiers de personnalisation glisses sous la fenetre transparente.
- Donnees dans le dossier du systeme, dumps detectes et importes, cle qui suit.
- Reglages tous retenus d'un lancement a l'autre.
