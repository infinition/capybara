# Tamagotchi Paradise, matériel et démarrage

Tout ce qui a été établi sur la plateforme, et sur quoi l'émulateur s'appuie.
Chaque affirmation ici est vérifiée sur un dump réel ou par un test de la suite.

## Plateforme

SoC Sonix **SNC7340** (famille SNC733x), cœur ARM Cortex-M, deux cœurs.
Flash externe SPI NOR Macronix MX25L12835F, 16 Mo.
Horloge principale relevée dans le firmware : 12 MHz (`0x00B71B00`).

## Clé de chiffrement

```
deviceKey = <CLE>
```

Elle déchiffre **les cinq éditions** : Earth, Water, Land, Sky, Jade Forest.
Ce n'est donc pas une clé par unité mais une clé de gamme.

Sur la puce elle se lit dans `SN_SYS0->FEUSE3` (`0x4500003C`), ou sur les 16 bits
de poids fort de `FEUSE2` (`0x45000038`). Elle est absente du dump de flash.

Elle a été retrouvée hors ligne, sans accès SWD, en exploitant deux faiblesses :

1. Le champ clé AES de la load table (offset `0x28`) est **entièrement nul**, donc
   la clé et l'IV dérivent de la seule `deviceKey` 32 bits.
2. Le CBC inverse **réinitialise l'IV toutes les `0x1000` octets**. Pour tout bloc
   qui n'est pas le premier de sa tranche, l'IV effectif est le bloc de chiffré
   précédent : 99,6 % de l'image se déchiffre donc sans aucune clé.

Il ne restait que 16 blocs et 2^32 candidats, testés contre la table de vecteurs
Cortex-M. Recherche exhaustive en une douzaine de secondes.
Sortie validée identique au décrypteur C# de référence de GMMan.

## Disposition de la flash

| Offset | Taille | Contenu |
|---|---|---|
| `0x000000` | 4 Ko | Load table `SONIXDEV`, version V3 (`0x5A5A0033` à `0x1F8`) |
| `0x001000` | 64 Ko | Code utilisateur, **chiffré**, recopié en PRAM à l'adresse 0 |
| `0x011000` | ~570 Ko | Code XIP, en clair |
| `0x110000` | 4 Ko | Blob DPD, table de vecteurs propre |
| `0x111000` | ~7,4 Mo | Conteneur `ARC2`, trois sous-blobs non compressés |
| `0xD49000` | | Sauvegarde et état persistant |

## Carte mémoire

Datasheet V1.7 section 4, recoupée avec le comportement du firmware.

| Région | Adresse | Taille |
|---|---|---|
| Program RAM | `0x00000000` | 64 Ko |
| ROM cœur 0 | `0x08000000` | 64 Ko |
| Fenêtre I-cache | `0x10000000` | 1 Mo, **base programmable** |
| SRAM AHB | `0x18000000` | 128 Ko |
| Mailbox RAM | `0x20000000` | 4 Ko |
| Flash SPI NOR | `0x60000000` | fenêtre 256 Mo |

Périphériques : PMU `0x40001000`, ISO `0x40002000`, RTC `0x40003000`,
SysCtrl0 `0x40004000`, SysCtrl1 `0x40005000`, USB `0x40007000`,
SAR ADC `0x4000A000`, I2S `0x4000E000` / `0x40012000` / `0x40019000`,
SPI1 `0x40020000`, IDMA `0x40025000` / `0x4002B000`,
GPIO `0x4002F000` / `0x40030000` / `0x40031000`, I2C1 `0x40033000`,
UART `0x40034000` / `0x40038000`, WDT `0x4003A000`,
timers CT32B `0x40040000`, zone système SN_SYS0 `0x45000000`.

### Correction sur `0x4002F000`

La figure 4-1 place GPIO2 à cette page. Le firmware y fait tout autre chose :
il écrit une adresse flash dans `0x4002F004` puis `3` dans `0x4002F000`. C'est
le **contrôleur de la fenêtre XIP**, et sa base est programmable.

C'est le point qui bloquait l'émulation. Un saut vers `0x1006D1C4` ne vise pas
l'offset flash `0x6D1C4` mais `0x11000 + 0x6D1C4 = 0x7E1C4`, où se trouve bien un
prologue `PUSH {r7, lr}`. Avec la base à `0`, on atterrissait au milieu d'une
fonction et la pile partait en vrille au premier `POP {r7, pc}`.

## Séquence de démarrage

1. Le bootrom lit la `deviceKey` dans les fusibles, déchiffre les 64 Ko de
   `0x1000`, les recopie en PRAM à l'adresse 0, puis saute sur le vecteur de
   reset qui s'y trouve (`0x000002F5` sur toutes les éditions).
