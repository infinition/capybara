# Feuille de route

Ce qui reste a faire. L'etat courant est dans `ou-on-en-est.md`, le point
d'entree pour reprendre dans `reprise.md`.

## Defauts connus

**Un bourdonnement en tete des sons de Jade Forest.** Releve a la sonde :
quarante secondes de 196 Hz, un Sol3 juste, avant que la melodie commence. Le
drapeau de son en cours est leve pendant tout ce temps et une voix porte cette
valeur. A trancher : voix de fond reelle du firmware, ou entree figee que le
modele prend pour une note. `melodie_probe` le montre en une commande.

**Le peripherique de sortie du son reste introuvable.** Le moteur du firmware
calcule ses notes mais n'ecrit sur aucun peripherique connu. Voir la section
« Le son » de `ou-on-en-est.md` pour ce qui a ete mesure.

La lecture en periode change la recherche, et c'est ce qu'il faut essayer en
premier : le registre attendu ne recoit pas un diviseur de 96 MHz par la
frequence, mais le compte lui meme, 568 ou 1516, sur une base a 1,5 MHz. C'est
ce nombre la qu'il faut chercher dans toutes les ecritures materielles pendant
une note, sans filtre de PC, avec un prediviseur a 64 pose quelque part.
Trancher cela reglerait aussi la question du volume : si la console le regle en
modulant le rapport cyclique, notre carre a cinquante pour cent est faux en
nature et pas seulement en timbre.

**Les quatre autres editions ne sont pas menees jusqu'au bout.** Seule Water a
ete jouee de bout en bout. Jade Forest affiche et sonne. Earth, Sky et Land
n'ont pas ete suivies jusqu'a l'image.

## A faire

### Mode UART, pour parler au monde exterieur

**Ce qu'attend l'outil officieux, releve dans TamaHome 5.0.6.** C'est une
application .NET Avalonia, `TamaParadiseTool.dll`. Elle embarque
`System.IO.Ports` et appelle `SerialPort.GetPortNames` : elle liste donc tous
les ports serie du systeme, sans filtrer sur un identifiant de fabricant, et
retient le dernier choisi dans son reglage `LastComPort`. N'importe quel port
fait donc l'affaire.

Son service de communication porte `deviceComm`, `configureComm`,
`getActiveComm` et un type `TcpComm`, mais ce dernier sert au relais en ligne,
`OnlineRelayBridge` avec ses pompes `PumpSerialToSocket` et
`PumpSocketToSerial` : c'est le flux serie qui est renvoye dans une prise
reseau pour jouer a deux, pas une seconde voie vers l'appareil. Le transport
vers la console reste le port serie.

L'echange lui meme s'appelle une visite : `PlaydateWireData`,
`ParsePlaydateData`, `SendPlaydate`, `PlaydateIdentity`.

**L'obstacle, a savoir avant de commencer.** Windows n'enumere en COMxxx que ce
qu'un pilote declare. Aucune application ne peut creer un port serie toute
seule. Il faut donc une paire de ports virtuels, com0com ou equivalent, que
l'utilisateur installe une fois : elle donne par exemple COM5 et COM6 relies
dos a dos, l'emulateur ouvre l'un, TamaHome voit l'autre comme un port
ordinaire. C'est la seule voie sans ecrire de pilote.

**Ce qu'on a trouve dans le firmware, et par ou continuer.**

La table des scenes est decodee : un tableau de descripteurs de vingt huit
octets, quatre gestionnaires, un pointeur de nom et un numero, cent vingt six
entrees. `PSID_HOME` y vaut 29, ce que la trace de demarrage confirme, la table
est donc juste. Les scenes qui nous interessent :

```text
   1  PSID_DEBUGMENU            17  PSID_DEVELOP_UARTTEST     0x1000C705
   2  PSID_DEVELOP_COMMONCTRL   18  PSID_DEVELOP_UARTAGEING   0x1000C325
  24  PSID_DEVELOP_TCP          19  PSID_DEVELOP_UARTBYTE     0x1000C471
 113  PSID_TAMASPACE_TUSHIN     20  PSID_DEVELOP_UARTHEADER   0x1000C5C1
 125  PSID_TESTMODE             29  PSID_HOME
```

Le firmware porte aussi un `** UART Monitor Service Ver.%d.%02d **` et des
entrees de menu `UART BYTE`, `UART AGEING`, `UART TEST` et `TAMA HOME`.

**`0x40038000` n'est pas le port serie.** J'ai cru le contraire une heure
durant : la page ressort douze fois dans la fenetre XIP, seule de son groupe,
et le code la scrute bit a bit. Le pilote lu jusqu'au bout dit autre chose.

