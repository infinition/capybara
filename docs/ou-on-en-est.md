# Passation : etat exact, et ou ca en est

Document de reprise. Il dit ce qui est etabli, ce qui reste a faire, et par ou
attaquer. Le detail materiel est dans `materiel-snc7340.md`.

## Resume

**Le jeu se joue.** Mise en route complete, boutons, menus, molette de zoom,
horloge qui avance, oeuf qui eclot, nourrissage, QR code secret, veille et
reveil, sauvegardes persistantes qui survivent a l'extinction de l'ordinateur et
vieillissement en temps reel, coque fidele suivant l'edition, son de la console.

L'ecran fait 128 x 128 en RGB565. La cle de dechiffrement est commune aux cinq
editions ; elle vient de `SONIX_DEVICE_KEY` et n'est nulle part dans le depot.
Voir `reprise.md`.

**Il n'y a plus de defaut visible a l'ecran.** La vitesse a longtemps ete le
dernier : l'emulation plafonnait a 0,78 fois le temps de la console, la planete
de l'ecran d'accueil oscillait toutes les deux secondes au lieu d'une et les
melodies trainaient d'autant. Depuis le 28 aout 2026 elle tient 1,40 fois le
temps reel sur `step` et 1,37 sur `run_frame`, et le regulateur de l'interface la
retient a 1,00. Voir la section « La vitesse ».

Ce qui reste ouvert ne se voit pas : le peripherique par lequel la console fait
sortir ses notes n'est toujours pas identifie.

## Outils

Vingt-deux sondes dans `examples/`. Toutes prennent `<dump.bin> <cle hex>`.
Les plus utilisees sont detaillees ci-dessous.

- **`boot_probe`** : le couteau suisse. Zones parcourues, adresses les plus
  executees, desassemblage de la boucle chaude, tous les registres touches avec
  le PC qui les touche, transferts de flash, etat des interruptions avec le mode
  du coeur, et la console de debug du firmware.
- **`watch_probe`** : la plus rentable. S'arrete a la Nieme visite d'une adresse
  ou a la Nieme modification d'un mot, et rend registres, pile d'appels reelle,
  trace des pas executes avant et apres l'arret.
- **`dis_probe`** : desassemble a froid un intervalle. Programme la fenetre XIP
  avant de lire, sans quoi tout le code au dela de `0x10000000` se lit decale de
  `0x11000`.
- **`ecran_probe`** : rend le tampon d'image en PPM et rejoue des appuis.
- **`irq_probe`** : releve entrees et sorties d'exception, et designe celle qui
  ne revient pas.
- **`spin_probe`**, **`race_probe`** : boucles mortes et courses.
- **`scene_probe`** : repart d'un instantane, rejoue des appuis en secondes de
  temps console, suit scene, horloge et compteur d'inactivite au fil de
  l'execution, rend l'ecran et la raison exacte de l'arret. `RESET=1` rallume
  la console sur la flash de l'instantane, `SORTIE_ETAT` ecrit l'etat atteint,
  `TRACE_PAS` garde les dernieres adresses executees.
- **`temps_probe`** : nomme les registres scrutes sans modele derriere, avec le
  PC responsable, compte les entrees en exception par gestionnaire, et donne
  l'histogramme des adresses executees. C'est elle qui montre une boucle morte.
- **`tas_probe`** : parcourt le tas du firmware et rend blocs, trous et plus
  grand trou disponible, au depart puis au fil du temps.
- **`alloc_probe`** : compte les prises et les rendus de memoire par appelant et
  par taille, aux deux entrees de l'allocateur.
- **`partie_probe`** : ecrit un emplacement de sauvegarde depuis un instantane,
  puis rallume la console dessus. Le cycle complet en une commande.
- **`son_probe`** : rejoue des appuis jusqu'a ce que le firmware joue une note,
  puis releve les voix, leur frequence, et tout ce que la console touche pendant
  ce temps. `MODULE_AUDIO=bas-haut` restreint la trace au code d'un module.

Variables d'environnement :