2. Le firmware lit `VTOR` (`0xE000ED08`), charge `SP` depuis la table de vecteurs.
3. Il relit le **bloc boot-info** que le bootrom a laissé : le pointeur est en
   `0x20000F60`, et le champ `+0x818` porte la base de la région XIP,
   soit `0x60011000`.
4. Il écrit cette base dans `0x4002F004`, active la fenêtre par `3` dans
   `0x4002F000`, puis **vérifie** que `[0x10000000]` vaut bien `[0x60011000]`.
   En cas d'écart il appelle `panic(5)`.
5. Init des horloges via SN_SYS0, du watchdog, du SysTick (`0xE000E014`), des
   priorités NVIC, puis bascule en XIP.

Le gestionnaire de panique est en `0xB924` : `CMP r0,#7` puis un `TBB` vers huit
boucles mortes distinctes, une par code d'erreur. Une console figée sur l'une
d'elles n'est pas endormie, elle a échoué son autodiagnostic.

Le bootrom n'étant pas émulé, le chargeur reconstruit le bloc boot-info
(voir `install_boot_info` dans `src/emulator/loader.rs`).

## Console de debug du firmware

Le firmware embarque le code d'exemple du SDK Sonix et son `printf`. Dans la
boucle de formatage, l'instruction en `0x1070` appelle la fonction de sortie avec
le caractère dans `r0`. L'intercepter donne la console complète sans avoir à
modéliser le DMA de l'UART :

```bash
cargo run --release --example boot_probe -- <dump.bin> <CLE> 300000000
```

Ce message a longtemps masqué le vrai problème :

```
[example]flash id:
unsupport chip, please check your flash vender
```

C'est une **chaîne de repli du SDK**, sans rapport avec le fabricant de la flash.
Elle s'affiche quand la fonction `0x0B574` rejette les deux emplacements de
sauvegarde. Cette fonction lit une page de 4 Ko, vérifie que ses deux premiers
octets sont le complément des deux suivants, puis compare la somme qu'ils portent
à celle calculée sur les 4092 octets restants. Depuis que l'accélérateur de somme
est modélisé, le firmware ne se plaint plus.

## Périphériques établis par la trace

| Base | Rôle | État |
|---|---|---|
| `0x4000A000` / `0x4000B000` | Convertisseurs SAR. Canal en `+0x00`, fin de conversion au bit 6 de `+0x14` | modélisé |
| `0x40022000` | Contrôleur de flash externe. `+0x100` adresse mémoire, `+0x104` longueur, `+0x108` contrôle, `+0x10C` adresse flash | DMA modélisé, identifiant JEDEC non résolu |
| `0x4002F000` | Contrôleur de la fenêtre XIP | modélisé |
| `0x40038000` | Accélérateur de somme de contrôle. Polynôme en `+0x18`, source `+0x04`, longueur `+0x08`, départ bit 4 de `+0x00`, résultat `+0x1C`. CRC-16/ARC, polynôme réfléchi `0xA001`, init 0 | modélisé |
| `0x45000000` | SN_SYS0, horloges et PLL | partiellement modélisé |
| `0x4001A000`, `0x40018000` | scrutés en statut, rôle non établi | **verrou actuel** |

La région bit-band du Cortex-M (`0x22000000` et `0x42000000`) est implémentée :
le firmware scrute un statut par l'alias `0x42340000`, qui vise `0x4001A000` bit 0.

## État de l'émulation

Les quatre éditions distinctes exécutent **97 % de leur temps en code applicatif
XIP**, console de debug vide, aucun message d'erreur. Elles valident leur
sauvegarde, la réécrivent, et tournent.

### Le contrôleur de flash, sémantique établie

Le registre `0x40022108` porte deux bits distincts : le **bit 0 est le départ**,
le **bit 1 la direction**. Zéro pour aller de la flash vers la mémoire, posé pour
remonter vers la flash. Le sens se déduit de l'ordre des transferts, le tout
premier étant la lecture d'une page de sauvegarde pour la valider.

Le firmware procède par lecture-modification-écriture sur ce registre : il doit
donc s'y relire tel qu'il a écrit, sinon le bit de direction se perd entre les
deux étapes et toute écriture passe pour une lecture.

### La sauvegarde

`0x0B814` écrit une sauvegarde : elle alloue 4 Ko, recopie l'état vivant depuis
`0x18000BA0`, pose la somme et son complément en tête, efface le secteur,
l'écrit, puis vérifie. Pour le slot 2 cette vérification ne relit pas la flash,
elle recalcule la somme sur l'état vivant en SRAM.

### Verrou actuel

Les trois éditions Earth, Water et Sky s'arrêtent sur une assertion en
`0x1005B4AC`, appelée depuis `0x0B8FA`, où la somme recalculée sur l'état vivant
diffère de celle que l'appelant avait fournie. Jade Forest s'arrête ailleurs,
en `0x1005E91E`.

