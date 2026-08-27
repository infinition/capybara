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

Les cinq éditions démarrent, exécutent des centaines de millions d'instructions
sans encodage inconnu, passent en code applicatif XIP et pilotent le watchdog,
SN_SYS0, le contrôleur XIP, les convertisseurs et le contrôleur de flash.

Le firmware termine désormais son démarrage. Les cinq éditions valident leur
sauvegarde et n'émettent plus aucun message d'erreur.

Deux comportements distincts au terme du boot :

**Earth et Land** (fichiers identiques, même empreinte) entrent en veille : bits
de veille posés dans `0x45000300`, instruction `WFI`, effacement des interruptions
en attente, boucle. SysTick désactivé, `PRIMASK` à 1, seule l'IRQ externe 3 armée,
le RTC ayant été configuré juste avant. La source de réveil reste à modéliser.

**Water, Sky et Jade Forest** vont plus loin : SysTick actif (`CSR = 0x00010007`,
donc horloge cœur, interruption armée, compteur en marche) et deux IRQ armées.
Elles butent sur un bit de `0x4001A000`, lu par la fenêtre bit-band et attendu
à 1. C'est le prochain verrou.

L'ADC a été instrumenté pour rendre une valeur de pile pleine, sans effet observé :
le registre de résultat n'a pas été localisé, le firmware ne lisant jamais d'autre
offset de cette page. L'hypothèse d'une coupure sur pile faible n'est donc **pas**
vérifiée.

Registres encore non modélisés, relevés par la trace MMIO :

| Adresse | Observation |
|---|---|
| `0x40022000` | 5 lectures, 5 écritures, dernière `0x8000` |
| `0x40008000`, `0x40009000` | écritures à motif `0x5AFA____`, clés de déverrouillage |
| `0x45000108` .. `0x45000110` | zone système, rôle non établi |

Aucun contrôleur LCD n'apparaît dans la figure 4-1. L'écran est donc piloté
autrement, vraisemblablement via SPI1 et l'IDMA. C'est la piste à suivre pour
obtenir une image.

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