| Variable | Effet |
|---|---|
| `MMIO_PAGE=0x...` | journalise dans l'ordre les acces a une page de 4 Ko |
| `MMIO_ECR=1` | n'y journalise que les ecritures |
| `MMIO_FORCE=adr:val,...` | impose une valeur de lecture sur un registre non modelise |
| `WATCH_COND=r6:0x...` | ne declenche `watch_probe` que si un registre vaut une valeur |
| `WATCH_MEM=0x...` | arrete `watch_probe` a la modification d'un mot de SRAM |
| `WATCH_DEPUIS=N` | ignore les modifications d'avant le Nieme pas |
| `TRACE_PAS=N` | rend les N derniers pas executes avant l'arret |
| `TRACE_APRES=N` | rend les N pas suivant l'arret |
| `MEM_DUMP=0x...` | vidange 64 octets a une adresse |
| `MEM_CMP=a:b:long` | compare deux zones et signale le premier ecart |
| `XIP_BASE=0x...` | base de la fenetre XIP pour `dis_probe` |
| `ECRAN_DEPART=n` | arrete `ecran_probe` au nieme transfert vers l'afficheur |
| `ENTREES=pas:broche:duree,...` | rejoue des appuis, en pas d'execution |
| `SONIX_FLASH_ID=0x...` | impose la paire d'identification de la flash |
| `SONIX_PILE=0x...` | impose l'echantillon de mesure de pile |
| `PILE_USEE=1` | laisse le drapeau de pile faible du dump en place |

Exemple, pour valider le choix de langue et atteindre le reglage de la date :

```bash
ENTREES=400000000:08:3000000 ECRAN_DEPART=600 cargo run --release --example ecran_probe -- dump.bin <CLE> ecran.ppm 128 4000000000
```

## Cinq lecons payees cher

**Ne jamais croire une etiquette sur parole.** Le datasheet place GPIO2 en
`0x4002F000` : c'est le controleur XIP. UART0 en `0x40038000` : c'est un
accelerateur de somme de controle. I2S0 en `0x40019000` : c'est le port
d'entrees-sorties numero 1. Le chien de garde en `0x4003A000` : c'est le
convertisseur de la mesure de pile. SAR_ADC0 et 1 : ce sont les sorties console.

**Ne jamais croire une conclusion heritee sans la refaire.** Une note precedente
affirmait que `unsupport chip, please check your flash vender` etait du texte de
repli sans consequence. C'etait une boucle sans sortie, et le vrai verrou du
demarrage.

**Ne jamais desassembler a froid sans programmer la fenetre XIP.** Un decalage
de `0x11000` produit du code qui se lit sans erreur et ne veut rien dire.

**Se mefier d'un modele qui compense un bug du coeur.** Le registre `0x40022014`
avait ete pris pour un identifiant rendu octet par octet, parce qu'un `CMP.W`
faux faisait echouer la comparaison qui distingue lecture et ecriture.

**Les bugs les plus couteux etaient dans le coeur, pas dans les
peripheriques.** Un immediat mal etendu, un `ITSTATE` non vide a l'entree en
exception, et un bit de pouce laisse dans le PC par `set_reg` ont chacun coute
des jours d'enquete sur de fausses pistes. Le dernier donnait un PC impair, donc
une lecture d'instruction decalee d'un octet, donc du code qui se lit encore
mais ne veut plus rien dire ; l'ecran de code secret restait noir et les
adresses de retour tombaient a cote des instructions.

## Ce qui est etabli et modelise

**Immediats modifies.** `ThumbExpandImm` replique son octet selon quatre motifs
quand `imm12[11:10]` vaut 00, il ne le decale pas. `0xFFFFFFFF` valait
`0xFF000000`, et tout `CMP.W rX, #-1` rendait un verdict faux, ce qui cassait le
decodeur de sprites.

**Etat du bloc IT a l'entree en exception.** Il voyage dans le xPSR empile, aux
places de l'architecture, est vide pour le gestionnaire et restaure au retour.
Sans cela, une interruption tombant entre un `IT` et son instruction
conditionnelle faisait sauter la premiere instruction du gestionnaire, donc le
`PUSH` de son adresse de retour, et le coeur restait en mode Handler pour
toujours.

**Identification de la flash**, registre `0x40022018`, paire fabricant et
composant `0xC217`. Le firmware en fait `(valeur & 0xFFFF) << 8` et compare les
bits 23:16 a `0xC2` puis `0xC8`.

**Registre de configuration de la flash**, `0x40022014`. Lu avec -1 pour ne pas
ecrire, valeur attendue `0x40`. Son bit 0 est le temoin d'ecriture en cours.

**Ports d'entrees-sorties**, `0x40018000`, `0x40019000`, `0x4001A000`. Donnees,
direction, mode, autorisation et drapeaux d'interruption. Une sortie relit son
verrou, une entree rend le niveau exterieur.

