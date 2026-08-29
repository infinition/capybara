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

**Ce qu'attend l'outil officieux, releve dans TamaHome 5.0.6.** C'est une
application .NET Avalonia, `TamaParadiseTool.dll`. Elle embarque
`System.IO.Ports` et appelle `SerialPort.GetPortNames` : elle liste donc tous
les ports serie du systeme, sans filtrer sur un identifiant de fabricant, et
retient le dernier choisi dans son reglage `LastComPort`. N'importe quel port
fait donc l'affaire.

Son service de communication porte `deviceComm`, `configureComm`,
`getActiveComm` et un type `TcpComm`, mais ce dernier sert au relais en ligne,
`OnlineRelayBridge` avec ses pompes `PumpSerialToSocket` et
`PumpSocketToSerial` : c'est le flux serie qui est renvoye dans une prise
reseau pour jouer a deux, pas une seconde voie vers l'appareil. Le transport
vers la console reste le port serie.

L'echange lui meme s'appelle une visite : `PlaydateWireData`,
`ParsePlaydateData`, `SendPlaydate`, `PlaydateIdentity`.

**L'obstacle, a savoir avant de commencer.** Windows n'enumere en COMxxx que ce
qu'un pilote declare. Aucune application ne peut creer un port serie toute
seule. Il faut donc une paire de ports virtuels, com0com ou equivalent, que
l'utilisateur installe une fois : elle donne par exemple COM5 et COM6 relies
dos a dos, l'emulateur ouvre l'un, TamaHome voit l'autre comme un port
ordinaire. C'est la seule voie sans ecrire de pilote.

**Ce qu'on a trouve dans le firmware, et par ou continuer.**

La table des scenes est decodee : un tableau de descripteurs de vingt huit
octets, quatre gestionnaires, un pointeur de nom et un compteur. `table_scenes_probe`
l'extrait de n'importe quelle edition sans rien executer, en partant des chaines
`PSID_` qui sont en clair dans l'image. Sur Jade Forest elle est en `0x1008128c`
et compte cent vingt neuf entrees.

**Le numero de scene est le rang dans le tableau, pas le champ `+0x10`.** Ce
champ vaut le rang plus un partout, et le prendre pour le numero decale tout
d'une unite. C'est l'erreur qui a fait conclure a tort sur l'ecran de connexion.
Deux mesures tranchent, et elles concordent :

- La mise en route d'une partie neuve donne `0`, `104`, `105`, `109`, `113`. En
  rang cela se lit `STARTUP`, `INITIAL_LOGO`, `LANGUAGE`, `SYSTEM_POWERDOWN`,
  soit l'extinction apres vingt et une secondes sans appui sur le choix de la
  langue. Au champ `+0x10`, la premiere scene serait `OSEWAGAME_04`.
- Les vues de zoom relevees a l'oeil, 29, 30 et 32, tombent en rang sur
  `HOME_SPACE`, `HOME_FIELD` et `HOME_CELL`.

Les scenes qui nous interessent, en rang, sur Jade Forest :

```text
   0  PSID_DEBUGMENU            16  PSID_DEVELOP_UARTTEST
  23  PSID_DEVELOP_TCP          17  PSID_DEVELOP_UARTAGEING
  28  PSID_HOME                 18  PSID_DEVELOP_UARTBYTE
 115  PSID_TAMASPACE_TUSHIN     19  PSID_DEVELOP_UARTHEADER
 121  PSID_TAMASPACE_RANK      127  PSID_TESTMODE
```

Le firmware porte aussi un `** UART Monitor Service Ver.%d.%02d **` et des
entrees de menu `UART BYTE`, `UART AGEING`, `UART TEST` et `TAMA HOME`.

**Les quatre scenes UART sont des scenes de developpement.** Elles ne dependent
d'aucun avancement dans le jeu, contrairement a `TAMASPACE_TUSHIN` que le niveau
de planete verrouille. Y entrer reglerait la question du peripherique sans avoir
a jouer, et `PSID_DEBUGMENU` en rang 0 est le menu qui y mene.

**`0x40038000` n'est pas le port serie.** J'ai cru le contraire une heure
durant : la page ressort douze fois dans la fenetre XIP, seule de son groupe,
et le code la scrute bit a bit. Le pilote lu jusqu'au bout dit autre chose.

