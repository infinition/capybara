# Tamagotchi Paradise, emulateur

Emulateur du Tamagotchi Paradise (Bandai, 2025), console a microcontroleur Sonix
SNC7340, ecrit en Rust avec une interface egui. Le vrai firmware demarre et le
jeu se joue : l'oeuf eclot, l'horloge avance, les jauges descendent, le son sort,
et la sauvegarde survit a l'extinction de l'ordinateur.

**Le dossier de retro-ingenierie est la page `index.html` a la racine.** Il
raconte le brochage, la carte memoire, le format des load tables Sonix, la
derivation de cle AES, les procedures d'extraction, et les seize defauts de
decodage ARMv7-M trouves en faisant tourner du vrai code. C'est la partie la plus
reutilisable du projet. Voir `dossier/README.md`.

## Ou en est l'emulateur

Ce qui marche : mise en route complete, boutons et molette, horloge qui avance,
eclosion, nourrissage, QR code secret, les deux veilles et le reveil materiel,
sauvegardes persistantes par dump avec vieillissement en temps reel, coque fidele
suivant l'edition chargee, et les notes que le firmware compose.

**Vitesse : 1,07 fois le temps de la console sur `step`, 1,17 sur `run_frame`**,
mesure du 28 aout 2026. Le gouverneur de l'interface la retient a 1,00. Il n'y a
plus de defaut visible a l'ecran.

Ce qui reste ouvert : le peripherique par lequel la console fait sortir ses notes
n'est pas identifie, l'interface synthetise donc les frequences que le firmware a
deja calculees. Seule l'edition Water a ete menee jusqu'au bout. La molette est
modelisee en appuis simples, pas en signal a deux phases decalees. Le second
coeur n'est pas emule, et le firmware ne l'a jamais reclame.

Le detail complet est dans `docs/ou-on-en-est.md`. Pour reprendre le travail,
commencer par `docs/reprise.md`.

## Mise en route

Il faut un dump de la Flash de votre propre appareil, et la deviceKey lue en SWD
sur cet appareil. Ni l'un ni l'autre ne sont dans ce depot, et ils n'y entreront
pas.

```bash
cargo run --release
```

Le dump se charge depuis l'interface. Pour les tests et les sondes, deux
variables designent le materiel de travail :

```bash
export SONIX_DEVICE_KEY=0x........
export SONIX_DUMPS=<dossier contenant les .bin>
cargo test --release
```

Sans ces variables, les tests qui dependent d'un dump se sautent proprement et la
suite reste verte. C'est voulu : le depot doit passer ses tests chez quelqu'un qui
n'a pas l'appareil. **Verifier les deux cas quand on touche a ce mecanisme**,
sinon on ne sait pas si les tests passent ou s'ils ne font plus rien.

## Commandes

| Touche | Action |
| --- | --- |
| `A` / fleche gauche / clic gauche sur l'ecran | bouton A |
| `B` / espace / entree / clic droit sur l'ecran | bouton B |
| `C` / echap / fleche droite | bouton C |
| molette de la souris / clic milieu | molette laterale, appui long possible |
| `F10` | pas a pas, dans le debogueur |

## Les sondes

Vingt-deux programmes dans `examples/`, tous en `<dump.bin> <cle hex> [...]`. Les
plus rentables :

- `scene_probe`, avec `RESET`, `SORTIE_ETAT` et `TRACE_PAS`
- `temps_probe`, pour trouver une boucle morte : il donne la repartition du temps par PC
- `watch_probe`, pour arreter sur une adresse ou une modification de memoire
- `vitesse_probe`, qui mesure les deux boucles, `step` et `run_frame`
- `son_probe` et `partie_probe`

## Sauvegardes et instantanes

A ne pas confondre. Un instantane, `.tamastate`, fige toute la machine pour
revenir en arriere pendant la mise au point. Une sauvegarde, `.tamasave`, ne
retient que ce que la console garde vraiment, et vit a cote de l'executable dans
`sauvegardes/<empreinte du dump>/`. L'empreinte mele le nom du fichier et une
somme FNV-1a de son contenu : les cinq editions ne se melangent pas, et deux
copies renommees du meme dump se retrouvent.

Le fichier porte les pages de Flash ecrites par le jeu, mais aussi l'horloge de
la console. Un Tamagotchi range vieillit donc pendant que l'ordinateur est
eteint, comme le vrai.

## Ce que ce depot ne contient pas

Aucun binaire proprietaire, aucun dump de Flash, aucune image de Boot ROM,
aucune cle. La deviceKey est gravee dans les fusibles de la puce, ne figure dans
aucun dump, et se lit en SWD sur l'appareil. L'historique git a ete reecrit le
28 aout 2026 pour qu'elle n'apparaisse dans aucun commit.

Les procedures d'extraction decrites dans le dossier s'appliquent a un appareil
dont vous etes proprietaire.

Tamagotchi et Tamagotchi Paradise sont des marques de Bandai. Ce projet n'est ni
affilie ni approuve par Bandai.