**TE de l'ecran**, P1.10, 60 Hz, front montant sur l'IRQ 27.

**Controleur de transferts**, page `0x4000F000`, deux canaux, fin sur l'IRQ 58.

**Convertisseur de la mesure de pile**, page `0x4003A000`, dix bits cales au
rang 6, conversions enchainees sur l'IRQ 9.

**Boutons.** `Machine::appuyer` et `relacher`, avec les identifiants du
firmware : molette `0x08`, A `0x09`, C `0x0A`, B `0x0B`, encodeur `0x20` et
`0x21`.

## Le temps du jeu

**Le calendrier n'est pas compte en logiciel.** Le firmware lit un compteur de
secondes materiel en `0x45000304`, dans la zone systeme SN_SYS0. Sa couche date
en `0x00003754` n'est qu'un `ldr r0, [0x45000304]`, et `0x10005860` en fabrique
la date :

```text
  secondes = [0x45000304] + [0x18000BAC]   decalage pose au reglage de l'heure
  jours    = secondes / 86400              constante magique 0xC22E4507 >> 48
  jour du calendrier = [0x18000BA8] + jours
  0x100093A4 rend annee, mois, jour a partir du nombre de jours
  heure = (secondes / 3600) % 24, minute = (secondes / 60) % 60, seconde % 60
```

Le compteur est libre : le firmware ne l'ecrit jamais, il ne fait qu'ajouter son
decalage. Tant qu'il rendait zero, la date restait sur celle reglee, l'oeuf
n'eclosait pas et les jauges ne descendaient pas. Il avance maintenant d'une
seconde toutes les 96 millions de cycles, la cadence que le firmware programme
lui-meme dans son SysTick.

Structures utiles, toutes en memoire vive :

| Adresse | Contenu |
|---|---|
| `0x18001BA4` | calendrier affiche, six demi-mots : annee, mois, jour, heure, minute, seconde |
| `0x18000BB8` | le meme, dans le bloc de sauvegarde |
| `0x18000BA8` | jour de base, `0x18000BAC` decalage en secondes |
| `0x18001BF4` | scene courante, `0x18001BF8` la precedente |
| `0x18001BFB` | drapeaux de boucle, bit 4 pose en veille |
| `0x18001BFE` | compteur d'inactivite, seuil 200, cadence de trame |
| `0x18001BA0` | drapeaux d'etat, bit 2 demande une sauvegarde |
| `0x1801C2C4` | millisecondes, entretenu par le SysTick en `0x0000C0F4` |
| `0x1801C2C0` | trames, entretenu par le TE en `0x0000C120` |

## La veille

Deux veilles, et elles n'ont rien a voir.

**La veille legere**, en `0x000022B8`, pose un bit dans `0x45000300`, execute un
`WFI`, et revient. C'est celle de l'affichage de l'heure, une dizaine de
secondes apres le dernier appui.

**La veille profonde**, en `0x000023D0`, est atteinte une trentaine de secondes
apres. Elle demande la mise hors tension du coeur par le bit 0 de
`0x45000300`, execute un `WFI`, remet des bits, efface toutes les interruptions
en attente, et se rebranche sur elle meme. Le saut de `0x00002432` est
inconditionnel, et les deux seules interruptions restees autorisees, 2 et 3, ont
des gestionnaires qui reviennent dans la boucle : aucune sortie logicielle
n'existe. Juste avant d'y entrer, le firmware desactive toutes les autres
interruptions une par une, en `0x00002352`.

Le reveil vient donc du materiel et remet le coeur a zero. `Machine::appuyer`
le reproduit : un appui pendant que le PC est dans `VEILLE_PROFONDE` rallume la
console. La memoire vive est effacee par le demarrage, mais la sauvegarde est en
flash et le compteur de secondes continue de tourner, donc la partie et l'heure
reprennent. Le firmware passe alors par son ecran continuer ou effacer, puis par
la confirmation de la date, deja remplie a l'heure juste.

## La console, telle que Bandai l'a faite

Releve sur le wiki Tamagotchi, page Tamagotchi Paradise, pour que l'interface
soit fidele plutot que devinee.

**La coque** reprend celle de la Tamagotchi Pix : meme taille, meme forme
d'oeuf. Sa moitie haute porte un motif de coquille fendue, d'une autre couleur
que le corps, et s'ouvre sur un port de connexion. Le bord d'ecran porte lui
aussi un motif de coquille imprime.