Sa fonction de configuration, en `0x10005560`, recoit une adresse qu'elle
compare a `0x18000000`, donc un tampon en memoire vive, et une longueur. Sa
fonction d'attente, en `0x10005534`, boucle sur le temoin d'etat, coupe
l'autorisation de marche, puis lit `+0x1C` et le rend a l'appelant. Une adresse,
une longueur, un depart, une attente, un resultat : c'est un accelerateur qui
calcule une valeur sur un bloc de memoire. La note heritee avait raison, et
c'est moi qui l'ai mise en doute a tort.

La banque reste utile a consigner, elle servira le jour ou ce bloc sera
modelise :

```text
  +0x00  controle et etat. Bit 4 : autorisation de marche, efface avant de
         configurer, repose pour lancer, efface a la fin. Bits 4 et 9 scrutes
         comme temoins d'achevement.
  +0x04  adresse du tampon, verifiee en memoire vive
  +0x08  longueur
  +0x0C  mis a 1 a la configuration
  +0x10  mis a 1 a la configuration
  +0x14  format : deux bits de mode, et un champ de cinq bits en 4..8 qui
         recoit un nombre moins un
  +0x18  parametre pose d'un bloc
  +0x1C  resultat, lu a la fin
```

**Ce qu'on sait du lien physique.** 460800 bauds, niveaux 3,3 V, adaptateur de
type CH341A ou CH347. Cela vient de la communaute, pas de notre mesure, mais
c'est une valeur precise a confronter au diviseur qu'on trouvera.

**Le peripherique reste a identifier.** Un balayage des paires MOVW puis MOVT
sur les seize mega-octets donne les pages que le firmware forme : les ports
d'entrees-sorties, le controleur de flash, les convertisseurs, le controleur de
transferts, la fenetre XIP, la zone systeme, l'alias bit-band, et le bloc
ci-dessus. Ni `0x40034000` ni `0x40020000`, les deux adresses que le datasheet
donne pour UART1 et SPI1.

Attention en refaisant ce balayage : il avance de deux octets en deux octets
sans suivre les frontieres d'instruction, il decode donc aussi des donnees. Les
pages vues moins de cinq fois sont a verifier une par une au desassembleur avant
d'y croire ; j'ai perdu du temps sur deux d'entre elles.

**Une page tres sollicitee que personne n'a modelisee : `0x4000E000`.** Le
datasheet l'annonce en I2S4. Releve sur Jade Forest et sur Water, avec le meme
resultat : une banque entiere y est lue et ecrite en permanence, `+0x00`,
`+0x04`, `+0x08`, `+0x0C`, `+0x1C`, `+0x20`, `+0x24`, `+0x50`, plus d'un millier
d'acces par releve, toujours depuis le meme code en memoire programme, autour de
`0x000052F2`, `0x0000546A`, `0x00005404`, `0x0000548E`, `0x0000532C` et
`0x00003FDA`. Un canal du controleur de transferts, `0x4000F01C` a `0x4000F028`,
est mene par le code voisin en `0x00004440`.

Deux choses la rendent interessante. C'est la seule page inconnue de ce poids.
Et elle est pilotee depuis la memoire programme, alors que la recherche du
peripherique de son filtrait les acces sur l'intervalle de PC du module audio,
en `0x1001F000` a `0x10080000` : elle ne pouvait pas la voir. C'est exactement
le trou de methode qu'on soupconnait.

Ce qu'elle est reste a etablir, mais un chiffre penche deja. Son canal de
transfert compte quatre vingt quatorze operations par releve, et le meme releve
compte quatre vingt quatorze trames poussees vers l'ecran. Le trafic suit donc
la cadence d'affichage et non celle du son : c'est vraisemblablement l'interface
serie de la dalle, pas le buzzer. A confirmer en relevant son trafic pendant
qu'une note joue puis au silence, mais il ne faut pas s'emballer dessus.

**L'ecran de connexion ne reveille aucun peripherique nouveau.** Mesure faite
sur un instantane pris pendant que la console attendait sur cet ecran : les
memes onze pages qu'en jeu ordinaire, aux memes cadences. Deux lectures
possibles, et rien ne les departage encore : le lien serie passe par un bloc
deja sollicite, ou l'instantane n'est pas ou l'on croit.

**Attention aux numeros de scene d'une edition a l'autre.** Ceux du tableau
ci-dessus viennent de l'image Earth. Sur Jade Forest, dont on sait deja que la
zone de travail est decalee de huit octets, l'instantane pris sur l'ecran de
connexion donne 119 en `0x18001BF4`, ce qui vaudrait `PSID_TAMASPACE_RANK` dans
la numerotation d'Earth. Avant de conclure quoi que ce soit sur Jade, il faut
extraire sa propre table.