Comme la vérification porte sur la SRAM et non sur la flash, l'écart signifie que
l'état a changé entre les deux calculs. Le firmware réactive les interruptions
juste avant, par un `CPSIE` en `0x0B8CC` : la piste la plus probable est un
gestionnaire qui s'intercale et modifie l'état, faute d'une fidélité temporelle
suffisante entre nos transferts instantanés et le cadencement du SysTick.

C'est donc un problème de justesse d'émulation, pas un périphérique manquant.

## Utilisation

La clé se fournit de trois façons, dans cet ordre de priorité : le champ
`device_key` de `Machine`, la variable d'environnement `SONIX_DEVICE_KEY`, ou un
fichier `<dump>.key` posé à côté du dump.

```bash
cargo run --release --example boot_probe -- <dump.bin> <CLE> 200000000
```

`boot_probe` rend compte du chargement, des zones parcourues, des adresses les
plus exécutées et de tous les registres touchés. `spin_probe` sert à isoler une
boucle morte et à afficher son contexte.

## Brochage de la console

Releve par tama-para-research, `hardware/testpads.txt`, et recoupe par la trace
du firmware.

| Broche | Role | Broche | Role |
|---|---|---|---|
| P0.0 | UART0 TXD | P1.0 a P1.4 | flash SPI : CLK, MISO, MOSI, D2, D3 |
| P0.1 | UART0 RXD | P1.5 | CS de l'ecran |
| P0.2 | UART1 TXD | P1.6 | SCLK de l'ecran |
| P0.3 | UART1 RXD | P1.8 | MOSI de l'ecran |
| P0.4 | RESET de l'ecran | P1.9 | alimentation de l'ecran |
| P0.5 | commande ou donnee, ecran | P1.10 | TE de l'ecran |
| P0.6 | mesure de la batterie | P1.11 | buzzer, voie 0 |
| P0.7 | retroeclairage | P1.13 | buzzer, voie 1 |
| P0.8 | bouton molette | P2.0 | encodeur, voie 1 |
| P0.9 | bouton A | P2.1 | encodeur, voie 2 |
| P0.10 | bouton C | P0.15 | CS de la flash |
| P0.11 | bouton B | P0.12 a P0.14 | SWO, SWCLK, SWDIO |

L'ecran fait 128 x 128 pixels en RGB565. Son tampon est en `0x180142A6` et part
vers `0x4000E01C` par le canal `0x4000F100`, 16384 unites par trame.

## Cartographie de la flash

Puce Macronix de 16 Mo.

```text
  0x000000 - 0x000fff   en-tete du firmware
  0x001000 - 0x010fff   firmware PRAM, chiffre
  0x011000 - 0x10ffff   firmware XIP
  0x110000 - 0x110fff   firmware DPD
  0x111000 - 0x8286c3   ressources
  0x8286c4 - 0xd48fff   inutilise
  0xd49000 - 0xde9fff   informations de version
  0xd4a000 - 0xd4dfff   preparation des objets telecharges
  0xd4e000 - 0xdedfff   objets telecharges
  0xdee000 - 0xe45fff   donnees de fantome
  0xe46000 - 0xe65fff   reception de fantome ou de correctif
  0xe66000 - 0xe85fff   export de fantome
  0xe86000 - 0xefdfff   captures d'amis
  0xefe000 - 0xefefff   sauvegarde principale
  0xeff000 - 0xefffff   copie de sauvegarde
  0xf00000 - 0xffffff   reserve
```

Les trois pages lues au demarrage pour validation sont `0xd49000`, `0xefe000` et
`0xeff000`, ce qui recoupe exactement cette cartographie.

## Table des scenes du jeu

En `0x1007C1F0` et suivants, des enregistrements de sept mots :
actif, nom, identifiant, puis trois fonctions. Les noms sont lisibles et donnent
la structure du jeu : `PSID_HOME`, `PSID_HOME_TAMA`, `PSID_OSEWA_GOHAN`,
`PSID_OSEWA_OTEIRE`, `PSID_VIEWER_CHARA`, `PSID_DEVELOP_TESTQR`, et d'autres.
C'est le point d'entree naturel pour comprendre la logique de jeu une fois
l'affichage obtenu.

## Vecteurs d'interruption utiles

| Vecteur | Adresse | Gestionnaire | Role |
|---|---|---|---|
| SysTick | `0x0000003C` | `0x0000C0F4` | base de temps |
| IRQ 2 | `0x00000048` | `0x00003788` | systeme, `0x45000300` |
| IRQ 9 | `0x00000064` | `0x10078774` | chien de garde |
| IRQ 27 | `0x000000AC` | `0x0000C120` | port 1, front du TE |
| IRQ 58 | `0x00000128` | `0x10014050` | fin de transfert vers l'ecran |