**La molette** est ce qui distingue la Paradise. Elle est portee par une antenne
sur le flanc droit, et sert a zoomer. Sa fenetre transparente laisse voir deux
fleches opposees imprimees, et fait aussi office de bouton : maintenue, elle
ouvre le laboratoire ; brievement, elle vaut B dans certains menus.

**Quatre niveaux de zoom**, qui correspondent exactement aux scenes observees :

| Vue | Scene | Fonctions |
|---|---|---|
| Space | 29 | horloge, connexion, Tama Stars, deco, voyage, niveau de planete |
| Field | 30 | poser des jouets, chasse aux oeufs, nettoyer, changer de champ |
| Tama | 33 | nourrir, jouer, laver |
| Cell | 32 | statut, espece, et le QR code secret |

**Six coques** sont sorties entre juillet 2025 et juillet 2026 : Pink Land,
Blue Water et Purple Sky pour la premiere vague, Jade Forest pour la deuxieme,
Orange Tropics et White Glacier pour la troisieme. Les dumps publies portent les
noms de champs, ce qui suffit a reconnaitre l'edition : rien dans l'image ne la
nomme, la table de biomes en `0x978E8` contenant LAND, WATER et SKY dans les
cinq.

## Le son

Le moteur audio du firmware est cadence a 125 Hz par le SysTick, via
`0x10079398`, qui compte en `0x180142A0` jusqu'a la periode gardee en
`0x1800ECDA`. Son banc compte 87 sons, rendus par `0x10015818`.

La console joue bien. `jouer_son`, en `0x1001FCB4`, prend un identifiant et un
volume ; il refuse au dela de 87 sons, charge la melodie depuis la table de
pointeurs en `0x10087210 + id * 4`, et pose le drapeau `0x18014284` tant qu'elle
dure. Le premier verrou, `0x18014282`, marque le moteur initialise et vaut 1.
Il faut jouer plusieurs secondes pour declencher un son : ce n'est pas la
navigation qui sonne, ce sont les evenements du jeu.

**Le tableau des voix, en `0x1801C820`,** compte huit entrees de `0x34` octets,
indexees par le type de son a l'allocation, en `0x10022BE2`. Une voix porte
l'horloge du coeur en tete, `0x05B8D800`, soit 96 MHz, son compte de
rechargement en `+4`, un temoin d'activite en `+8` et son volume en `+0xC`. Le
tableau garde ses valeurs une fois le son fini : seul le drapeau `0x18014284`
dit ce qui sonne vraiment, et seule la voix dont `+8` n'est pas nul est en
cours.

**Le champ `+4` n'est pas une frequence, c'est une periode.** La hauteur vaut
`1500000 / valeur`. Les valeurs relevees sur des notes reelles, 4545, 1911,
1516, 1351, 955, 758 et 568, ne tombent sur la gamme temperee que prises ainsi,
a trois cents pres : Mi4, Sol5, Si5, Do#6, Sol6, Si6 et Mi7. Lues comme des
hertz elles en sont toutes a quarante deux cents, presque un quart de ton, et un
firmware ne compose pas faux de facon aussi reguliere. La base, 1 500 000, vaut
96 MHz divises par 64, un prediviseur rond sur l'horloge du coeur, ce qui est la
forme habituelle d'un timer en carre.

La hauteur etant l'inverse du compte, le contour de chaque melodie s'inversait :
une suite qui monte dans le tableau descend a l'oreille. C'est ce qui faisait
entendre toutes les melodies a l'envers, et c'est aussi pourquoi le reglage de
hauteur `/2` semblait plus juste sans l'etre.

Releve sur une note reelle, apres l'appel : 568, 1516, 955 et 758, soit Mi7,
Si5, Sol6 et Si6, sur environ cent cinquante millisecondes. Une melodie.

**Le peripherique de sortie n'est pas atteint.** Pendant un son, aucun acces
materiel ne vient du module audio, aucune page de PWM n'est touchee, et le
port 1, qui porte le buzzer sur ses broches 11 et 13, n'est jamais ecrit. On ne
le modelise donc pas : l'interface tire les hauteurs des comptes que le moteur a
calcules et les rend en signal carre, ce qu'est un buzzer. Le son entendu est
celui que la console a compose.

