# Reprise : ce qu'il faut savoir pour continuer

Ce fichier est le point d'entree. Le detail est dans `ou-on-en-est.md`, le
materiel dans `materiel-snc7340.md`.

## Le projet en une phrase

Emulateur du Tamagotchi Paradise, console de 2025 a SoC Sonix SNC7340, ecrit en
Rust avec une interface egui. Le vrai firmware demarre et le jeu se joue.

## Ce qui marche

Mise en route complete, boutons, molette de zoom, horloge qui avance, oeuf qui
eclot, nourrissage, QR code secret, veille profonde et reveil, son de la console,
sauvegardes persistantes qui survivent a l'extinction de l'ordinateur et
vieillissent en temps reel, coque fidele suivant l'edition chargee.

73 tests passent. La branche est `feat/real-firmware-boot`.

## La vitesse

**L'emulation depasse la console depuis le 28 aout 2026 : 1,40 fois le temps
reel sur `step`, 1,37 sur `run_frame`.** Le regulateur de l'interface la retient
maintenant a 1,00. Il n'y a plus de defaut visible a l'ecran.

Mesurer avec :

```bash
cargo run --release --example vitesse_probe -- <dump.bin> <CLE> <etat.tamastate> 5
```

La sonde donne les deux boucles. `step` mesure le coeur seul, `run_frame` est ce
que l'interface appelle vraiment. Longtemps elle ne montrait que `step`, ce qui
cachait les deux derniers freins.

Le chemin parcouru, de 0,36 a 1,40 :

- entretenir les peripheriques par paquets de 256 cycles au lieu de chaque
  instruction ;
- conditionner la recherche d'interruption en attente a un drapeau ;
- l'optimisation entre modules en une seule unite de generation ;
- les chemins rapides en 32 bits pour la memoire vive ;
- recuperer les deux demi mots d'une instruction en une seule resolution de
  region ;
- aiguiller les instructions longues sur l'octet haut ;
- sortir la table de points d'arret du chemin chaud. C'est une table de hachage,
  et `run_frame` l'interrogeait a chaque instruction : un hachage complet par
  pas, plus cher que le decodage lui meme ;
- tester `snsys.reveil_demande` avant d'appeler `reveil_materiel`, qui sert une
  fois par mise en veille et etait appele a chaque instruction.

Ce qui n'a rien donne, mesure, a ne pas refaire : les chemins rapides en 16 bits,
la recopie de trame en bloc, `target-cpu` adapte a la machine, et remonter les
instructions les plus frequentes en tete de la chaine de decodage.

Si un jour il faut plus de marge, restent la table de dispatch sur les quatre
bits hauts plutot que la chaine de tests, et le cout du chemin de recuperation
d'instruction dans la fenetre XIP.

## Les six trouvailles qui ont debloque le reste

1. **Le temps du jeu ne se compte pas en logiciel.** Il se lit sur un compteur de
   secondes materiel en `0x45000304`, que le modele rendait constant. Tant qu'il
   ne bougeait pas, l'oeuf n'eclosait pas et les jauges ne descendaient pas.
2. **La veille profonde n'a aucune sortie logicielle.** Le firmware programme une
   echeance en `0x45000230`, pose un temoin d'armement, bit 8 de `0x45000234`,
   puis boucle sur `WFI`. Le materiel doit effacer ce temoin en sonnant et poser
   les bits 9 et 11 ; sans cela le reveil passe pour une pile neuve et redemande
   l'heure.
3. **`Registers::set_reg` laissait le bit de pouce dans R15.** PC impair, lecture
   d'instruction decalee d'un octet, execution dans du code qui se lit encore
   mais ne veut plus rien dire. L'ecran de code secret restait noir.
4. **`Instantane::restaurer` ne reecrivait que les pages de flash deja salies.**
   Une machine fraichement chargee n'en a aucune : la sauvegarde etait perdue a
   chaque chargement et le firmware repartait sur sa premiere mise en route.
5. **Le son ne sort que sur des evenements de jeu**, jamais sur la navigation.
   Une premiere mesure sur quatre cents millions de pas avait conclu a tort au
   silence.
6. **Le champ de hauteur d'une voix est une periode, pas une frequence.** La
   hauteur vaut `1500000 / valeur`. Comme elle est l'inverse du compte, le
   contour de chaque melodie s'inversait, et tous les sons s'entendaient a
   l'envers. La gamme temperee tranche sans oreille : lues en periodes les
   valeurs relevees tombent a trois cents des notes justes, lues en hertz a
   quarante deux, presque un quart de ton.

## Les pieges qui coutent une journee

