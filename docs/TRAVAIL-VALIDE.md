# Travail validé

Une ligne par étape terminée, jamais avant. Une étape absente de ce registre n'est pas
considérée comme finie, même si son code existe.

Une étape est terminée quand : son code est dans `main`, les vérifications GitHub sont vertes,
un agent distinct de celui qui l'a écrite l'a relue, et les constats de la relecture sont
traités ou notés ici comme restant ouverts.

Pour chaque étape : la date, le commit qui l'a fait entrer dans `main`, qui a relu et avec quel
niveau d'effort, ce que la relecture a contrôlé et ce qu'elle n'a pas pu contrôler, le résultat
des vérifications GitHub (essais, mémoire au repos, puis chrono de la sélection à partir de
l'étape 5), et le résultat de l'essai sur le PC de la maison quand l'étape en prévoit un.

## MVP, version 0.1 « Zone »

Les étapes 1 à 3 sont entrées ensemble dans `main` le 2026-11-11 par le commit `4b55f95`
(demande d'intégration n° 1, tête `6982c83`).

Vérifications GitHub sur `6982c83` (exécution n° 6), toutes vertes :

- Windows : `cargo fmt`, `cargo clippy -D warnings`, 153 tests et 2 tests d'architecture,
  construction du `.exe`, puis essais sur le vrai `.exe` : `01-socle` 19 points, `02-reglages`
  16 points, `03-raccourci` 43 points, aucun échec.
- macOS : `cargo check` et `cargo test`.
- Pages : la page du bandeau dans Chromium.
- Mémoire privée au repos (l'app et son moteur web, bandeau prêt) : 97,9 Mo.

### Étape 1, le socle

- Écrite par un agent, effort moyen. Relue par un agent distinct, effort élevé, sur `ac5163c`.
- La relecture a contrôlé : les appels Win32 un par un, la vie sans fenêtre, l'instance
  unique, le journal et sa rotation, la table des états, les essais PowerShell, la
  fabrication GitHub ; elle a tenté de tromper le test d'architecture (9 formes sur 13
  passaient).
- 11 corrections faites (`f83fae8` à `e023175`) : test d'architecture réécrit avec `syn`,
  journal indépendant du dossier Images, journal de secours, rotation qui ne peut plus perdre
  les archives, icône réellement vérifiée dans la zone de notification, manifeste cherché
  dans le `.exe`, version de Rust figée, essais plus stricts.
- Prouvé sur la machine Windows de GitHub : icône visible, échelle par écran déclarée et
  manifeste présent dans le `.exe`, seconde instance refusée, `--quitter`, arrêt propre.
- Reste ouvert : les états Gel et Découpe n'ont pas encore d'issue d'erreur avec délai (à
  traiter à l'étape 4) ; rien n'a tourné sur macOS au-delà de la compilation et des tests.

### Étape 2, les réglages

- Écrite par un agent, effort moyen. Relue par un agent distinct, effort élevé, avec ses
  propres essais hostiles ; corrections contre-vérifiées par le même relecteur.
- La relecture a trouvé deux pertes de données possibles, corrigées : une sauvegarde neuve
  supprimée quand l'horloge avait reculé, et un fichier illisible ou impossible à mettre de
  côté écrasé à la première écriture. Aussi corrigés : version hors limites, nettoyage qui
  touchait des fichiers au nom proche, fichier abîmé en lecture seule, fichier trop gros.
- 65 tests. Prouvé sur la machine Windows de GitHub : fichier créé, fichier abîmé mis de
  côté, champ inconnu conservé, version plus récente et fichier juste laissés identiques
  octet pour octet.
- Acceptés tels quels : un très grand nombre dans un champ inconnu peut perdre en précision ;
  le fichier temporaire porte un nom fixe (une seule instance écrit) ; un lien symbolique à
  la place du fichier est remplacé par un fichier ordinaire (à revoir pour le Mac).
- Non contrôlé : un vrai verrou d'antivirus ou de OneDrive, un dossier redirigé hors ligne.

### Étape 3, le raccourci et le bandeau de messages

- Logique des raccourcis et page du bandeau écrites par un agent, effort moyen ; relues par
  un agent distinct, effort élevé (4 défauts bloquants dans la page : injection par un
  libellé, message trop long impossible à fermer, action envoyée deux fois, message invalide
  affiché), corrigées.
- Assemblage (fenêtre du bandeau, réservation réelle, câblage) écrit par un agent, effort
  élevé parce que rien de ce code Windows ne pouvait tourner avant GitHub ; relu par un autre
  agent, effort élevé, qui a trouvé dans les sources de la bibliothèque de fenêtres que le
  bandeau n'était jamais réellement caché sous Windows et qu'il apparaissait dans Alt+Tab.
  10 corrections, contre-vérifiées, puis 5 retouches.
- Prouvé sur la machine Windows de GitHub : Impr. écran reçue quand elle est libre ; repli
  sur Ctrl + Maj + 1 quand Windows la réserve ou qu'une autre application la tient ; annonce
  affichée une seule fois par situation ; bandeau peint (copie d'écran : fond `#F9F9F9` sur
  4 points) puis réellement caché ; fenêtre de style outil, sans activation ; clic sur la
  croix sans changement de fenêtre au premier plan ; clics traversants à côté de la pilule ;
  pilule absente des captures d'écran ; raccourci libéré à l'arrêt.
- Règle appliquée en cours de route (demande du 11/11) : un message tient en une ligne, sans
  astuce ; seule une erreur porte sa cause.
- Reste ouvert : rien n'a tourné à une échelle autre que 100 % ni sur plusieurs écrans (seuls
  les calculs sont testés, l'essai se fera sur le poste à trois écrans) ; le filet de
  sécurité (bandeau caché d'office si la page ne répond plus) est testé en fonction pure,
  pas provoqué ; les raccourcis par défaut du Mac sont provisoires ; l'affichage écrit « Win »
  partout, il faudra « Cmd » sur Mac.

## MVP, version 0.1 « Zone », suite : capturer une zone de bout en bout

Les étapes 4, 5 et 6 et leur assemblage sont entrés ensemble dans `main` le 2026-11-11 par
le commit `860806e` (demande d'intégration n° 2, tête `981e262`, verte elle aussi).

Vérifications GitHub sur `442da12` (exécution n° 18, 2026-11-11), toutes vertes :

- Windows : `cargo fmt`, `cargo clippy -D warnings`, 269 tests et 2 tests d'architecture
  (2 mesures ignorées), construction du `.exe`, puis essais sur le vrai `.exe` : `01-socle`
  19 points, `02-reglages` 16, `03-raccourci` 40, `04-zone` 92, aucun échec.
- macOS : `cargo check` et `cargo test`.
- Pages : le bandeau et la sélection dans Chromium (sélection : 159 points).
- Machine de l'essai : un écran 1024×768 à 100 %.
- Mémoire privée au repos (l'app et son moteur web, bandeau et sélection prêts) : 125,3 Mo
  (97,9 Mo avant la fenêtre de sélection).
- Chronos lus dans le journal, sur 21 captures : écran figé visible après l'appui, médiane
  29 ms (de 10 à 65) ; sélection prête, médiane 38 ms (de 34 à 54) ; du relâchement à
  l'annonce, médiane 8 ms (de 6 à 15, sur 13 captures).

### Étape 4, l'écran figé

- Écrite par un agent, effort élevé parce que rien de ce code Windows ne tourne avant
  GitHub. Relue par un agent distinct, effort élevé : 9 constats, tous corrigés, puis 4
  constats à la contre-vérification, corrigés.
- Corrigé après relecture : les deux coins d'un glisser sont inclus ; l'écran figé se cache
  seul si un écran change ou si la session se verrouille ; pas de nouvelle photo tant que
  l'ancienne est montrée ; un fil de garde arrête l'app si le fil principal ne répond plus
  pendant 5 s avec l'écran figé montré ; la durée de la photo ne compte que la copie.
- Prouvé sur la machine Windows de GitHub : la photo couvre le bureau virtuel et elle est
  exacte au pixel près ; 200 photos sans fuite d'objets GDI (2 avant, 2 après) ; fenêtre
  figée sans activation, absente de la barre des tâches, cachée et montrée ; verrouillage de
  session suivi ; garde testée dans les deux sens.
- Reste ouvert : rien n'a tourné sur plusieurs écrans ni à une autre échelle que 100 % ; pas
  d'image de la capture précédente au réaffichage, à vérifier en vrai.

### Étape 5, la sélection de la zone

- Écrite par un agent, effort élevé (fenêtre transparente, focus, échelles). Relue par un
  agent distinct, effort élevé : 12 constats corrigés, puis quatre contre-vérifications,
  la dernière sans constat.
- Corrigé après relecture : la page ne peut plus rester bloquée (charge renvoyée, filet de
  1,5 s, toutes les touches neutralisées) ; plus aucun vol de focus par le système ; Alt+F4
  ne détruit pas la fenêtre ; une seule échelle, celle de la page, contrôlée par Rust ; trait net quand
  la largeur divisée par l'échelle n'est pas entière (2560 px à 150 %, 1366 px à 125 %) ;
  numéro de charge contre les réponses en retard après un redimensionnement.
- Trouvé par le premier essai sur Windows et corrigé : le pixel sous le curseur est inclus à
  l'appui comme au relâchement (un glisser de 412 à 611 donne 200 pixels ; la dernière
  colonne et la dernière ligne de l'écran peuvent être prises).
- Prouvé dans Chromium : zone dessinée égale à la zone découpée au pixel près, sans pixel
  intermédiaire, à 100, 110, 125, 133, 150, 175 et 200 %, tailles non entières comprises.
  Prouvé sur la machine Windows de GitHub : la sélection couvre le bureau, est juste
  au-dessus de l'écran figé, a le premier plan, n'est pas dans la barre des tâches ; Échap,
  clic droit et perte du focus annulent ; un clic ou un glisser de 1 pixel ne ferme rien ;
  Ctrl+P n'ouvre aucune fenêtre ; après F5, Échap ferme encore ; Alt+F4 reste sans effet
  (l'essai accepterait aussi une annulation propre) ; l'échelle retenue est celle de la page.
- Changement de plan : la loupe n'est pas construite dans la 0.1 (le réglage existe, sans
  effet).
- Reste ouvert : ce que le moteur web de Windows donne exactement comme position du curseur
  et comme largeur à une échelle fractionnaire (à voir sur un écran à 125 % ou 150 %) ;
  plusieurs écrans d'échelles différentes ; un vrai changement d'échelle pendant la sélection.

### Étape 6, la copie et l'enregistrement

- Écrite par un agent, effort moyen. Relue par un agent distinct, effort élevé : 14 constats
  corrigés, puis retouches contre-vérifiées.
- Corrigé après relecture : dossier vide ou relatif refusé ; vraie échéance de 250 ms pour
  le presse-papiers ; noms coupés à 200 octets ; copie faite en parallèle de l'écriture ;
  un fichier existant n'est jamais écrasé ; « Récents » limité à 50, en temps universel.
- Deux tests faux sur macOS corrigés (casse des noms de fichier, durée mesurée), un test
  instable sous Linux corrigé (le test seul était en cause).
- Prouvé sur la machine Windows de GitHub : image de 200×200 posée dans le presse-papiers
  et relue, chaque pixel à la couleur attendue ; même image dans « Récents » et dans le
  fichier enregistré, nommé d'après le modèle ; dossier impossible : « Impossible
  d'enregistrer », la copie est faite quand même.
- Non contrôlé : coller dans de vraies applications (conversation, Paint, document) ; un
  dossier réseau ou OneDrive ; un presse-papiers tenu par une autre application.

### Assemblage des étapes 4 à 6

- Écrit par un agent, effort élevé. Relu par un agent distinct, effort élevé : la relecture
  n'a trouvé aucun chemin qui laisse l'écran couvert ; 14 constats, 11 corrigés, puis deux contre-vérifications
  sans constat bloquant.
- Corrigé après relecture : une panique de la sortie donne un bilan d'échec et, sans bilan
  en 10 s, l'automate revient au Repos (il ne peut plus rester en Découpe) ; attente du compositeur bornée à
  100 ms ; une erreur de capture n'affiche que son titre, la cause va au journal ; rien
  n'est montré si la sélection n'existe pas.
- Trouvé par l'essai sur Windows et corrigé : pour le message du démarrage, la page ne
  renvoyait jamais sa zone et le message était caché d'office après 3 s (cause supposée : la
  boucle imbriquée de la création du moteur web de la sélection) ; il part maintenant de la
  boucle lancée, et l'essai le confirme.
- Prouvé sur la machine Windows de GitHub (`04-zone`, 92 points) : la séquence complète,
  dans trois sens du glisser (les quatre dans les tests unitaires) ; second appui ignoré pendant une capture ; deux captures
  de suite sans le bandeau de la première sur la seconde ; fenêtre au premier plan rendue ;
  pas de fuite de fenêtres ni de fils sur trois captures ; arrêt propre.
- Notés, non corrigés : un message de démarrage peut se montrer par-dessus une sélection
  lancée dans la première seconde ; un bilan arrivé après l'échéance de 10 s est ignoré
  (l'app annonce une erreur alors que la copie a pu réussir) ; un changement d'écran arrivé
  en retard peut annuler la capture suivante ; la page de la sélection pourrait se dire
  chargée avant que l'app soit prête à l'entendre (supposé ; l'essai `04-zone` le verrait).
- Ajouté à la fabrication : quand `main` est verte, le `.exe` essayé est publié en
  pré-version « v0.1.0-essai ».

## MVP, version 0.2 « Trois modes » : l'écran et la fenêtre

Les étapes 7 et 8 sont entrées ensemble dans `main` le 2026-11-12 par le commit `8429ab4`
(demande d'intégration n° 3, tête `36a5d44`, verte elle aussi). La version passe à
0.2.0 : la pré-version publiée devient « v0.2.0-essai », et les anciennes pré-versions
d'essai sont supprimées à chaque publication.

Vérifications GitHub sur `de1427b` (2026-11-12), toutes vertes :

- Windows : `cargo fmt`, `cargo clippy -D warnings`, les tests (323 et 2 tests
  d'architecture sous Linux ; le nombre sous Windows n'a pas été relevé), construction du
  `.exe`, puis essais sur le vrai `.exe` : `01-socle` 19 points, `02-reglages` 16,
  `03-raccourci` 40, `04-zone` 95, `05-plein-ecran` 41, `06-fenetre` 46, aucun échec.
- macOS : `cargo check` et `cargo test`.
- Pages : le bandeau et la sélection dans Chromium (sélection : 214 points, dont 55 pour
  le mode fenêtre).
- Machine de l'essai : un écran 1024×768 à 100 %, Windows build 26100 (coins arrondis).
- Mémoire privée au repos : 126,6 Mo.

### Étape 7, capturer l'écran

- Écrite par un agent, effort moyen. Relue par un agent distinct, effort moyen ;
  corrections contre-vérifiées par le même relecteur.
- Corrigé après relecture : une demande venue du menu est abandonnée si une autre capture
  a démarré pendant son attente ; l'enregistrement automatique ne vaut que si le dossier
  Images est connu ; deux points de l'essai qui pouvaient réussir sans rien prouver
  (second appui pendant une capture, bandeau absent de la photo suivante) sont devenus
  déterministes.
- Décidé en route : tant que la vignette n'existe pas, chaque capture est enregistrée dans
  Images › BreezyShot sans toucher au fichier des réglages (« Copié et enregistré ») ; une
  capture lancée depuis le menu de l'icône attend 350 ms que le menu s'efface ; au
  démarrage, une action sans raccourci est annoncée par son nom.
- Trouvé par l'essai sur Windows, erreurs de l'essai et non de l'app : une pilule de
  réussite n'écrit jamais de ligne `bandeau zone` ; un presse-papiers ouvert sans fenêtre
  ne bloque pas une autre application qui l'ouvre de la même façon (l'essai le tient
  maintenant au nom d'une fenêtre à lui).
- Prouvé sur la machine Windows de GitHub (`05-plein-ecran`) : `Ctrl + Maj + 2` donne
  l'écran sous la souris, 1024×768, dans le presse-papiers et en PNG, la fenêtre témoin à
  sa place ; ni écran figé ni sélection ne se montrent ; le bandeau de la capture
  précédente n'est pas sur la suivante ; un second appui pendant la capture est ignoré ;
  presse-papiers tenu par une autre application : « Impossible de copier », le fichier
  est enregistré quand même, et la capture suivante remarche.
- Non contrôlé : le choix de l'écran avec plusieurs écrans ; une capture lancée depuis le
  menu de l'icône (le délai et l'absence du menu sur l'image) ; l'absence de raccourci.

### Étape 8, capturer une fenêtre

- Trois parties écrites par des agents, effort moyen : la liste des fenêtres, les coins et
  le presse-papiers à deux versions (A), la sélection en mode fenêtre (B), l'assemblage.
  Relues par des agents distincts : A effort élevé, B effort moyen, assemblage effort
  élevé ; corrections de A et de B contre-vérifiées par leur relecteur.
- Corrigé après relecture de A : l'image du presse-papiers est préparée une seule fois et
  non à chaque nouvel essai ; chaque essai retente l'ouverture six fois à 5 ms, comme pour
  une zone ; les fenêtres à la fois en calque et transparentes aux clics sont écartées.
- Corrigé après relecture de B : le seuil entre clic et glisser se compte en pixels
  logiques (il était trop strict à 200 %) ; un appui resté en suspens ne fait plus perdre
  le clic suivant.
- Relecture de l'assemblage : aucun défaut de comportement trouvé ; l'Échap est libéré et
  l'écran découvert sur tous les chemins ; rien ne passe d'une capture à la suivante.
  Reprises faites : deux tests de décision, un ménage raté des pré-versions n'échoue plus
  une publication déjà faite, l'essai exige un rayon supérieur à 0 sur Windows 11.
- Trouvé par l'essai sur Windows, erreur de l'essai : deux variables qui ne différaient
  que par la casse.
- Décidé en route : en mode zone, un clic sans glisser ne capture pas la fenêtre sous la
  souris (le plan le prévoyait) ; la fenêtre a son propre mode, `Ctrl + Maj + 3`. Une
  fenêtre à cheval sur deux écrans est coupée à l'écran du clic.
- Prouvé sur la machine Windows de GitHub (`06-fenetre`) : la fenêtre choisie a exactement
  le vrai bord de la fenêtre témoin (242×272) ; PNG à coins transparents (rayon 8), centre
  et bords opaques, contenu à sa place ; presse-papiers en deux versions, le format PNG
  transparent et l'image à coins blancs ; glisser sans effet, clic droit et Échap
  annulent ; premier plan rendu, rien ne reste à l'écran, arrêt propre.
- Non contrôlé : que le rayon de 8 px corresponde au vrai rendu de Windows (il est lu au
  journal, pas comparé à la photo) ; le contour bleu lui-même ; coller dans Paint, un
  navigateur ou un document ; fenêtre agrandie, ancrée, sans cadre, à cheval sur deux
  écrans ; échelles autres que 100 % ; Windows 10 ; le clic sur le bureau (sauté par
  l'essai : aucun point libre de fenêtre garanti sur cette machine).
- Noté, non corrigé : comme la bibliothèque du presse-papiers, l'app ouvre le
  presse-papiers sans fenêtre ; une application qui l'a ouvert de la même façon ne la
  bloque pas, et les deux écritures peuvent se croiser.

## MVP, version 0.3 « Vignette, Réglages, installateur »

Les étapes 9 et 10 et l'installateur entrent ensemble dans `main` par la demande
d'intégration de la branche `claude/project-thread-tkz3qd` (dernier commit de code :
`87d1950` ; le commit suivant n'ajoute que cette section). La version passe à 0.3.0. Le fichier publié devient un
installateur : son nom décide du rôle (`breezyshot.exe` est l'app, tout autre nom installe,
`--desinstaller` désinstalle).

Vérifications GitHub sur `87d1950` (2026-11-12, passage 42), toutes vertes :

- Windows : `cargo fmt`, `cargo clippy -D warnings`, les tests (423 et 2 tests
  d'architecture), construction du `.exe`, puis essais sur le vrai `.exe` :
  `01-socle` 19 points, `02-reglages` 16, `03-raccourci` 40, `04-zone` 95,
  `05-plein-ecran` 41, `06-fenetre` 46, `07-vignette` 64, `08-reglages-fenetre` 61,
  `09-installateur` 138, aucun échec. Un point de `06-fenetre` est sauté et dit comme tel
  (clic sur le bureau : aucun point libre sur cet écran).
- macOS : `cargo check` et `cargo test`.
- Pages dans Chromium : bandeau 22 points, sélection 214, réglages 49, vignette 39,
  souffle 12, installateur 23.
- Machine de l'essai : un écran 1024×768 à 100 %, Windows build 26100, animations coupées
  par le système.
- Mémoire privée au repos : 185 Mo (126,6 en 0.2 : les pages de la vignette et du souffle
  sont créées au démarrage). Chrono de la sélection : 67 ms. Installation : 0,4 s.
  Fichier téléchargé : 10,8 Mo.
- Lenteurs de la machine de GitHub vues pendant la mise au point, sans défaut de l'app
  trouvé : WebView2 a mis plus de 15 s à charger la page de l'installateur, deux fois sur
  sept passages (la fenêtre se montre alors vide au bout de 2 s, comme prévu ; 1,9 s au
  plus lent du passage vert) ; le sélecteur de dossier de Windows s'ouvre en 0,7 à 13 s.

Chaque chantier a été écrit par un agent (effort moyen), sur un contrat écrit d'abord, puis
relu par un agent distinct (effort élevé) ; un quatrième relecteur (effort élevé) a relu
l'ensemble assemblé. Les reprises faites après les passages sous Windows (essais 07 à 09) et
le nouvel habillage des Réglages n'ont pas été relus par un agent distinct : ils l'ont été
par l'assembleur (l'habillage sur photos, comparé au design), et prouvés par les passages
suivants. Sept passages ont été nécessaires pour les essais 07 à 09 : toutes les corrections
portent sur les essais eux-mêmes, aucune sur le code de l'app ; aucun point n'a été retiré.

### Étape 9, la vignette

- La carte à cinq actions (copier, enregistrer, glisser, supprimer, fermer) et le souffle
  qui part de la zone capturée. Les captures ne sont plus enregistrées d'office : bouton
  « Enregistrer » de la vignette, ou réglage « Enregistrer automatiquement ».
- Corrigé après relecture : un « Enregistrer » arrivé après « Supprimer » ne recrée pas le
  fichier ; le bouton principal suit le réglage des gauchers ; une page qui redit prête
  ne relance rien ; l'arrêt attend 2 s les suppressions en cours ; le souffle suit le
  thème ; un point de l'essai qui dépendait du précédent est isolé.
- Trouvé par l'essai sur Windows : la page du souffle pouvait dire prête avant que
  l'Application ne gère la vignette, et son appel était perdu ; l'appel est maintenant
  refusé explicitement et retenté par la page. Un test du glisser utilisait un chemin qui
  n'est pas absolu sous Windows.
- Prouvé sur la machine Windows de GitHub (`07-vignette`) : les deux fenêtres créées
  cachées, pages prêtes ; le souffle visible puis caché, la carte au coin bas gauche de la
  zone utile à 12 px ; presse-papiers, un PNG dans « Récents », aucun dans Images, premier
  plan inchangé ; la carte ne prend les clics que sur elle ; départ à 3 s (3,043 s) et
  pause au survol ; Copier, Enregistrer (puis « déjà fait »), Fermer, Supprimer puis
  Annuler ; glisser : une fenêtre cible reçoit exactement un fichier, mêmes octets que
  « Récents », Échap et dépôt sur soi annulent ; après Supprimer, ce fichier, son
  enregistrement et son fichier de glisser sont effacés, aucun autre ; aucune fenêtre de
  BreezyShot sur la photo suivante ; un second appui pendant le souffle est ignoré ;
  fenêtres et fils stables après 10 vignettes (197,3 Mo) ; réglages : vignette coupée, coin
  haut droit, animations réduites.
- Non contrôlé : l'allure du souffle et de la carte ; le survol ; l'image de glisser
  (celle de Windows par défaut) ; le dépôt dans une vraie conversation ou un vrai
  document ; plusieurs écrans ; échelles autres que 100 % ; les animations (coupées sur
  cette machine).
- Noté, non corrigé : pas de verre flouté derrière la carte ; les deux fenêtres créées au
  démarrage coûtent environ 60 Mo au repos (les créer à la première capture les
  rendrait, au prix d'un délai sur la première vignette).

### Étape 10, la fenêtre Réglages

- Trois rubriques montrées sur les quatre du design : la rubrique Capture n'a aucun
  réglage qui ait un effet dans cette version (règle : un réglage sans effet n'est pas
  montré). La barre de capture du plan n'est pas construite (mode Zone épuré, choix de
  l'utilisateur). Fichier des réglages en version 2, la version 1 est reprise au
  démarrage.
- Corrigé après relecture : la relecture du fichier pendant que la fenêtre est ouverte
  est une décision pure (garder ou adopter) ; les refus s'effacent ; les codes d'erreur
  vont au journal sans chemin ; la fenêtre s'ouvre sur l'écran de la souris ; un seul
  choix de dossier à la fois ; trois points de l'essai qui pouvaient réussir sans rien
  prouver.
- Décidé en route : l'app refuse d'éteindre sa dernière sortie (« Gardez au moins la
  copie, l'enregistrement ou la vignette ») ; le démarrage avec Windows a pour vérité le
  registre, et une valeur qui pointe vers une autre copie de l'exe est réécrite au
  démarrage ; les raccourcis `Ctrl + Maj + 4`, `T` et `H` ne sont pas proposés.
- Trouvé par l'essai sur Windows, erreurs de l'essai : le menu de l'icône n'est pas
  visible par l'automatisation de l'interface (lu par l'API des menus) ; une recherche
  par nom trouvait le libellé et non l'interrupteur ; l'essai éteignait la dernière
  sortie, ce que l'app refuse à raison ; le bureau de la machine n'est pas figé entre
  deux photos (comparaison de la carte à moins de 2 % de points près) ; une console d'aide
  couvrait la ligne cliquée ; un clic parti pendant qu'une ligne se déplie tombait à côté
  (l'essai attend que l'élément ne bouge plus) ; le sélecteur de dossier ne prend pas un
  texte posé par message, le chemin est tapé au clavier comme le ferait une personne.
- Habillage : direction B « Épuré » du design, validée par l'utilisateur le 2026-11-12
  (onglets en haut, contenu centré, logo une fois dans Général, touches en relief), fenêtre
  de 680 × 512.
- Prouvé sur la machine Windows de GitHub (`08-reglages-fenetre`) : ouverture par le vrai
  menu de l'icône, une seule fenêtre ; la dernière sortie ne s'éteint pas (deux refus) ;
  copie éteinte, la capture ne touche pas le presse-papiers ; vignette au coin et à la durée
  du réglage, puis éteinte sans redémarrer ; raccourci changé au clavier, l'ancien rendu à
  Windows, le nouveau tenu par l'app ; un raccourci tenu par une autre application n'arrive
  pas à la saisie, Échap la ferme ; démarrage avec Windows écrit, relu dans le système et
  retiré, sur une clé d'essai, la vraie clé intacte, et une valeur qui visait une autre
  copie réécrite ; dossier tapé dans le sélecteur de Windows et validé, la capture y est, le chemin
  jamais au journal ; fichier changé de l'extérieur non écrasé ; thème clair (#FAFAFA) puis
  sombre (#1C1C1C), les fonds de l'habillage B ; mémoire après fermeture : 185,2 Mo.
- Non contrôlé : l'allure de la fenêtre dans le .exe (police de Windows, mouvement) ; le
  refus « Pris par une autre application » en vrai (Windows ne laisse pas taper un
  raccourci tenu ailleurs : tests purs seulement) ; le démarrage réel à l'ouverture de
  session et sa désactivation par le gestionnaire des tâches (l'essai écrit dans une clé
  à part) ; plusieurs écrans.

### L'installateur

- Un seul fichier : il se copie dans `%LOCALAPPDATA%\Programs\BreezyShot`, crée le
  raccourci du menu Démarrer et l'entrée « Applications installées », lance l'app, puis
  s'envole vers la zone de notification. Aucun réglage dedans. La désinstallation retire
  aussi le démarrage avec Windows, si l'exe est bien parti.
- Corrigé après relecture : la suite des étapes est une décision pure sous verrou ; une
  minuterie n'écrase plus une erreur ; la désinstallation retire l'exe d'abord, et le
  raccourci seulement s'il vise cet exe ; la fenêtre ne se montre que page prête ; les
  codes vont au journal sans chemin.
- Relecture de l'assemblage : aucun point bloquant. Reprises faites : le démarrage avec
  Windows est gardé si l'exe reste ; une copie lancée sans extension (Linux, macOS) est
  l'app et non l'installateur ; documents.
- Trouvé par l'essai sur Windows : la machine coupe les animations, l'envol devenait un
  fondu sans rien écrire (le journal dit maintenant lequel joue, et l'essai force l'envol
  par une variable réservée aux essais) ; le bouton de la boîte « WebView2 absent »
  n'est pas un bouton pour l'automatisation de l'interface (pressé par message Windows) ;
  l'essai attend maintenant « installateur pret » avant de chercher un bouton, et note le
  lancement le plus lent.
- Prouvé sur la machine Windows de GitHub (`09-installateur`) : installation, raccourci,
  inscription et lancement de l'app ; l'envol (forcé par l'essai) part de la place réelle de
  la fenêtre, et le fondu quand les animations sont coupées ; réinstallation et mise à jour
  par-dessus une app ouverte ; refus d'une version plus ancienne ; place insuffisante ;
  fermeture pendant l'installation ; une app qui ne se ferme pas n'est pas tuée ;
  désinstallation complète (exe, raccourci, inscription, démarrage avec Windows), réglages
  et captures gardés ; WebView2 absent : message, « OK », code 1, rien d'installé ; la vraie
  clé de désinstallation, le vrai dossier et la vraie clé Run intacts.
- Non contrôlé : SmartScreen et l'antivirus sur un fichier téléchargé (non signé) ;
  l'entrée dans « Applications installées » telle que Windows la montre, et la
  désinstallation lancée de là ; le menu Démarrer ; l'allure de l'envol et sa cible quand
  l'icône est rangée derrière « ^ » ; l'installation de WebView2 quand il manque
  vraiment ; une mise à jour par-dessus une version qui tourne sur un vrai poste.
- Noté, non corrigé : le cache WebView2 de l'app
  (`%LOCALAPPDATA%\dev.breezyshot.desktop\EBWebView`) reste après la désinstallation.
