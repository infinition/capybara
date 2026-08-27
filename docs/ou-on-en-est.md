# Passation : etat exact, et ou ca bloque

Document de reprise. Il dit ce qui est etabli, ce qui est incertain, et ce que
j'aurais fait ensuite. Le detail materiel est dans `materiel-snc7340.md`.

## Resume

Les cinq firmwares demarrent, valident leur sauvegarde, passent l'identification
de la flash, initialisent leur ecran et entrent dans leur code de rendu. La cle
de dechiffrement est `<CLE>`, commune aux cinq editions.

L'ecran est etabli : **128 x 128 pixels en RGB565**, tampon en `0x180142A6`,
pousse par un canal de transfert vers le registre `0x4000E01C`. Le firmware y
ecrit deja quelques pixels. Il n'y a pas encore d'image complete : la routine de
remplissage `0x00007040` s'emballe et sort de la memoire vive.

## Outils

Six sondes dans `examples/`. Toutes prennent `<dump.bin> <cle hex>`.

- **`boot_probe`** : le couteau suisse. Zones parcourues, adresses les plus
  executees, desassemblage de la boucle chaude, tous les registres touches avec
  le PC qui les touche, transferts de flash, etat des interruptions, et la
  console de debug du firmware.
- **`dis_probe`** : desassemble a froid un intervalle. Programme la fenetre XIP
  avant de lire, sans quoi tout le code au dela de `0x10000000` se lit decale de
  `0x11000`, ce qui donne un desassemblage plausible mais faux.
- **`spin_probe`** : isole une boucle morte et affiche ses 60 derniers pas.
- **`watch_probe`** : s'arrete a la Nieme visite d'une adresse ou a la Nieme
  modification d'un mot, et rend registres, pile d'appels reelle, trace des pas
  executes avant et apres l'arret.
- **`ecran_probe`** : rend le tampon d'image en PPM, en lisant source et
  longueur dans le canal plutot qu'en les supposant.
- **`race_probe`** : verifie si l'etat vivant change entre deux points.

Variables d'environnement :

| Variable | Effet |
|---|---|
| `MMIO_PAGE=0x...` | journalise dans l'ordre les acces a une page de 4 Ko |
| `MMIO_ECR=1` | n'y journalise que les ecritures |
| `MMIO_FORCE=adr:val,...` | impose une valeur de lecture sur un registre non modelise |
| `WATCH_COND=r6:0x...` | ne declenche `watch_probe` que si un registre vaut une valeur |
| `WATCH_MEM=0x...` | arrete `watch_probe` a la modification d'un mot de SRAM |
| `TRACE_PAS=N` | rend les N derniers pas executes avant l'arret |
| `TRACE_APRES=N` | rend les N pas suivant l'arret |
| `MEM_DUMP=0x...` | vidange 64 octets a une adresse |
| `MEM_CMP=a:b:long` | compare deux zones et signale le premier ecart |
| `XIP_BASE=0x...` | base de la fenetre XIP pour `dis_probe` |
| `ECRAN_DEPART=n` | arrete `ecran_probe` au nieme transfert vers l'afficheur |
| `SONIX_FLASH_ID=0x...` | impose la paire d'identification de la flash |

## Trois lecons payees cher

**Ne jamais croire une etiquette sur parole.** Le datasheet place GPIO2 en
`0x4002F000` : c'est le controleur XIP. Il place UART0 en `0x40038000` : c'est un
accelerateur de somme de controle. Il place I2S0 en `0x40019000` : c'est le port
d'entrees-sorties numero 1.

**Ne jamais croire une conclusion heritee sans la refaire.** La note precedente
affirmait que `unsupport chip, please check your flash vender` etait du texte de
repli imprime sans consequence. C'etait faux : c'est une boucle sans sortie en
`0x1006A018`, et c'etait le vrai verrou du demarrage.

**Ne jamais desassembler a froid sans programmer la fenetre XIP.** Un decalage
de `0x11000` produit du code qui se lit sans erreur et ne veut rien dire.

