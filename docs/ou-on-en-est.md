# Passation : etat exact, et ou ca bloque

Document de reprise. Il dit ce qui est etabli, ce qui est incertain, et ce que
j'aurais fait ensuite. Le detail materiel est dans `materiel-snc7340.md`.

## Resume

Les cinq firmwares demarrent, valident leur sauvegarde, passent l'identification
de la flash, initialisent leur ecran, decodent leurs ressources et **affichent
une image**. L'ecran de demarrage de Water a ete rendu en entier : 128 x 128 en
RGB565, texte japonais et deux personnages dessines.

La cle de dechiffrement est `<CLE>`, commune aux cinq editions.

Le firmware franchit maintenant sa mesure de pile et entre dans sa **boucle de
jeu**, ou il synchronise plusieurs centaines de trames. Il n'y dessine pas
encore : sa machine a etats reste sur l'etat 101 avec une transition vers 102
(`PSID_STARTUP`) qui n'est jamais appliquee.

## Outils

Six sondes dans `examples/`. Toutes prennent `<dump.bin> <cle hex>`.

- **`boot_probe`** : le couteau suisse. Zones parcourues, adresses les plus
  executees, desassemblage de la boucle chaude, tous les registres touches avec
  le PC qui les touche, transferts de flash, etat des interruptions, et la
  console de debug du firmware.
- **`dis_probe`** : desassemble a froid un intervalle. Programme la fenetre XIP
  avant de lire, sans quoi tout le code au dela de `0x10000000` se lit decale de
  `0x11000`, ce qui donne un desassemblage plausible mais faux.
- **`watch_probe`** : s'arrete a la Nieme visite d'une adresse ou a la Nieme
  modification d'un mot, et rend registres, pile d'appels reelle, trace des pas
  executes avant et apres l'arret. C'est la sonde la plus rentable.
- **`ecran_probe`** : rend le tampon d'image en PPM, en lisant source et
  longueur dans le canal plutot qu'en les supposant.
- **`spin_probe`** : isole une boucle morte et affiche ses 60 derniers pas.
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
| `SONIX_PILE=0x...` | impose l'echantillon de mesure de pile |
| `PILE_USEE=1` | laisse le drapeau de pile faible du dump en place |

## Quatre lecons payees cher

**Ne jamais croire une etiquette sur parole.** Le datasheet place GPIO2 en
`0x4002F000` : c'est le controleur XIP. UART0 en `0x40038000` : c'est un
accelerateur de somme de controle. I2S0 en `0x40019000` : c'est le port
d'entrees-sorties numero 1. Le chien de garde en `0x4003A000` : c'est le
convertisseur de la mesure de pile.

**Ne jamais croire une conclusion heritee sans la refaire.** La note precedente
affirmait que `unsupport chip, please check your flash vender` etait du texte de
repli imprime sans consequence. C'etait une boucle sans sortie en `0x1006A018`,
et c'etait le vrai verrou du demarrage.

**Ne jamais desassembler a froid sans programmer la fenetre XIP.** Un decalage
de `0x11000` produit du code qui se lit sans erreur et ne veut rien dire.

**Se mefier d'un modele qui compense un bug du coeur.** Le registre `0x40022014`
avait ete pris pour un identifiant rendu octet par octet, parce qu'un `CMP.W`
faux faisait echouer la comparaison qui distingue lecture et ecriture. Corriger
le coeur a fait tomber le modele, et revele que c'est un registre de
configuration de la puce.

## Ce qui est etabli et modelise

**Immediats modifies du jeu d'instructions.** `ThumbExpandImm` replique son
octet selon quatre motifs quand `imm12[11:10]` vaut 00, il ne le decale pas.
`0xFFFFFFFF` valait `0xFF000000`, et tout `CMP.W rX, #-1` rendait un verdict
faux. Le decodeur de sprites en `0x1006A20C` s'en sert pour distinguer une
repetition d'une suite litterale : il ne voyait que des repetitions.

**Identification de la flash.** Bit 15 de `0x40022004`, attente qu'il retombe
ainsi que le bit 1, puis lecture de `0x40022018` d'un bloc en `0x000039E8`. Le
firmware en fait `(valeur & 0xFFFF) << 8` et compare les bits 23:16 a `0xC2`
(Macronix) puis `0xC8` (GigaDevice). Le registre porte la paire fabricant et
composant, `0xC217`. La console affiche `[example]flash id:c21700`.

**Registre de configuration de la flash**, en `0x40022014`. Le firmware le lit
en appelant sa routine d'acces avec -1, attend `0x40`, et l'ecrit sinon en
deposant la valeur en `0x40022010` puis en posant le bit 11 de la commande. Son
bit 0 est le temoin d'ecriture en cours, scrute apres chaque programmation.

**Ports d'entrees-sorties**, en `0x40018000`, `0x40019000` et `0x4001A000`.
Donnees en `0x00`, direction en `0x04`, mode en `0x08`, autorisation
d'interruption en `0x18`, drapeaux en `0x1C`, effacement en `0x20`. Une sortie
relit son verrou, une entree rend le niveau exterieur. Cette distinction est
indispensable : le firmware pilote ses broches par bit-band, et le bus traduit
cela en lecture puis ecriture du mot entier.