Corollaire pour qui cherchera ce peripherique : le registre attendu ne recoit
pas un diviseur de 96 MHz par la frequence, mais le compte lui meme, 568 ou
1516, sur une base a 1,5 MHz. C'est ce nombre la qu'il faut chercher dans les
ecritures materielles pendant une note, sans filtre de PC, et un prediviseur a
64 quelque part.

L'interface ne sonne plus pour elle meme. Le clic de bouton et le cran de
molette se superposaient aux tonalites du jeu et brouillaient le seul son qui
compte. `SoundEffect` reste dans `audio.rs` si le besoin revient, derriere un
reglage.

## La vitesse

**L'emulateur depasse la console.** Mesure du 28 aout 2026 sur Water, etat 111,
quatre passes : 134 millions de cycles par seconde sur `step`, soit 1,40 fois
la vitesse de la console, et 131 millions sur `run_frame`, soit 1,37. Une
seconde de console vaut 96 millions de cycles, ce que le firmware declare en
armant son SysTick a 95999 pour une milliseconde.

L'interface est gouvernee : elle n'accorde a la console que le temps qui lui
revient. Tant que la mesure restait sous un, ce gouverneur ne servait a rien et
les reglages au dessus de « temps reel » ne changeaient rien, la machine donnait
deja tout. Maintenant qu'elle depasse, il fait son travail et retient
l'emulation a 1,00.

Il a fallu neuf mois de mesures pour passer de 0,36 a 1,40. Ce qui a paye, dans
l'ordre :

- **Les peripheriques sont entretenus par paquets de 256 cycles** au lieu de
  chaque instruction. C'etait sept appels par pas, dont une division en soixante
  quatre bits pour le signal de trame. Deux cent cinquante six cycles valent
  moins de trois microsecondes, cent fois plus fin que la plus courte echeance
  du firmware.
- **La recherche d'interruption en attente est conditionnee a un drapeau.** Elle
  parcourait les huit mots de drapeaux avant chaque instruction, alors qu'il n'y
  a presque jamais rien a prendre.
- **L'optimisation entre modules est activee**, en une seule unite de
  generation. Le coeur passe son temps dans une poignee de fonctions appelees
  des millions de fois par seconde.
- **Les chemins rapides en 32 bits pour la memoire vive.** En 16 bits ils
  n'avaient rien donne, en 32 si.
- **Les deux demi mots d'une instruction sont recuperes en une seule resolution
  de region**, au lieu de deux traversees de la carte memoire.
- **Les instructions longues sont aiguillees sur l'octet haut**, ce qui coupe la
  chaine de tests en deux des le premier examen.
- **La table de points d'arret sort du chemin chaud.** C'est une table de
  hachage, et `run_frame` l'interrogeait a chaque instruction : un hachage
  complet par pas, plus cher que le decodage lui meme, et pour rien la quasi
  totalite du temps.
- **`reveil_materiel` est precede du test de `snsys.reveil_demande`.** Il sert
  une fois par mise en veille et etait appele des millions de fois par seconde.

Les deux derniers ont mis longtemps a se voir parce que `vitesse_probe` ne
mesurait que `step`, que l'interface n'appelle jamais. J'ai passe des mois a
optimiser en regardant la mauvaise boucle. Elle mesure les deux maintenant.

Ce qui n'a rien donne, pour ne pas le refaire : les chemins rapides en 16 bits,
la recopie de trame en bloc, `target-cpu` adapte a la machine, et remonter les
instructions les plus frequentes en tete de la chaine de decodage.

S'il fallait plus de marge un jour, pour une cible plus modeste qu'un ordinateur
de bureau, restent la table de dispatch sur les quatre bits hauts plutot que la
chaine de tests, et le cout du chemin de recuperation d'instruction dans la
fenetre XIP.

## Le tas et la sauvegarde

Le tas fait 32 Ko, pose en dur par `heap_init(0x18005D70, 0x8000)` en
`0x00009142`. L'allocateur entre en `0x10016358`, la liberation en `0x100162F0`.
Il ne tient pas de liste de blocs libres : il parcourt les blocs alloues et
cherche un trou assez grand entre deux voisins, et saute a l'assertion de
`0x1005B4AC`, qui boucle sur place, quand il n'en trouve pas.

