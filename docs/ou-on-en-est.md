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

**Les deux bugs les plus couteux etaient dans le coeur, pas dans les
peripheriques.** Un immediat mal etendu et un `ITSTATE` non vide a l'entree en
exception ont chacun coute des jours d'enquete sur de fausses pistes.

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

1. **Le panneau de l'interface.** Le tampon d'image est lisible en
   `0x180142A6` ; il faut le brancher sur la fenetre de l'application, pas
   seulement sur `ecran_probe`.
2. **Les entrees en direct.** `Machine::appuyer` est prete ; il reste a la
   cabler sur le clavier de l'interface, et a modeliser l'encodeur en quadrature
   plutot qu'en simples appuis.
3. **Le son.** Le buzzer est sur P1.11 et P1.13 en PWM. Rien n'est modelise.
4. **Traverser la premiere mise en route.** Langue, date, anniversaire, nom de
   planete. Chaque ecran se pilote deja par `ENTREES`.
5. **Les quatre autres editions.** Seule Water a ete suivie jusqu'au bout ; les
   autres passent l'identification mais n'ont pas ete menees jusqu'a l'image.

## Ce qu'il ne faut pas refaire

- Ne pas retoucher au sens du bit de direction du DMA de flash : pose, il va de
  la flash vers la memoire.
- Ne pas chercher un identifiant JEDEC de trois octets en `0x40022018`.
- Ne pas supposer que le SysTick cadence la veille : le firmware le desactive
  explicitement avant de dormir.
- Ne pas conclure d'un desassemblage a froid sans le recouper avec la trace
  d'execution de `watch_probe`.
