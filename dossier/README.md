# Le dossier

Le dossier technique de retro-ingenierie du Tamagotchi Paradise. Il vivait dans
son propre depot, `dossier-paradise-snc73410`. Il est ici depuis le 28 aout
2026, parce que le separer n'avait plus de sens : il decrivait ligne a ligne des
fichiers qui vivaient ailleurs, et il fallait recopier ces fichiers a chaque
seance pour que le dossier reste vrai. Les copies finissaient toujours par
diverger.

## Ou est le dossier

`index.html`, a la racine du depot. C'est la page servie par GitHub Pages.

La page est autonome : aucun script ni feuille de style externe, hors les
polices Google Fonts, qui ont une pile de repli declaree. Fond gris sombre,
accents orange et cyan. Elle s'adapte au format mobile, et les tableaux larges
defilent dans leur propre cadre plutot que de pousser la page.

Pour la lire hors ligne, l'ouvrir directement dans un navigateur. Aucun serveur
n'est necessaire.

## Ce que le dossier cite, et ou ca se trouve maintenant

Le dossier ne recopie plus rien. Il renvoie aux fichiers du depot, qui sont la
source :

| Ce que le dossier documente | Le fichier |
|---|---|
| AES-128 et AES-256 ecrits a la main, sans dependance | `src/emulator/aes.rs` |
| Load tables Sonix, derivation d'IV, CBC inverse et OFB permute | `src/emulator/sonix.rs` |
| Carte memoire et tracage des acces aux peripheriques | `src/emulator/mmu/mod.rs` |
| Le decodeur ARMv7-M, et les seize defauts corriges | `src/emulator/cpu/thumb16.rs`, `thumb32.rs` |
| Les fusibles FEUSE porteurs de la deviceKey | `src/emulator/peripherals/fuses.rs` |
| Le controleur XIP, que le datasheet nomme GPIO2 | `src/emulator/peripherals/xip.rs` |
| La zone systeme SN_SYS0, le compteur de secondes et l'alarme, donc tout le temps du jeu | `src/emulator/peripherals/snsys.rs` |
| Le format des sauvegardes persistantes | `src/emulator/sauvegarde.rs` |

Les notes de travail dont le dossier est la synthese sont dans `docs/` :
`ou-on-en-est.md` pour l'etat exact du projet, `materiel-snc7340.md` pour le
releve materiel, `reprise.md` pour reprendre le travail.

## Le document est cumulatif

Je n'efface rien quand je me suis trompe. Les vieux constats restent ou ils
sont, et j'ecris a cote ce qui les a remplaces. C'est pour ca que la section des
dementis de la carte peripheriques existe : cinq etiquettes du datasheet se sont
revelees fausses, et j'ai passe des jours a les croire. Autant que ca serve.

## Ce que le dossier n'inclut pas

Aucun binaire proprietaire, aucun dump de Flash, aucune image de Boot ROM. Les
procedures d'extraction decrites s'appliquent a un appareil dont vous etes
proprietaire.

La deviceKey necessaire au dechiffrement est gravee dans les fusibles de la
puce. Elle ne figure nulle part dans un dump et se lit en SWD sur l'appareil.
Elle n'est ni dans la page, ni dans le code, ni dans l'historique git : voir
`docs/reprise.md` pour la fournir par `SONIX_DEVICE_KEY`.

## Origine des travaux

Le dechiffrement, le format de load table et la carte memoire detaillee ont ete
retrouves par retro-ingenierie dans ce projet, sans executer aucun outil tiers.
La seule ressource exterieure utilisee est la documentation publique de la
semantique des drapeaux d'en-tete, dans le depot `sonix-boot-decrypter`.

Les travaux publics de Yukai Li (GMMan / Caralynx), juillet a octobre 2025,
couvrent le layout Flash, le brochage et l'extraction de la Boot ROM. Ils sont
references en fin de dossier.

## Marques

Tamagotchi et Tamagotchi Paradise sont des marques de Bandai. Ce projet n'est ni
affilie ni approuve par Bandai.