Toute sauvegarde demande un tampon de page de 4 Ko, en `0x0000B586` pour la
verification et `0x0000B826` pour l'ecriture. La scene de jeu occupe environ
30,9 Ko du tas : reprendre un vieil instantane pris en scene de jeu avec une
sauvegarde en attente mene donc a l'assertion. Ce n'est pas le cas d'une partie
menee depuis le demarrage, ou la sauvegarde tombe a un moment ou la place existe.

## Les sauvegardes persistantes

A ne pas confondre avec les instantanes. Un instantane, `.tamastate`, fige toute
la machine pour revenir en arriere pendant la mise au point. Une sauvegarde,
`.tamasave`, ne retient que ce que la console garde vraiment, et survit a
l'extinction de l'ordinateur.

Elles vivent a cote de l'executable, dans
`sauvegardes/<empreinte du dump>/<emplacement>.tamasave`. L'empreinte mele le
nom du fichier et une somme FNV-1a de son contenu : les cinq editions ne se
melangent pas, et deux copies renommees du meme dump se retrouvent.

Le fichier porte les pages de flash ecrites par le jeu, mais aussi l'horloge :
le compteur de secondes, tous les registres de la zone systeme, et la date
d'ecriture. Au chargement, le compteur repart a sa valeur enregistree augmentee
du temps reellement passe, mesure a l'horloge de l'ordinateur. Un Tamagotchi
range vieillit ; celui ci aussi.

La zone SN_SYS0 est alimentee en permanence sur la puce et garde son contenu
coeur eteint : la sauvegarde la restitue entiere, ce qui permet au firmware de
retrouver la trace de son reveil. Ranger la console revient a l'endormir :
l'echeance d'alarme est posee a l'instant de l'ecriture, et la relance ressemble
alors a une sortie de veille. Sans cela le demarrage passait par l'ecran
continuer ou effacer puis par le reglage de l'heure ; avec, il reprend
directement, `0` puis `101` puis `28` puis `29`, a l'heure juste.

L'ecriture est automatique, espacee d'une seconde, avec une derniere passe a la
fermeture de la fenetre. `SpiFlash` porte un compteur d'ecritures pour que
l'interface sache que la partie a bouge sans comparer seize mega-octets a chaque
image.

**Piege corrige, a ne pas reintroduire :** `Instantane::restaurer` ne parcourait
que les pages salies par la machine courante. Une machine fraichement chargee
n'en a aucune, donc les pages de l'instantane etaient ignorees et la sauvegarde
perdue a chaque chargement. Il parcourt maintenant l'union des deux ensembles.

## L'interface

La coque suit la vraie console, d'apres le wiki. Corps en oeuf construit en
balayant l'angle, la largeur suivant le cosinus, ce qui resserre le sommet et
elargit la base sans casser la convexite dont depend le remplissage d'egui. La
calotte se construit de la meme facon, de part et d'autre du sommet, et non en
filtrant le contour complet : un filtre garderait les points dans l'ordre du tour
entier et la forme se croiserait.

Commandes : trois boutons A, B et C sous l'ecran, et la molette sur l'antenne
droite. Sa fenetre transparente montre deux fleches opposees qui defilent quand
on tourne et gardent leur elan ; elle est aussi le bouton de molette, comme sur
la console. A la souris, le clic gauche sur l'ecran vaut A, le droit vaut B, le
milieu vaut l'appui de molette, maintien compris dans les trois cas, sans quoi
l'appui long qui ouvre le laboratoire serait impossible. On ne compte pas
l'appui de molette pendant qu'on la tourne, sinon chaque cran vaudrait une
pression.

Six coques, celles que Bandai a sorties : Pink Land, Blue Water et Purple Sky de
la premiere vague, Jade Forest de la deuxieme, Orange Tropics et White Glacier de
la troisieme. La coque suit l'edition du dump, reconnue au nom du fichier, et
reste choisissable a la main. Rien dans l'image ne nomme l'edition.

Le son de l'interface repond a chaque appui et a chaque cran de molette, avec
une coupure, un volume, et un reglage de hauteur pour le buzzer de la console.

## La chaine de la mesure de pile

Elle sert de modele pour les prochaines enquetes :

```text
  0x10003754  compare la tension au seuil 0x23332
  0x10003830  la lit dans le mot 0x18005C68
  0x1000397E  l'y ecrit, a partir des echantillons accumules
  0x10003924  n'accumule que si le gestionnaire a pose son drapeau
  0x10078774  gestionnaire de l'IRQ 9, extrait par UBFX #6, #10
```