Sa fonction de configuration, en `0x10005560`, recoit une adresse qu'elle
compare a `0x18000000`, donc un tampon en memoire vive, et une longueur. Sa
fonction d'attente, en `0x10005534`, boucle sur le temoin d'etat, coupe
l'autorisation de marche, puis lit `+0x1C` et le rend a l'appelant. Une adresse,
une longueur, un depart, une attente, un resultat : c'est un accelerateur qui
calcule une valeur sur un bloc de memoire. La note heritee avait raison, et
c'est moi qui l'ai mise en doute a tort.

La banque reste utile a consigner, elle servira le jour ou ce bloc sera
modelise :

```text
  +0x00  controle et etat. Bit 4 : autorisation de marche, efface avant de
         configurer, repose pour lancer, efface a la fin. Bits 4 et 9 scrutes
         comme temoins d'achevement.
  +0x04  adresse du tampon, verifiee en memoire vive
  +0x08  longueur
  +0x0C  mis a 1 a la configuration
  +0x10  mis a 1 a la configuration
  +0x14  format : deux bits de mode, et un champ de cinq bits en 4..8 qui
         recoit un nombre moins un
  +0x18  parametre pose d'un bloc
  +0x1C  resultat, lu a la fin
```

**Ce qu'on sait du lien physique.** 460800 bauds, niveaux 3,3 V, adaptateur de
type CH341A ou CH347. Cela vient de la communaute, pas de notre mesure, mais
c'est une valeur precise a confronter au diviseur qu'on trouvera.

## Le port serie, trouve

**Il est en `0x4000B000`.** Le datasheet l'annonce en SAR_ADC1, une etiquette de
plus a jeter. La page n'apparait que lorsque le lien s'ouvre, et elle apparait a
ce moment la seulement : c'est la mesure qui le dit, pas une lecture de code.

**Comment y arriver a la main**, depuis la vue Space. Le menu de communication
est la scene 115, `TAMASPACE_TUSHIN`. `A` deplace le curseur, `B` valide, `C`
revient. Ses quatre entrees mènent aux scenes voisines, et chacune correspond a
un onglet de TamaHome :

| Entree du menu | Scene | Onglet de TamaHome | Ce qui passe |
|---|---|---|---|
| s'amuser | 116 `TAMASPACE_PLAY` | Simulate Playdate | la visite entre deux consoles |
| cadeaux | 117 `TAMASPACE_ITEM` | Built-In Items | les objets du catalogue |
| planetes | grisee | | |
| telechargement | 119 `TAMASPACE_DOWNLOAD` | Lab Items | les objets de laboratoire |

Sur « s'amuser », la scene 116 affiche « connecte deux appareils et appuie sur le
bouton B sur chacun d'eux ». C'est ce second `B` qui ouvre le lien, et c'est le
geste que TamaHome demande aussi : « press B and wait for the searching screen ».
Les quatre voies passent vraisemblablement par le meme transport, seul le
contenu change.

L'onglet « Online Bridge » de TamaHome ne parle pas a la console autrement : il
relaie le meme flux serie dans une prise reseau pour jouer a deux a distance.

**Ce qui n'apparait qu'a ce moment la**, releve par `mmio_releve_probe` en
comparant deux executions, l'une en jeu ordinaire, l'autre sur le lien :

```text
  0x4000B000  +0x00  ecrit en dernier a l'emission, depuis 0x00004E42
  0x4000B000  +0x04  ecrit juste avant, depuis 0x00004E3C
  0x4000B000  +0x08  configuration, depuis 0x00004C26
  0x4000B000  +0x0C  controle de ligne, depuis 0x00004B9C
  0x4000B000  +0x14  etat, scrute des dizaines de milliers de fois par
                     seconde depuis 0x00004EC0, jamais ecrit
  0x4000B000  +0x28  ecrit en premier a l'emission, depuis 0x00004E36
  0x4000B000  +0x30  configuration, depuis 0x00004C60
```

**Le registre d'etat, `+0x14`.** Deux bits sont lus, et rien d'autre.