## Ce qui est etabli et modelise

**Identification de la flash.** Le firmware pose le bit 15 de `0x40022004`,
attend qu'il retombe ainsi que le bit 1, puis lit `0x40022018` d'un bloc en
`0x000039E8`. Il en fait `(valeur & 0xFFFF) << 8` et compare les bits 23:16 a
`0xC2` (Macronix) puis `0xC8` (GigaDevice). Le registre porte donc la paire
fabricant et composant, `0xC217` pour la puce montee sur la console. La
transformation a ete etablie en imposant `0x01020304`, qui rend `0x00030400`.
La console affiche alors `[example]flash id:c21700`.

**Ports d'entrees-sorties**, en `0x40018000`, `0x40019000` et `0x4001A000`.
Donnees en `0x00`, direction en `0x04`, mode en `0x08`, autorisation
d'interruption en `0x18`, drapeaux en `0x1C`, effacement en `0x20`. Une sortie
relit son verrou, une entree rend le niveau exterieur. Cette distinction est
indispensable : le firmware pilote ses broches par bit-band, et le bus traduit
cela en lecture puis ecriture du mot entier.

**TE de l'ecran**, sur P1.10. Demi-periode de 800000 cycles, soit 60 Hz pour un
coeur a 96 MHz, cadence deduite du SysTick arme a 95999. Son front montant leve
l'IRQ 27, dont le gestionnaire `0x0000C120` efface le drapeau et incremente le
compteur de trames `0x1801C2C0`.

**Controleur de transferts**, page `0x4000F000`, canaux en `0x100` et `0x120`.
Par canal : controle en `0x00` avec le bit 0 pour partir, configuration en
`0x04`, source en `0x08`, destination en `0x0C`, nombre d'unites sur 22 bits en
`0x14`. Drapeaux communs en `0x00`, acquittement en `0x08`, commande en `0x10`.
La fin leve l'IRQ 58, dont le gestionnaire `0x10014050` charge le descripteur
`0x1801C9C0` et le repasse a l'etat 1.

## Le blocage actuel

La routine de remplissage en `0x00007040` ecrit un mot, avance de quatre octets
et decremente un compteur. Son pointeur part de `0x180142A8`, soit le debut du
tampon d'image, et devrait s'arreter au bout. Il atteint `0x18B83F0C`, tres
au-dela de la memoire vive, et ne s'arrete jamais. Seuls 64 pixels sont ecrits.

La cause est en amont : la source decodee, en `0x1800ECF4`, est **entierement a
zero**. Aucun transfert ne l'a remplie. Le seul acces aux ressources releve est
`flash 0x25F33C -> 0x1800FFB4`, sur 32 octets, soit un en-tete. Le corps de la
ressource n'est jamais charge, et le decodeur tourne sur du vide.

**Piste concrete** : trouver qui devrait charger ce corps. Poser `WATCH_MEM` sur
`0x1800ECF4` pour confirmer que rien ne l'ecrit, puis remonter le chemin qui lit
l'en-tete en `0x1800FFB4` et voir quelle branche, apres cette lecture, aurait du
declencher la lecture du corps. La zone des ressources va de `0x111000` a
`0x8286C3`, bien au-dela de la fenetre XIP de 1 Mo : le chargement passe donc
par le DMA de la flash ou par un rebasage de la fenetre.

## Ce qu'il ne faut pas refaire

- Ne pas retoucher au sens du bit de direction du DMA de flash : pose, il va de
  la flash vers la memoire. Le prendre a l'envers ecrase les pages de sauvegarde
  avec un tampon rempli du motif de poison `0xAB`.
- Ne pas chercher un identifiant JEDEC de trois octets en `0x40022018` : le
  firmware n'en lit que deux, et n'en garde que le fabricant.
- Ne pas supposer que le SysTick cadence la veille : le firmware le desactive
  explicitement avant de dormir.