**TE de l'ecran**, sur P1.10, demi-periode de 800000 cycles, soit 60 Hz pour un
coeur a 96 MHz. Son front montant leve l'IRQ 27, dont le gestionnaire
`0x0000C120` efface le drapeau et incremente le compteur de trames
`0x1801C2C0`.

**Controleur de transferts**, page `0x4000F000`, canaux en `0x100` et `0x120`.
Par canal : controle en `0x00` avec le bit 0 pour partir, configuration en
`0x04`, source en `0x08`, destination en `0x0C`, nombre d'unites sur 22 bits en
`0x14`. Drapeaux communs en `0x00`, acquittement en `0x08`, commande en `0x10`.
La fin leve l'IRQ 58, dont le gestionnaire `0x10014050` charge le descripteur
`0x1801C9C0` et le repasse a l'etat 1.

**Convertisseur de la mesure de pile**, page `0x4003A000`. Controle en `0x00`,
commande en `0x04` avec le bit 0 pour partir, resultat en `0x08` sur dix bits
cales au rang 6. Le firmware ne relance jamais la conversion : elle s'enchaine
et leve l'IRQ 9 a chaque echantillon.

## La chaine de la mesure de pile

Etablie pas a pas, elle sert de modele pour les prochaines enquetes :

```text
  0x10003754  compare la tension au seuil 0x23332
  0x10003830  la lit dans le mot 0x18005C68
  0x1000397E  l'y ecrit, a partir des echantillons accumules
  0x10003924  n'accumule que si le gestionnaire a pose son drapeau
  0x10078774  gestionnaire de l'IRQ 9, extrait par UBFX #6, #10
```

Sans tension, le firmware pose le drapeau de pile faible dans l'etat sauvegarde
en `0x10010E54`, imprime `** LOW BATTERY FLAG DETECTED **` en `0x10030E5E`,
passe a l'etat 111 et s'eteint apres avoir affiche son message. Le drapeau est
le bit 3 du premier octet de l'etat. Les dumps Earth et Land le portent deja,
Water, Sky et Jade Forest non ; `Machine::remplacer_la_pile` l'efface et refait
la somme de controle des deux pages.

## Le blocage actuel

La boucle de jeu tourne : 369 trames synchronisees en 400 millions de pas. Mais
la machine a etats reste sur l'etat 101 avec une transition vers 102
(`PSID_STARTUP`) en attente, jamais appliquee. Le mot d'etat est en
`0x18001BF4` : demi-mot bas l'etat courant, demi-mot haut la transition
demandee, `0xFFFF` quand il n'y en a pas.

```text
  34 293 266  etat 101 pose par 0x0000954C
  34 301 826  transition effacee par 0x00009702
  34 307 577  transition 28 demandee par 0x00001F8C
  37 155 743  transition 102 demandee par 0x00001F8C
              plus rien : l'etat courant ne change jamais
```

Consequence : le rendu `0x00006CEC` n'est plus appele du tout, et l'afficheur ne
recoit qu'un seul transfert.

**Piste concrete** : trouver ou la transition est appliquee. Le demi-mot bas
n'est ecrit qu'en `0x00009540` par la sequence `MOVW r1,#0x1BF4 / MOVT / MOVS
r0,#101 / STRH`, donc l'application passe par une autre base de registre. La
boucle principale est appelee en `0x1006D1CA` ; son corps est autour de
`0x000096C8` a `0x0000985A`. Un troisieme demi-mot, en `0x18001BF8`, est compare
a 101 en `0x0000970C` : il joue probablement le role d'etat precedent ou
d'etat en cours de sortie, et c'est lui qu'il faut suivre.

Deux hypotheses a departager :

1. La sortie de l'etat 101 attend un evenement encore non modelise, par exemple
   une fin d'animation ou une entree utilisateur.
2. L'application de la transition se fait dans une branche que nous ne prenons
   pas, faute d'un registre ou d'une interruption.

## Les entrees, pretes a cabler

Rien ne presse tant que le rendu ne repart pas, mais tout est en place :
`GpioPort::appuyer` et `relacher` sur les ports modelises, et le brochage des
boutons est connu (`materiel-snc7340.md`). Bouton A sur P0.9, B sur P0.11, C sur
P0.10, molette sur P0.8, encodeur sur P2.0 et P2.1. Une entree au repos se lit
haute, un appui la tire bas.

## Ce qu'il ne faut pas refaire

- Ne pas retoucher au sens du bit de direction du DMA de flash : pose, il va de
  la flash vers la memoire. Le prendre a l'envers ecrase les pages de sauvegarde
  avec un tampon rempli du motif de poison `0xAB`.
- Ne pas chercher un identifiant JEDEC de trois octets en `0x40022018` : le
  firmware n'en lit que deux, et n'en garde que le fabricant.
- Ne pas supposer que le SysTick cadence la veille : le firmware le desactive
  explicitement avant de dormir.
- Ne pas conclure d'un desassemblage a froid sans le recouper avec la trace
  d'execution de `watch_probe` : les zones melant code et donnees se decalent.