```text
  0x00004EC0  LDR r0,[base,#0x14] ; UBFX r0,r0,#10,#1 ; CMP r0,#1
  0x00004EDA  LDR r0,[base,#0x14] ; LSLS r0,r0,#25 ; BMI ; sinon boucle
```

Le second est une boucle qui ne sort que sur le **bit 6**, la forme habituelle
d'un « emetteur pret ». Le **bit 10** est teste une fois avant. La page n'etant
pas modelisee, elle rend zero, le bit 6 ne monte jamais et le firmware tourne en
rond : c'est ce qu'on observe, des dizaines de milliers de lectures par seconde
et rien d'autre.

**Le controle de ligne, `+0x0C`.** Deux champs, poses depuis une structure de
reglages :

```text
  0x00004B9C  LDR r0,[base,#0x0C] ; BFI r0,r2,#0,#2 ; STR   bits 0 et 1
  0x00004BAA  LDR r0,[base,#0x0C] ; BIC #4 ; ORR r2 LSL #2  bit 2
```

Deux bits puis un bit isole, c'est la forme d'une longueur de mot et d'un
temoin de parite ou de bit d'arret.

**La cadence.** La fonction en `0x000047C4` rend la frequence de l'horloge
peripherique : les trois bits bas de `0x4500000C` choisissent la source, par une
table de sauts, dont une branche vaut douze millions ; `0x45000010` porte
ensuite un decalage a droite. C'est cette valeur qu'un pilote divise pour
obtenir ses bauds, et c'est la qu'il faudra retomber sur les 460800 annonces.

**Une jumelle en `0x4000A000`.** Son `+0x14` est lu au meme moment, depuis
`0x00004DD4`, un code voisin de celui de `0x4000B000`. Deux exemplaires du meme
bloc, donc, ce qui cadre avec le `** UART Monitor Service **` que porte le
firmware : l'une des deux est vraisemblablement la console de mise au point,
l'autre le lien vers l'exterieur.

**Ce qui reste a faire dessus.** Modeliser le bloc, en commencant par rendre le
bit 6 de `+0x14` toujours pose pour debloquer la boucle d'emission, puis relever
ce que le firmware ecrit en `+0x00`, `+0x04` et `+0x28`. `MMIO_FORCE` suffit
pour le premier essai, mais il ne suffit pas seul : force a `0x40`, le firmware
ne progresse pas encore, il faut donc comprendre le bit 10 avant.

**Attention aux balayages de pages.** Le balayage des paires MOVW puis MOVT sur
les seize mega-octets ne voyait pas `0x4000B000`, parce que l'adresse ne se
forme pas par une paire immediate mais se lit dans une structure. Il avance en
plus de deux octets en deux octets sans suivre les frontieres d'instruction,
donc il decode aussi des donnees. La comparaison de deux executions, elle, ne
suppose rien : c'est ce qui a trouve la page.

**Une page tres sollicitee que personne n'a modelisee : `0x4000E000`.** Le
datasheet l'annonce en I2S4. Releve sur Jade Forest et sur Water, avec le meme
resultat : une banque entiere y est lue et ecrite en permanence, `+0x00`,
`+0x04`, `+0x08`, `+0x0C`, `+0x1C`, `+0x20`, `+0x24`, `+0x50`, plus d'un millier
d'acces par releve, toujours depuis le meme code en memoire programme, autour de
`0x000052F2`, `0x0000546A`, `0x00005404`, `0x0000548E`, `0x0000532C` et
`0x00003FDA`. Un canal du controleur de transferts, `0x4000F01C` a `0x4000F028`,
est mene par le code voisin en `0x00004440`.

Deux choses la rendent interessante. C'est la seule page inconnue de ce poids.
Et elle est pilotee depuis la memoire programme, alors que la recherche du
peripherique de son filtrait les acces sur l'intervalle de PC du module audio,
en `0x1001F000` a `0x10080000` : elle ne pouvait pas la voir. C'est exactement
le trou de methode qu'on soupconnait.

Ce qu'elle est reste a etablir, mais un chiffre penche deja. Son canal de
transfert compte quatre vingt quatorze operations par releve, et le meme releve
compte quatre vingt quatorze trames poussees vers l'ecran. Le trafic suit donc
la cadence d'affichage et non celle du son : c'est vraisemblablement l'interface
serie de la dalle, pas le buzzer. A confirmer en relevant son trafic pendant
qu'une note joue puis au silence, mais il ne faut pas s'emballer dessus.