- **Ne jamais trancher une question de hauteur a l'oreille seule.** La
  conclusion « le champ donne des hertz » avait ete tiree ainsi et tenue pour
  acquise pendant tout un jalon. Une comparaison a la gamme temperee la
  renversait en une commande.
- **Ne jamais reprendre un vieil instantane pris en scene de jeu.** La scene
  occupe 30,9 Ko d'un tas de 32 Ko, la sauvegarde en demande 4, et le firmware
  boucle sur son assertion en `0x1005B4AC`. `temps_probe` le montre en une
  commande : 97 % du temps en `0x1005B4C4`.
- **Ne jamais desassembler a froid sans programmer la fenetre XIP.** Un decalage
  de `0x11000` produit du code qui se lit sans erreur et ne veut rien dire.
- **Ne jamais croire une etiquette du datasheet.** GPIO2 est le controleur XIP,
  UART0 un accelerateur de somme de controle, le chien de garde un convertisseur
  de mesure de pile.
- **Ne jamais croire une conclusion heritee sans la refaire.** Deux notes de
  passation se sont averees fausses, dont celle qui disait la console muette.

## Outils

Vingt-deux sondes dans `examples/`, toutes en `<dump.bin> <cle hex> [...]`. Les plus
rentables : `scene_probe` avec `RESET`, `SORTIE_ETAT` et `TRACE_PAS`,
`temps_probe` pour trouver une boucle morte, `watch_probe` pour arreter sur une
adresse ou une modification de memoire, `vitesse_probe` pour la vitesse,
`son_probe` pour le son, `partie_probe` pour les sauvegardes.

## La cle, et pourquoi le depot est publiable

L'historique a ete reecrit le 28 aout 2026. Ni la cle ni les chemins personnels
n'apparaissent dans un seul commit. Ne pas defaire ce travail : un `git add -A`
suffit a tout remettre, et il a fallu trois passes pour en sortir.

Trois pieges m'ont coute la journee.

1. **`git log --all -S` ment juste apres une reecriture.** `filter-branch` range
   les anciennes references sous `refs/original/`, et `--all` les inclut. Il faut
   les supprimer, faire expirer le reflog et passer `git gc --prune=now` avant que
   la mesure veuille dire quelque chose.
2. **Un script de purge qui filtre sur une liste d'extensions rate ce qui n'y est
   pas.** `mesurer.ps1` portait la cle et deux chemins absolus ; `.ps1` n'etait pas
   dans la liste, le fichier n'a jamais ete ouvert, et la purge s'est declaree
   satisfaite. Balayer tous les fichiers et laisser `grep -I` ecarter le binaire.
3. **`s|C:\\Users\\...|` ne matche rien dans ce sed.** La forme qui marche est
   `s|C:[\\/]Users[\\/]...|` avec `-E`. Une substitution qui ne matche pas n'est pas
   une erreur : sed ne dit rien et sort en code 0. Les chemins ont donc survecu a
   deux purges completes qui se declaraient reussies.

Je ne crois plus une purge sur ce qu'elle affiche. Je rebalaie tous les blobs
atteignables, avec un motif que je reecris a la main. La premiere fois j'avais
copie le motif depuis le script de purge : les deux avaient le meme bug et se
confirmaient l'un l'autre.

Le controle, a repasser apres toute manipulation d'historique :

```bash
git for-each-ref --format='%(refname)' refs/original | xargs -n 1 git update-ref -d
git reflog expire --expire=now --all && git gc --prune=now
git rev-list --objects --all | git cat-file --batch-check='%(objecttype) %(objectname)' \
  | awk '$1=="blob"{print $2}' \
  | while read o; do git cat-file blob $o | grep -qI "<motif>" && echo "FUITE $o"; done
```

## Materiel de travail

Rien de tout cela n'est dans le depot, et rien n'y entrera. Trois variables
designent le materiel :

```bash
export SONIX_DEVICE_KEY=0x........   # la cle, commune aux cinq editions
export SONIX_DUMPS=<dossier des .bin>
export SONIX_ETAT=<chemin d'un .tamastate>
```

Sur le poste de travail, leurs valeurs sont dans `cle-device.txt` a la racine,
non suivi par git.

La cle est gravee dans les fusibles de la puce, ne figure dans aucun dump, et se
lit en SWD sur l'appareil. Elle est identique sur les cinq editions : c'est une
constante de modele, pas un secret par exemplaire.

Sans ces variables, les tests qui dependent d'un dump se sautent et la suite
reste verte. **Verifier les deux cas apres toute modification de ce mecanisme**,
sinon on ne sait pas si les tests passent ou s'ils ne font plus rien. Reference :
73 tests en 0,19 s sans les variables, les memes 73 en 2,25 s avec.