Sans tension, le firmware pose le bit 3 du premier octet de l'etat sauvegarde,
imprime `** LOW BATTERY FLAG DETECTED **` et s'eteint. Les dumps Earth et Land
portent deja ce drapeau ; `Machine::remplacer_la_pile` l'efface et refait la
somme de controle des deux pages.

## Ce qui reste a faire

**1. Le peripherique de sortie du son.** Le moteur du firmware calcule ses notes
mais n'ecrit sur aucun peripherique. Mesure faite avec `son_probe`, filtre par
intervalle de PC : aucun acces materiel ne vient du module audio pendant qu'une
note joue, aucune page de PWM n'est touchee, et le port 1, qui porte le buzzer
sur ses broches 11 et 13, n'est jamais ecrit. En attendant de trouver par ou il
sort, l'interface lit les frequences deja calculees dans les voix et les rend en
signal carre. Le son entendu est donc bien celui que la console compose, mais on
ne sait toujours pas comment elle le fait sonner.

**2. Les quatre autres editions.** Seule Water a ete menee jusqu'au bout ; Jade
affiche son oeuf. Earth, Sky et Land n'ont pas ete suivies jusqu'a l'image.

**3. La molette en quadrature.** Elle est modelisee en appuis simples sur les
broches `0x20` et `0x21`, pas en signal a deux phases decalees.

## Blocages connus, et comment les reconnaitre

**Reprendre un vieil instantane pris en scene de jeu fige la console.** La scene
occupe 30,9 Ko d'un tas de 32 Ko ; la premiere sauvegarde declenchee par l'avance
de l'horloge demande 4 Ko, ne les trouve pas, et le firmware saute a son
assertion en `0x1005B4AC`, qui boucle sur place. On le reconnait tout de suite :
`temps_probe` montre 97 % du temps passe en `0x1005B4C4`. Ce n'est pas un defaut
du modele mais un instantane pris dans un etat que la console n'atteint pas
d'elle meme, capture quand l'horloge etait encore figee. Une partie menee depuis
le demarrage n'y va pas.

**Le tableau des voix audio reste rempli au silence.** Le lire sans verifier le
drapeau `0x18014284` et le temoin `+8` de la voix donne un bourdonnement
permanent.

**Le son part a l'envers si on l'echantillonne a la cadence de l'interface.** Une
melodie dure cent cinquante millisecondes et change plusieurs fois de note : un
releve par image n'en attrape que des morceaux, dans un ordre arbitraire. Il faut
la suivre dans la boucle d'emulation, ce que fait `TamagotchiApp`, et rendre la
suite d'un bloc.

**Les reglages de vitesse au dessus de « temps reel » ne font rien** tant que la
mesure reste sous un : la machine donne deja tout. Le diagnostic affiche la
vitesse atteinte, c'est le chiffre a regarder avant de conclure.

## Ce qu'il ne faut pas refaire

- Ne pas retoucher au sens du bit de direction du DMA de flash : pose, il va de
  la flash vers la memoire.
- Ne pas chercher un identifiant JEDEC de trois octets en `0x40022018`.
- Ne pas supposer que le SysTick cadence la veille profonde : le firmware le
  desactive explicitement avant de dormir, et la boucle n'a aucune sortie
  logicielle.
- Ne pas conclure d'un desassemblage a froid sans le recouper avec la trace
  d'execution de `watch_probe`.
- Ne pas conclure que la console est muette sur une fenetre de mesure courte :
  elle ne joue que sur des evenements de jeu, jamais sur la navigation, et il
  faut jouer plusieurs secondes avant qu'une note sorte. Une premiere mesure sur
  quatre cents millions de pas avait conclu a tort au silence.
- Ne pas chercher a accelerer par les chemins rapides de la memoire vive en 16
  bits, la recopie de trame en bloc, ou `target-cpu` adapte a la machine : les
  trois ont ete essayes et mesures, aucun ne donne rien. Le 32 bits, lui, paye.
- Ne pas lire le champ `+4` d'une voix comme une frequence : c'est une periode,
  la hauteur vaut `1500000 / valeur`. La conclusion inverse avait ete tiree a
  l'oreille et tenue pour acquise ; elle etait fausse, et c'est elle qui faisait
  jouer toutes les melodies a l'envers. La gamme temperee tranche en une
  commande, sans oreille : lues en periodes les valeurs tombent a trois cents,
  lues en hertz a quarante deux.
