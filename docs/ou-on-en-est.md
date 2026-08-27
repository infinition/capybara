# Passation : état exact, et où ça bloque

Document de reprise. Il dit ce qui est établi, ce qui est incertain, et ce que
j'aurais fait ensuite. Le détail matériel est dans `materiel-snc7340.md`.

## Résumé en cinq lignes

Les cinq firmwares démarrent, valident leur sauvegarde, la réécrivent et
exécutent leur code applicatif. La clé de déchiffrement est trouvée
(`<CLE>`, commune aux cinq éditions). Le CPU exécute des centaines de
millions d'instructions sans encodage inconnu. Il n'y a **pas encore d'image à
l'écran**, et le chemin d'affichage n'est pas commencé.

## Outils, à connaître avant de plonger

Quatre sondes dans `examples/`. Toutes prennent `<dump.bin> <clé hex>`.

- **`boot_probe`** : le couteau suisse. Chargement, zones parcourues, adresses les
  plus exécutées, désassemblage de la boucle chaude, tous les registres touchés
  avec le PC qui les touche, transferts du contrôleur de flash, état des
  interruptions, et **la console de debug du firmware**.
- **`spin_probe`** : isole une boucle morte et affiche ses 60 derniers pas.
- **`watch_probe`** : s'arrête à la Nième visite d'une adresse, affiche les
  registres et reconstitue la chaîne d'appels depuis la pile.
- **`race_probe`** : vérifie si l'état vivant du jeu change entre deux points, et
  compte les exceptions qui s'intercalent.

Variables d'environnement utiles :

| Variable | Effet |
|---|---|
| `MMIO_PAGE=0x...` | journalise dans l'ordre tous les accès à une page de 4 Ko |
| `MMIO_FORCE=adr:val,...` | impose une valeur de lecture sur un registre non modélisé |
| `WATCH_COND=r6:0x...` | ne déclenche `watch_probe` que si un registre vaut une valeur |
| `MEM_DUMP=0x...` | vidange 64 octets à une adresse |
| `MEM_CMP=a:b:long` | compare deux zones et signale le premier écart |
| `PRINTF_PC=0x...` | adresse d'interception de la console, `0x1070` par défaut |

**La console du firmware est l'outil le plus rentable.** Dans la boucle de
formatage du `printf`, l'instruction `0x1070` appelle la fonction de sortie avec
le caractère dans `r0`. `boot_probe` la restitue automatiquement.

## Deux leçons payées cher

**Ne jamais croire une étiquette sur parole.** Le datasheet place GPIO2 en
`0x4002F000` : c'est le contrôleur XIP. Il place UART0 en `0x40038000` : c'est un
accélérateur de somme de contrôle. Les deux m'ont coûté des heures.

**Ne jamais croire un message d'erreur sur parole.** La chaîne
`unsupport chip, please check your flash vender` n'a rien à voir avec la flash.
C'est du texte de repli du SDK Sonix, imprimé **à la fin de la routine de
formatage de sauvegarde `0x0B76C`**, quoi qu'il arrive. Sa présence n'indique pas
un échec. J'ai perdu beaucoup de temps à modéliser un identifiant JEDEC pour rien.

## Le blocage actuel, honnêtement

Water exécute un milliard d'instructions, partagées entre PRAM et XIP, sans
plantage ni encodage inconnu. Il tourne. Mais il repasse en boucle par la routine
de formatage de sauvegarde au lieu de progresser vers le jeu.

Ce que j'ai **vérifié** et qui est donc hors de cause :

- la validation de sauvegarde **réussit** : à `0x0B5EC` les deux valeurs comparées
  valent `0x80F3`, identiques ;
- les transferts du contrôleur de flash sont sémantiquement corrects, tracés un
  par un : trois lectures de validation, puis écriture et relecture de
  vérification sur chacun des deux emplacements ;
- l'écriture en flash est **exacte**, octet pour octet, en-tête cohérent ;
- toutes les sommes de contrôle calculées concordent ;
- l'état vivant **ne change pas** entre le calcul de la somme et la recopie, et
  aucune exception ne s'intercale.

Ce qui reste à comprendre : pourquoi `0x0B76C` est appelée alors que la
validation réussit. Le dispatcher est en `0x09032`, il enchaîne
`0x0B574(0)` puis `0x0B574(1)` et n'appelle le formatage que si les deux
échouent. Il faut instrumenter ce dispatcher, pas les fonctions en dessous.

**Piste concrète** : poser un `watch_probe` sur `0x09038` et `0x09042` pour lire
la valeur de retour de chaque validation telle que le dispatcher la voit. Si elle
diffère de ce que `0x0B5F2` laisse dans `r0`, le problème est dans le chemin de
retour, pas dans la validation.

## Le chemin d'affichage, non commencé

Aucun contrôleur LCD n'existe dans le datasheet. L'écran passe donc par un autre
biais, vraisemblablement SPI1 (`0x40020000`) et un canal DMA. Rien de tout cela
n'apparaît encore dans la trace, ce qui est cohérent : le firmware n'a pas atteint
son initialisation d'écran.

Les entrées, elles, sont prêtes : le port 2 est modélisé en `0x4001A000` avec ses
résistances de tirage, et `GpioPort::appuyer` / `relacher` attendent d'être
câblées sur des boutons.

## Ce que je ferais dans cet ordre

1. Instrumenter le dispatcher `0x09032` pour comprendre pourquoi le formatage est
   appelé. C'est le seul verrou identifié, et il est cerné.
2. Une fois passé, relever ce que le firmware touche de neuf dans la trace : la
   séquence d'initialisation de l'écran devrait y apparaître.
3. Modéliser SPI1 et son DMA, puis recopier le tampon d'image vers le panneau de
   l'émulateur.

## Ce qu'il ne faut pas refaire

- Ne pas chercher à satisfaire l'identifiant JEDEC de la flash, c'est une fausse
  piste complète.
- Ne pas forcer `0x4001A000` à la main, le port est modélisé proprement.
- Ne pas supposer que le SysTick cadence la veille : le firmware le désactive
  explicitement avant de dormir.