**L'ecran de connexion ne reveille aucun peripherique nouveau, et la mesure ne
vaut rien.** L'instantane portait 119 en `0x18001BF4`, pris pour
`PSID_TAMASPACE_RANK` en numerotation d'Earth. Avec la table de Jade et le rang
comme numero, 119 est `PSID_TAMASPACE_DOWNLOAD`. La console n'etait donc pas sur
l'ecran de connexion, et le releve des onze pages ne dit rien du lien serie. A
refaire une fois `TAMASPACE_TUSHIN`, rang 115, reellement atteint.

**Ce qui ne marche pas, pour ne pas le refaire.** Ecrire le numero de scene
voulu en `0x18001BF6` ne declenche aucune transition. Ce mot vaut `0xFFFF` au
repos, ce qui ressemble bien a « aucune transition demandee », mais y poser un
numero ne suffit pas : le firmware attend autre chose en plus. `uart_probe`
fait cet essai en une commande. A noter que cet essai a ete mene avec les
numeros decales d'un rang, il est donc a refaire proprement avant de conclure.

**La machine a scenes, telle qu'elle est lue.** Elle vit en `0x000096C8`,
appelee a chaque tour de la boucle principale depuis `0x000095E4`. Son etat
tient dans les trois bits bas de `0x18001BFA` :

```text
  0  entree : lit la scene en 0x18001BF4, cherche son objet par 0x0000C492,
     ecrit 0xFFFF en 0x18001BF6, puis appelle le gestionnaire +4
  1  marche : appelle le gestionnaire +8 a chaque tour, puis +12
  2  sortie et veille, en 0x00009886
```

L'etat avance par `(etat + 1) & 7` en `0x00009824`, les bits hauts conserves.
La scene de demarrage, 104, est posee en dur en `0x0000956C`.

**Forcer une scene par la memoire se heurte au tas, pas a la machine a
scenes.** `scene_forcee_probe` ecrit le numero voulu en `0x18001BF4` et remet
les trois bits d'etat a zero. La machine obeit : elle entre bien dans la scene
demandee. Mais l'entree alloue, la scene quittee n'a rien rendu, et
l'allocateur saute au halt fatal de Jade Forest. Le chemin est net et se relit
en une commande : marche des blocs en `0x100166DA` et `0x10016704`, saut en
`0x10016746`, boucle morte `NOP` puis `B` en `0x1005E91C`. Quatre vingt dix
neuf pour cent du temps y passe.

Forcer plus tot ne sert a rien non plus : avant neuf dixiemes de seconde la
mise en route ecrase l'ecriture et repart sur `HOME_SPACE`. La fenetre utile
n'existe pas.

**Par ou continuer, donc.** Il faut que la scene quittee rende sa memoire,
c'est a dire passer par la sortie que le firmware prevoit plutot que par la
memoire. Le gestionnaire `+8` rend une valeur non nulle pour demander la
sortie, et c'est elle qui fait avancer l'etat. Reproduire cela, ou trouver la
fonction que le menu de mise au point appelle pour changer de scene, est le
prochain pas. `PSID_DEBUGMENU` en rang 0 est la cible : depuis lui, la
navigation vers les quatre scenes UART se fait par le firmware lui meme, qui
gere son tas correctement.

**Ce qui manque cote emulateur, et qui vient avant.** Le port serie n'est pas
modelise. `src/hw_bridge/uart_pcom.rs` et `src/emulator/peripherals/uart.rs`
datent d'avant la retro-ingenierie : registres inventes, et jusqu'a des
compteurs d'octets ecrits en dur. A jeter plutot qu'a completer.

Et le peripherique n'est pas identifie. Mesure sur une partie ordinaire :
aucune page serie n'est touchee, ni UART1 en `0x40034000`, ni SPI1, ni I2C1.
C'est attendu, le lien ne s'ouvre que dans le menu de connexion. Le premier
travail est donc d'atteindre ce menu avec `scene_probe` et de regarder quelle
page s'allume, exactement comme pour le buzzer.

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
