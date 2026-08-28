# Passation : etat exact, et ou ca en est

Document de reprise. Il dit ce qui est etabli, ce qui reste a faire, et par ou
attaquer. Le detail materiel est dans `materiel-snc7340.md`.

## Resume

**Le jeu demarre et repond aux boutons.** Le firmware Water passe son
identification de flash, sa mesure de pile, initialise son ecran, decode ses
ressources, affiche son ecran titre, puis son menu de choix de langue. Le bouton
A fait defiler la selection, le bouton molette valide et fait passer a l'ecran de
reglage de la date.

L'ecran fait 128 x 128 en RGB565. Pres de sept cents trames sont poussees vers
l'afficheur sur quatre milliards de pas.

La cle de dechiffrement est `<CLE>`, commune aux cinq editions.

## Outils

Sept sondes dans `examples/`. Toutes prennent `<dump.bin> <cle hex>`.

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

1. **Le son.** Le buzzer est sur P1.11 et P1.13 en PWM. Rien n'est modelise.
   Le moteur audio est cadence a 125 Hz par le SysTick, via `0x10079398` qui
   compte en `0x180142A0` jusqu'a la periode gardee en `0x1800ECDA`.
2. **La confirmation de la date apres un reveil.** Le firmware la demande a
   chaque rallumage, alors que le compteur de secondes n'a pas ete perdu. Il
   lit pourtant son drapeau de validite en `0x00003760`, qui rend bien vrai, et
   le compteur en `0x00003754`. Reste a trouver ce qui lui fait croire que
   l'heure est a refaire.
3. **Une execution qui derive de deux octets.** Les instantanes pris a un
   plantage montrent une adresse de retour a un octet impair de l'instruction
   reelle, par exemple `0x1006DF3E` la ou le mot commence en `0x1006DF3C`. Le
   code s'y lit encore, se recale sur les `b .+2` de remplissage, et finit par
   ecrire des zeros dans la table des vecteurs. L'origine du decalage n'est pas
   trouvee.
4. **Les quatre autres editions.** Seule Water a ete menee jusqu'au bout ; Jade
   affiche son oeuf, les trois autres n'ont pas ete suivies jusqu'a l'image.

## Ce qu'il ne faut pas refaire

- Ne pas retoucher au sens du bit de direction du DMA de flash : pose, il va de
  la flash vers la memoire.
- Ne pas chercher un identifiant JEDEC de trois octets en `0x40022018`.
- Ne pas supposer que le SysTick cadence la veille : le firmware le desactive
  explicitement avant de dormir.
- Ne pas conclure d'un desassemblage a froid sans le recouper avec la trace
  d'execution de `watch_probe`.