**Ce qui ne marche pas, pour ne pas le refaire.** Ecrire le numero de scene
voulu en `0x18001BF6` ne declenche aucune transition. Ce mot vaut `0xFFFF` au
repos, ce qui ressemble bien a « aucune transition demandee », mais y poser un
numero ne suffit pas : le firmware attend autre chose en plus. `uart_probe`
fait cet essai en une commande.

**Ce qui manque cote emulateur, et qui vient avant.** Le port serie n'est pas
modelise. `src/hw_bridge/uart_pcom.rs` et `src/emulator/peripherals/uart.rs`
datent d'avant la retro-ingenierie : registres inventes, et jusqu'a des
compteurs d'octets ecrits en dur. A jeter plutot qu'a completer.

Et le peripherique n'est pas identifie. Mesure sur une partie ordinaire :
aucune page serie n'est touchee, ni UART1 en `0x40034000`, ni SPI1, ni I2C1.
C'est attendu, le lien ne s'ouvre que dans le menu de connexion. Le premier
travail est donc d'atteindre ce menu avec `scene_probe` et de regarder quelle
page s'allume, exactement comme pour le buzzer.

Le port serie de la console sert a trois choses, par difficulte croissante.

- **Les logiciels PCOM.** Le firmware parle deja un protocole sur ce port ;
  `src/hw_bridge/uart_pcom.rs` en porte le debut. Il faut exposer le port a
  l'exterieur, par un port TCP local ou un port serie virtuel, pour qu'un
  logiciel PCOM existant s'y connecte comme a une vraie console.
- **Deux consoles emulees entre elles.** Une fois le port expose, relier deux
  instances revient a croiser leurs flux. C'est ce qui permet les visites et les
  echanges entre Tamagotchi.
- **Une console physique et une emulee.** Meme chose, avec un adaptateur USB
  serie en face. C'est le test le plus severe du modele : le vrai firmware juge
  les reponses.

### Explorateur d'assets

Voir tout ce que le firmware porte en images, sprites et sons, dans une vue qui
les liste et les affiche. Puis pouvoir les sortir, les remplacer et les
reinjecter dans la flash. Le decodeur de sprites existe deja,
`src/gui/sprites.rs`, et la carte de la flash est dans
`src/hw_bridge/flash_map.rs`.

### Timbre du buzzer

Le vrai Tamagotchi a un piezo dans une coque plastique : un transducteur tres
resonant, un pic entre deux et quatre kilohertz, presque rien en dessous d'un
kilohertz. Un haut parleur d'ordinateur restitue le fondamental en entier, d'ou
un son plus plein et plus grave que la console. Le signal electrique est le
meme, c'est le haut parleur qui differe.

Trois facons de s'en rapprocher, par cout croissant : un passe haut vers un
kilohertz avec une resonance, une vingtaine de lignes dans `audio.rs` ; une
reponse impulsionnelle relevee au micro sur la vraie console, puis convolution ;
ou l'echantillonnage des quatre vingt sept sons du banc, ce qui n'est plus de
l'emulation.

### Coques

Le dessin suit la vraie console : oeuf au rapport 6,5 sur 7,5, fente de
coquille, fenetre transparente decoupee sans trait, molette cannelee, boutons
d'une couleur qui tranche. Reste le decor imprime propre a chaque theme, corail
et bulles pour Blue Water, feuillages pour Pink Land. C'est moins urgent depuis
qu'on peut glisser son propre papier sous la fenetre.

## Fait

Pour memoire, et pour ne pas le refaire.

- La vitesse, de 0,73 a 1,00 fois le temps de la console.
- Le son : le champ de hauteur est une periode et non une frequence, base
  750 000. Les melodies ne sont plus a l'envers, ne trainent plus, ne gresillent
  plus, et ne jouent plus la note heritee de la precedente.
- Le tableau des voix est cherche en memoire : son adresse change d'une edition
  a l'autre, et c'est ce qui rendait Jade Forest muette.
- Trois modes, dont un mode jeu sans cadre, transparent, deplacable, avec tout
  au clic droit. Il a fallu passer au moteur wgpu : sur Windows le compositeur
  du bureau ignore la couche alpha des fenetres OpenGL.
- Points de reprise horodates, par sauvegarde, avec pose, import et export.
- Papiers de personnalisation glisses sous la fenetre transparente.
- Donnees dans le dossier du systeme, dumps detectes et importes, cle qui suit.
- Reglages tous retenus d'un lancement a l'autre.
