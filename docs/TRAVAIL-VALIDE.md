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

## MVP, version 0.4 « Confort et éditeur d'annotation »

La 0.4 entre dans `main` par la demande d'intégration de la branche
`claude/project-thread-tkz3qd` (dernier commit de code : `7fbaec6` ; le commit suivant
n'ajoute que cette section). La version passe à 0.4.0.

Ce qu'elle apporte : refaire la dernière zone (Ctrl + Maj + 4), le retardateur (3, 5 ou
10 s), le pointeur dans l'image, et l'éditeur d'annotation ouvert par « Annoter » sur la
vignette. La loupe de la sélection, construite puis retirée à la demande de l'utilisateur
(12/11/2026), n'existe plus nulle part.

Vérifications GitHub sur `7fbaec6` (2026-11-12, passage 56), toutes vertes :

- Windows : `cargo fmt`, `cargo clippy -D warnings`, les tests (521 et 2 tests
  d'architecture, 2 ignorés), construction du `.exe`, puis essais sur le vrai `.exe` :
  `01-socle` 19 points, `02-reglages` 18, `03-raccourci` 40, `04-zone` 95,
  `05-plein-ecran` 41, `06-fenetre` 46, `07-vignette` 64, `08-reglages-fenetre` 76,
  `09-installateur` 138, `10-confort` 47, `11-pointeur` 14, `12-editeur` 42, aucun échec.
  Un point de `06-fenetre` est sauté et dit comme tel (clic sur le bureau : aucun point
  libre sur cet écran).
- macOS : `cargo check` et `cargo test`.
- Pages dans Chromium : bandeau 22 points, décompte 24, sélection 214, réglages 70,
  vignette et souffle 55, installateur 23, modèle et dessin de l'éditeur 53, page de
  l'éditeur 145.
- Machine de l'essai : un écran 1024×768 à 100 %, animations coupées par le système.
- Mémoire privée au repos : 172 à 184 Mo selon l'essai (185 en 0.3). Chrono de la
  sélection : 71 ms. Installation : 0,4 s. Fichier téléchargé : 11,2 Mo (10,8 en 0.3).

Chaque chantier a été écrit par un agent (effort moyen) sur un contrat écrit d'abord, puis
relu par un agent distinct (effort élevé) : les quatre chantiers du confort et leur
assemblage, le retrait de la loupe, puis pour l'éditeur le socle, les sorties natives, le
modèle et le dessin, le cœur (deux relectures), la page (deux relectures : la première sur
l'ancien design, la seconde sur la page refaite), et enfin les corrections faites d'après
les passages sous Windows (voir « Trouvé sur Windows »). N'ont PAS été relus par un agent
distinct : les dernières reprises de la page et du cœur faites après leur seconde
relecture (contrôlées par l'assembleur, pour la page en rejouant les scénarios du
relecteur), la correction du clic sur la fenêtre Réglages inactive, l'assemblage final de
l'éditeur, et les reprises faites à la demande du dernier relecteur (bornes du navigateur
et du GPU, journaux rangés par le lanceur, témoin de l'ombre dans l'essai de la page).

### Confort : dernière zone, retardateur, pointeur

- `Ctrl + Maj + 4` (et le menu de l'icône) refait la dernière zone sans rien tracer ; sans
  zone depuis le lancement, ou si l'écran a changé, la sélection s'ouvre.
- Retardateur de 3, 5 ou 10 s (Réglages › Capture) : un disque de décompte au centre de la
  zone utile, caché avant la photo ; Échap ou la croix l'annulent.
- « Inclure le pointeur dans l'image » (Réglages › Capture).
- Prouvé sur la machine Windows de GitHub (`10-confort`, `11-pointeur`) : la dernière zone
  rend le même PNG que le tracé au clavier ; le décompte dure entre 2,95 et 3,6 s, se cache
  avant la photo, s'annule par Échap, par la croix et à l'arrêt ; le pointeur est dans
  l'image quand le réglage est allumé, absent sinon, et les fenêtres restent stables.
- Corrigé en chemin, défaut présent dans la 0.3 publiée : le premier clic sur un raccourci
  dans une fenêtre Réglages inactive n'ouvrait pas la saisie (la page se redessinait entre
  l'appui et le relâcher).
- Non contrôlé : plusieurs écrans, échelles autres que 100 %, pointeur agrandi ou animé,
  un vrai verrouillage de session pendant un décompte.

### Éditeur d'annotation

- Design : « Éditeur épuré », piste B « Fenêtre », choisi par l'utilisateur le 12/11/2026
  (il remplace la piste A v2, sur laquelle la page avait d'abord été construite). Fenêtre
  sans cadre Windows dont la barre du haut porte Fermer, Enregistrer et Copier ; palette du
  bas : Flèche, Stylo, Surligneur, Gomme, puis par « + » Texte, Forme, Numéro, Flou,
  Projecteur ; recadrage par les coins de l'image ; « … » : Enregistrer sous, Glisser vers
  une appli.
- Absents, pas grisés : Copier le texte, Fond, Épingler (feuille de route).
- L'image va à la page et en revient en RGBA brut ; le même code dessine l'écran et
  l'export ; tout ce qui quitte l'app passe par `sortie::sortir`.
- Corrigé après relecture, entre autres : le filet de fermeture ne détruit plus un
  document modifié quand la page ne répond pas ; un export accepté reçoit toujours un seul
  résultat ; Flouter ne laisse plus passer l'image d'origine sur une image avec alpha ;
  Pixeliser reçoit un grain tiré au hasard ; l'historique est borné en mémoire ; après
  « Copier et fermer », la page ne referme plus la capture suivante ; le verrou d'export
  est levé quand la capture change ; le Projecteur se prend par son bord.
- Limite connue : en intensité Légère, un attaquant qui connaît la formule retrouve encore
  une partie d'un petit texte pixelisé (1,9 chiffre sur 8 en moyenne). Pour cacher un
  secret, l'outil sûr est Bloc noir.
- Écarts voulus au design : Bloc noir est un rectangle plein ; les poignées de
  redimensionnement de l'objet sélectionné sont gardées (la maquette n'en a pas) ; pas de
  message « Déposé » ; le zoom se fait au clavier et à la molette, sans bouton.
- Noté, non corrigé : si l'éditeur ne peut pas s'ouvrir, la capture reste dans
  « Récents » et le presse-papiers seulement (bandeau « Impossible d'ouvrir l'éditeur ») ;
  la gomme glissée très vite peut sauter un trait fin ; un tracé commencé à moins de 22 px
  d'un coin de l'image devient un recadrage.
- Prouvé sur la machine Windows de GitHub (`12-editeur`, clics et touches réels sur le vrai
  .exe) : « Annoter » ouvre la fenêtre au premier plan, dans la barre des tâches, sans
  barre de titre et redimensionnable ; un rectangle tracé puis copié laisse l'image égale
  à l'original au bit près hors du trait, et le trait est au rouge exact ; le Flou rend
  des blocs unis ; annuler et rétablir rendent les images attendues ; « Enregistrer »
  écrit un PNG égal au presse-papiers, une seule fois ; fermeture propre, fermeture d'un
  document modifié (menu, Continuer, Fermer sans copier) ; nouvelle capture pendant
  l'édition (remplacement, abandon) ; dialogue « Enregistrer sous » ouvert et annulé ;
  glisser réel depuis l'éditeur vers une fenêtre cible ; arrêt de l'app éditeur ouvert.
- Mesuré au passage 56 : du clic sur Annoter à l'image affichée 284 ms
  (budget 800 ; 182 à 312 ms d'un passage à l'autre), page prête en 217 ms,
  de Ctrl+C au bilan 73 ms (budget 400). Éditeur ouvert : 229,6 Mo pour
  172 au repos, soit environ 58 Mo de plus (un processus de rendu de plus, parti à
  la fermeture).
- Non contrôlé, à voir par l'utilisateur : l'allure de la fenêtre sans cadre (ombre, coins
  arrondis), son déplacement par la barre, son redimensionnement par les bords et le
  double-clic qui l'agrandit ; plusieurs écrans ; échelles de 125, 150 et 200 % ; captures
  de trois écrans 1080p et de 2K ; « Enregistrer sous » jusqu'au bout ; le glisser vers
  l'Explorateur, Word ou Teams ; le sélecteur « Autre couleur » de Windows.

### Trouvé sur Windows, corrigé

Aucun de ces défauts n'était visible hors de Windows ; tous ont été trouvés par l'essai
`12-editeur` sur la machine de GitHub.

- « Annoter » bloquait l'app pour toujours. La fenêtre de l'éditeur était créée dans la
  commande de la page de la vignette ; sous Windows, créer une fenêtre Web dans le rappel
  d'une autre vue Web bloque le fil principal. Le document est maintenant reçu à un tour
  suivant de la boucle (`annoter`, `application/mod.rs`). Le relecteur a vérifié qu'aucune
  autre commande d'une page ne crée de fenêtre, et que la destruction d'une fenêtre, elle,
  est postée à la boucle.
- L'essai arrêtait la machine de GitHub. Quand un point échouait, la suite envoyait encore
  Ctrl+C, Alt+F4 et des clics à la fenêtre du premier plan, qui n'était plus celle de
  l'app : trois passages sont restés sans journal. L'essai n'envoie plus aucune touche ni
  aucun appui de souris hors des fenêtres de l'app, et s'arrête si l'éditeur ne s'ouvre
  pas. Le lanceur arrête un essai pas fini en 600 s.
- La page disait la place de l'image dans sa toile, pas dans la page (52 px trop haut) :
  la ligne `editeur vue` était fausse, sans effet sur ce que voit l'utilisateur. Corrigé,
  et vérifié dans l'essai de la page aux échelles 1, 1,25, 1,5 et 2.
- Budget de mémoire après fermeture, changé et dit ici. Le contrat demandait « app et
  WebView2 ≤ repos + 8 Mo après 10 fermetures » ; mesuré : 18,1 puis 8,4 Mo. La mesure par
  processus montre que ce seuil ne mesurait pas l'éditeur : son processus de rendu part à
  chaque fermeture (4 au repos, 5 éditeur ouvert, 4 après une fermeture comme après vingt), le processus de l'app prend 3,4 Mo en 19 ouvertures
  de plus (une image gardée par ouverture en ferait 60), et ce sont les quatre pages
  permanentes (bandeau, vignette, souffle, Réglages) qui grossissent avec les captures :
  88,8 Mo après une capture, 99,9 après vingt, 94,8 après quarante de plus sans
  éditeur. La hausse s'arrête donc ; tout l'arbre est à 197,2 Mo après vingt éditeurs et
  188 après quarante captures de plus. L'essai vérifie maintenant ce que l'éditeur
  laisse : nombre de processus de rendu revenu à celui du repos, processus de l'app
  ≤ + 8 Mo, navigateur et GPU ≤ + 6 Mo chacun, et un filet de + 40 Mo sur l'ensemble.
- Poignées : 394 → 413 en vingt ouvertures, environ une par ouverture. Sous le seuil de
  l'essai, mais à suivre en 0.5.

Relecture distincte de ces corrections (effort élevé), sur le code et les essais, sans
machine Windows : le report de l'ouverture est juste et c'est le seul chemin à risque ; la
garde de l'essai tient ; la place de l'image est juste aux quatre échelles ; les deux
contrôles principaux de la mémoire prouvent quelque chose. Elle n'a pas relu les essais
01 à 11, le reste de `commun.ps1`, `application/editeur.rs`, ni le comportement de la
bibliothèque de fenêtres après l'arrêt, et n'a joué aucun script sur Windows.

Noté, non corrigé :
- Une bulle d'aide de WebView2 est une fenêtre à part : si elle se trouvait sous la souris,
  la garde de l'essai refuserait un clic (essai rouge à tort, jamais l'inverse).
- Si l'app quitte entre le clic sur Annoter et le tour suivant de la boucle, le document
  est perdu avec elle (la capture reste dans « Récents »).
- Trois essais bloqués de suite (600 s chacun) dépasseraient les 30 minutes de l'étape.
- Taille du fichier téléchargé : 11,2 Mo, à la limite de son budget.

## MVP, version 0.5 « Historique et Copier le texte »

La 0.5 est sur la branche `version-0.5` (derniers commits de l'app : `d83e28b` pour le cœur,
`6582dac` pour les pages). La version passe à 0.5.0. Contrat :
`contrat-0.5-historique-texte.md`, avec ses avenants « Droits » et « Licence ».

Ce qu'elle apporte, dans une seule app :

- L'historique (Ctrl + Maj + H, ou le menu de l'icône) : les 50 captures de « Récents » en
  grille, avec Copier, Enregistrer, Annoter, Supprimer (Annuler pendant 5 s), glisser et
  Tout effacer.
- « Copier le texte » (Ctrl + Maj + T, le menu, ou le bouton « Texte » de l'éditeur) : le
  texte d'une zone va au presse-papiers, lu par le moteur de Windows dans les langues
  installées. Dans l'éditeur, on choisit les mots à copier.
- La licence, collée dans Réglages › Général, ouvre les deux fonctions, sans redémarrer.
  Sans licence, l'app se comporte comme la 0.4, plus la ligne « Licence » (ni aperçu, ni
  recherche des langues). Licence d'essai publique : `essais/licence-essai.txt`.

Vérifications GitHub :

- Tests : 641 tests du cœur et 2 tests d'architecture (2 ignorés) ; dix essais de pages,
  `ECHECS : 0` ; essais Windows 01 à 13 sur le vrai `.exe`.
- Passage 65 : 10 essais Windows sur 13 réussis (voir « Trouvé sur Windows, corrigé »).
- Passage 66 (`688d7b1`) : tests verts sur Windows, macOS et Linux, 13 essais Windows sur 13.
- Les corrections de la seconde relecture (`d83e28b`, `6582dac`) sont venues après le
  passage 66. Au passage suivant (`67a3095`), avec elles : tests verts sur les trois
  systèmes, 12 essais Windows sur 13 (un clic de l'essai 13 parti trop tôt, corrigé dans
  l'essai). C'est le dernier passage de la demande de fusion qui fait foi.

Chaque chantier a été écrit par un agent sur un contrat écrit d'abord, puis relu par un
agent distinct de l'auteur (effort élevé) : L (licence) et B (données de l'historique)
ensemble, puis A (moteur de texte), C (réglages), D (cœur de l'historique), E (cœur du
texte), F (page de l'historique), G (pages du texte) et H (essai 13). Un autre agent
distinct (effort élevé) a relu l'assemblage. Un dernier agent distinct (effort élevé) a relu
les corrections des pages du texte (`63bfea5`) et celles de l'assemblage (`7eaec83`), que
personne n'avait relues.

### Trouvé par les relectures, corrigé

- F, bloquant : Ctrl + Z et Ctrl + C étaient lus sur `e.code` ; sur un clavier AZERTY,
  Ctrl + Z ne faisait rien et Ctrl + W annulait. Ils lisent `e.key`, et l'essai de la page
  le prouve. Aussi : un aperçu qui pouvait ne jamais s'afficher, le focus perdu après
  « Effacer ».
- A : `RoUninitialize` pouvait défaire le MTA alors que la crate `windows` garde ses
  fabriques WinRT en cache. Le MTA est tenu une fois pour tout le processus
  (`CoIncrementMTAUsage`). Le `Debug` dérivé aurait écrit le texte lu au journal : il est
  écrit à la main. Test Windows ajouté : `windows_lit_deux_fois_une_image_blanche`.
- E : une lecture échue (plus de 10 s) copiait quand même plus tard, sans message. Un seul
  des deux fils rend le résultat (`ATTENDUE`). Un fil impossible à lancer laissait l'état en
  Lecture pour toujours. L'annonce propre au texte est retirée : une seule annonce au
  démarrage, pour toutes les actions.
- D : Annoter n'était pas limité (Entrée tenue lançait un fil par répétition) ; son repli
  sur le fil principal aurait bloqué l'app, comme le défaut d'« Annoter » de la 0.4. Plus
  aucune action n'est faite sur le fil principal. Une case effacée ne revient plus.
- G : quand le focus quittait le calque « Choisir le texte », Échap fermait l'éditeur et
  Ctrl + C copiait l'image. Le clavier est pris par le calque tant qu'il est ouvert. Aussi :
  un export qui pouvait rester bloqué, un anneau de focus absent.
- L : un retrait qui échouait laissait `licence.txt`, et la licence revenait au démarrage
  suivant ; un collage précédé de beaucoup de blancs était tronqué à la relecture ; un
  fichier illisible était dit absent.
- B : `lire_recent` pouvait allouer 1 Gio avant de juger l'en-tête (plafond de 512 Mio,
  en-tête jugé d'abord) ; une liste partielle faisait effacer des aperçus valides ;
  Supprimer pouvait viser un autre fichier sur un disque sans casse.
- C : une saisie en cours sur une ligne retirée avec la licence suspendait tous les
  raccourcis 15 s. `Cmd` et `Win`, `Option` et `Alt` comptent pour la même touche. La ligne
  « Licence » est la dernière de Général (décision du responsable).
- H : Échap du menu de l'icône était refusé par la garde de premier plan de l'essai ;
  « Réessayer » était cherché trop tard ; trois preuves creuses sont élargies ou retirées.
- Assemblage : une lecture abandonnée parce que les écrans changeaient pouvait avoir déjà
  copié, sans rien dire. Seul l'écran figé est libéré, la lecture finit normalement. Sans
  licence, l'app faisait des aperçus et cherchait les langues : plus maintenant. Le journal
  de la licence écrivait un chemin : il n'écrit plus que le genre d'erreur.

- Seconde relecture (`63bfea5` et `7eaec83`), rien de bloquant. Une régression de `63bfea5` :
  le calque « Choisir le texte » gardait le clavier quand le menu « Fermer » était ouvert
  par la croix de la barre des tâches, Alt + F4 ou une capture en attente (Entrée copiait
  le texte au lieu de l'image). Toute bulle ou tout menu qui s'ouvre ferme d'abord le
  calque. Aussi : Alt + F4 n'est plus annulé pendant le calque ; Espace active de nouveau
  les boutons de l'éditeur (défaut de la 0.4) ; anneau de focus sur « Copier » et les
  outils ; la ligne `texte ecran libere: bureau change` n'est écrite que si l'écran a été
  libéré ; une capture faite avec l'historique encore ouvert après le retrait de la licence
  a son aperçu (`6582dac`, `d83e28b`).

### Non vérifié

- Les corrections faites après les relectures de chantier n'ont été relues que par le
  relecteur de l'assemblage. Celles de la seconde relecture (`6582dac`, `d83e28b`) et
  celles des essais (`3d2dc0a`, `3b03c43`) n'ont pas été relues par un second agent.
- Les tests de `7eaec83` et de `d83e28b` prouvent les règles pures, pas tout leur câblage
  (`bureau_change` demande une vraie app).
- Le français et les accents ne sont pas prouvés sur la machine GitHub : le témoin est en
  anglais ASCII.
- Un texte de 12 px est mal lu, même agrandi deux fois (« Hello areezyShct 2026, … »).
- Le premier plan réel, le glisser vers une vraie app, un PC à un écran 2K et un PC à trois
  écrans : à l'utilisateur, d'après la liste unique des tests à faire.

### Limites acceptées

- Trois copies de l'image dans le chemin de lecture : environ 100 Mo de pointe en 4K.
- Le ménage à 50 compte les captures en attente de suppression.
- « Déjà fait » est oublié à la fermeture de la fenêtre de l'historique.
- Après Annuler, la vignette fermée par Supprimer ne revient pas.
- Course théorique de `liberer_ecran`, documentée : un fil suspendu au mauvais moment
  cacherait l'écran figé de la capture suivante.
- L'export de but `texte` d'un éditeur déjà ouvert n'est pas revérifié après le retrait de
  la licence.
- Le test des liens symboliques ne prouve rien hors Unix.

### Trouvé sur Windows, corrigé

- Trois tests de `sortie` échouaient sur Windows et macOS (passage 63) : le fichier
  étranger `…123.PNG` était le même fichier que la capture `…123.png` sur un disque sans
  casse. Défaut du test (`1f051d9`).
- Premier passage des essais (passage 65) : 10 essais sur 13 réussis. L'essai 02 attendait
  la version 3 des réglages. L'essai 07 écrivait un fichier en version 3, que l'app met à
  niveau et réécrit : son point « réglages non réécrits » échouait. Le point (17) de
  l'essai 13 comptait toutes les suppressions en attente, et non celles du motif attendu.
  Trois défauts des essais, aucun de l'app (`3b03c43`).
- Passage de la demande de fusion (`67a3095`) : 12 essais sur 13. Le point (5) de l'essai 13
  cliquait « Réessayer » 40 ms après l'arrivée de la souris, alors que le bandeau ne reçoit
  les clics qu'une fois la souris vue sur sa pilule (surveillance toutes les 30 à 60 ms) :
  le clic était refusé par la garde de l'essai, une fois sur trois passages. L'essai pose
  la souris, attend que le point soit sur une fenêtre de l'app, puis clique. Défaut de
  l'essai, pas de l'app.

### Mesures sur la machine Windows de GitHub (passage 65)

- Repos sans licence : 174,1 Mo (app 6,8 Mo). Repos avec licence : 190,3 Mo.
- Copier le texte : du relâcher à l'écran figé caché 230 ms ; lecture des trois lignes du
  témoin 225 ms (budget 400) ; du relâcher à « Texte copié » 244 ms ; moteur créé en 0 ms ;
  « Aucun texte trouvé » affiché 4,14 s ; écran entier 534 ms (budget 1 000).
- Dix lectures : app 11,6 Mo avant, 12,6 Mo après.
- Historique : de Ctrl + Maj + H à l'historique affiché 188 ms (budget 700), page prête en
  137 ms ; 20 ouvertures entre 185 et 209 ms. Ouvert : 226,3 Mo ; 186,3 Mo dix secondes
  après 20 fermetures. Fils 22 puis 22 ; poignées 396 puis 415.
- Lecture d'une zone dans l'éditeur : 11 ms. Trois aperçus manquants faits en 24 ms.
- Copier, Enregistrer et glisser depuis l'historique : exacts au pixel.

## Après la 0.5 : une annotation plus naturelle (version 0.5.4)

Branche `claude/project-thread-9qu6yx`, la version passe à 0.5.4 (la 0.5.2 est celle du
menu de l'icône, la 0.5.3 celle des Réglages en volet). Demandé par l'utilisateur le
13/11 : que la gomme n'efface que là où elle passe, que tout ce qui est annoté se comporte
de façon logique pour un humain, et que Copier le texte montre ce qu'il copie. Choix du
même jour, sur des pages à essayer : barre C (`design/annotation-pistes`), instruments B
(`design/annotation-instruments`) avec une flèche qu'on courbe directement, « Surligné »
pour Copier le texte (`design/copier-texte-animation`). Ces pages sont dans les fichiers
du projet, pas dans le dépôt, comme celles des versions précédentes.

Ce qu'elle apporte :

- La gomme efface les encres (flèche, ligne, rectangle, ellipse, stylo, surligneur) là où
  elle passe, et là seulement ; posée sur un texte, un numéro, un flou ou un projecteur
  qu'aucune encre ne recouvre, elle l'enlève en entier : ce qui est dessus décide. Un geste
  = un pas d'annulation, aucun s'il n'efface rien ; une encre tout effacée disparaît ; Échap
  abandonne le geste. Vue et export au bit près.
- La barre d'annotation : l'outil en main s'y déplie (variantes, trois tailles, six
  couleurs et « Autre couleur », opacité) ; plus de menu « Ajouter » ni de bulle par-dessus
  l'image. Flèche, stylo, surligneur et gomme sont dessinés, le corps à la couleur de
  l'encre. Un objet choisi déplie l'outil qui l'a fait ; ce qu'on y règle va à lui ; une
  variante prise dans son pli le change sur place, alors que la touche d'une variante
  (« L » : Ligne) la prend pour le tracé suivant et défait la sélection. Un texte en saisie
  déplie l'outil Texte.
- L'éditeur s'ouvre assez large pour que la barre dépliée tienne sans défiler (1088 px
  logiques quand l'écran le permet). Plus étroit, seuls les outils défilent ; Annuler,
  Rétablir et Plus restent en entier.
- Une flèche tracée reste choisie : sa poignée du milieu la courbe, le trait passe par la
  poignée ; ramenée près de la corde, elle redevient droite ; Maj la garde symétrique.
- Un objet emmené puis ramené à sa place, ou une glissière ramenée à sa valeur, ne laisse
  aucun pas d'annulation, et le document n'est pas marqué modifié pour autant.
- Copier le texte : chaque ligne copiée se surligne de gauche à droite, puis la sélection
  s'efface et « Texte copié » s'affiche ; l'écran figé reste au plus 810 ms de plus. Sur un
  texte penché, les boîtes des mots sont ramenées sur l'image.
- La fenêtre de l'éditeur garde 72 × 180 px autour de l'image (barre de 80 px) : l'image
  tient encore à la taille réelle.

Vérifications, ici sous Linux :

- Sur l'arbre fusionné avec main (menu de l'icône, puis Réglages en volet) :
  `cargo test --locked` : 669 tests du cœur réussis, 2 ignorés, et les 2 tests
  d'architecture. `cargo fmt --check` et
  `cargo clippy --locked --all-targets -- -D warnings` propres, sous Linux et pour la
  cible Windows (`x86_64-pc-windows-msvc`, vérifiée sans édition de liens : le code de
  `TextAngle` compile, il n'a pas tourné).
- Les onze essais de pages, `ECHECS : 0` : `editeur.cjs` 243 points, `editeur-modele.cjs`
  69, `selection.cjs` 232, `historique.cjs` 122, `reglages.cjs` 119, `vignette.cjs` 43,
  `bandeau.cjs` 25, `decompte.cjs` 24, `installateur.cjs` 23, `menu.cjs` 20,
  `souffle.cjs` 12 ; `jetons.css` à jour.
- Essais par mutation : sans le filet de 2 s, sans le plafond de 400 lignes ou sans la
  borne de 2 s du surlignage, un essai de la page de sélection échoue ; avec l'ancien appui
  de la gomme (5 bis), l'ancien sondage en travers (G3) ou sans `h.avant = null` dans
  `annuler` (G6), un essai de l'éditeur échoue.
- Passage GitHub de la demande de fusion avant les Réglages en volet : vert, dont 685
  tests sous Windows et les 13 essais Windows sur le vrai `.exe` (l'essai 13 corrigé
  compris). Le passage après la refusion fait foi.

Relecture : deux agents distincts de l'auteur (Fable 5.2, effort élevé) ont relu le travail
avant les corrections. L'un a relu l'éditeur : `document.js`, `outils.js`, `barre.js`,
`dessin.js` en entier, les parties touchées de la page et de `editeur.rs`, avec des scripts
Playwright jetables en clair et en sombre, aux échelles 1 et 2, animations réduites
comprises. L'autre a relu Copier le texte : les fils et les verrous déroulés à la main
(borne pendant le surlignage, réveil tardif, échec d'émission, écrans changés, arrêt), les
durées du cœur et de la page, la géométrie, la confidentialité, et des essais par mutation.

Les corrections ont ensuite été relues par un troisième agent distinct (Fable 5.2, effort
élevé) : le diff entier et le code autour (`document.js`, `outils.js`, `barre.js`,
`dessin.js`, `editeur.css`, `selection.js`, `geste_texte.rs`, `application/texte.rs`,
`fenetres/editeur.rs`, `fenetres/selection.rs`, `systeme/texte/`), tous les essais lancés
(cœur, clippy sous Linux et pour Windows, fmt, jetons, dix essais de pages), 23 mutations
posées, dont 22 prises, des scripts jetables (pile de trois objets sous la gomme, flèches
extrêmes, largeur de la barre par outil et par échelle, numéros de version tirés au
hasard, surligneur gommé) et le journal du passage GitHub Windows.

### Trouvé par les relectures, corrigé

- Éditeur, important : la barre dépliée (jusqu'à 1 071 px avec Forme) ne tenait pas dans
  la fenêtre d'une petite capture (640 px au moins) : à 640 px, la plupart des outils et
  « Plus » étaient hors champ. L'éditeur s'ouvre à 1088 px de large au moins quand
  l'écran le permet (`LARGEUR_OUVERTURE`, jeton et essai d'égalité) ; plus étroit, seuls
  les outils défilent, et le fondu n'efface plus le fond ni l'ombre de la barre.
- Éditeur, important : la gomme pressée sur un numéro ou un texte posé sur une encre
  trouait l'encre cachée dessous et laissait l'objet. L'objet du dessus décide ; un clic
  qui tremble n'entame pas ce qu'il couvrait.
- Éditeur, important : un texte ouvert par double clic avec un autre outil en main
  montrait le pli de cet outil, qui réglait le texte et la préférence du texte à tort. Le
  pli montré est celui du texte.
- Éditeur, mineurs : un surligneur gommé dans sa longueur ne pouvait plus être pris (l'axe
  est sondé en travers) ; le trait courbé passait à 6 à 8 px de la poignée (le corps suit
  la courbe jusqu'à la tête, posée sur elle) ; le focus tombait sur la page quand un pli
  se refermait (il revient au bouton de l'outil) ; une touche de variante avec un objet
  choisi faisait diverger la barre et l'outil (elle défait la sélection et prend la
  variante pour le tracé suivant ; le bouton du pli, lui, change l'objet) ; un
  geste sans pas marquait le document modifié, et la fermeture redemandait « Copier et
  fermer » après un Copier (la version d'avant le geste revient, un numéro de version
  n'est jamais repris pour un autre contenu) ; courbes dégénérées (poignée bornée à la
  moitié du milieu de la corde, aimant mesuré à la corde, tête calculée sur la longueur
  de la courbe) ; une glissière aller-retour laissait un pas vide ; l'ombre des
  instruments avait des mesures en dur (jetons).
- Copier le texte, mineurs : le filet du balayage, le plafond de 400 lignes, Échap et les
  clics pendant le surlignage n'avaient pas d'essai (une mutation passait) : essais
  ajoutés ; côté cœur, `surlignable` et `fin_du_balayage` sont des fonctions pures
  testées. La page bornait chaque durée à 2 s, pas leur somme (un surlignage de
  13 minutes était accepté) : tout le surlignage tient en 2 s, ou rien. Les durées et les
  débords étaient recopiés sans jeton : jetons `texte-lecture-reduite` et
  `texte-surligne-*`, essai d'égalité du cœur. Le journal disait « sans affichage » quand
  il n'y avait aucune ligne : `selection surlignage sans ligne`. Le filet de la page
  tombait juste à la borne de 1,5 s, et un surlignage commencé à la borne pouvait être
  ignoré : il garde 0,5 s de marge. Commentaire de `LectureEnCours.fin` mis à jour.
- Copier le texte, signalé sans pouvoir le vérifier (pas de Windows) : `OcrResult.TextAngle`
  n'était pas lu, et les bandes d'un texte penché auraient été décalées. D'après la
  documentation de Windows, les boîtes sont celles du texte redressé, à tourner de l'angle
  dans le sens des aiguilles d'une montre autour du centre de l'image : `redresser` le
  fait, testé sur des boîtes calculées à la main.

### Trouvé par la relecture des corrections, corrigé

- Bloquant : le passage GitHub Windows était rouge depuis la barre C. L'étape (14) de
  l'essai 13 cliquait encore « Ajouter » puis « Flou », que la barre C a retirés. L'outil
  Flou se prend maintenant à la touche B, car dans une fenêtre étroite son bouton peut être
  hors de la partie visible des outils, puis « Bloc noir » ; l'Échap qui suivait est
  retiré (sans sélection ni menu, il fermerait l'éditeur). Mesuré dans Chromium à 921 px,
  la largeur de la fenêtre sur la machine GitHub : Flou déplié, la barre tient sans
  défiler et le centre de « Bloc noir » est bien ce bouton, quel que soit l'outil en main
  avant.
- Important : la documentation disait le contraire du code sur la touche de variante.
  Elle suit le code : le bouton du pli change l'objet choisi ; la touche prend la variante
  pour le tracé suivant et défait la sélection.
- Mineurs : la gomme posée sur une pile (un trait de stylo sur un numéro posé sur une
  flèche) trouait encore la flèche cachée ; à l'appui, seules les encres par-dessus
  l'objet entier sont gommées (essai 5 bis). Un surligneur Moyen (24 px) gommé au milieu
  par la gomme Moyenne (20 px) ne se prenait plus par ses 2 px de bord ; l'encre est
  sondée en travers jusqu'à 0,95 demi-largeur (G3). Aucun essai ne tenait les numéros de
  version après annuler puis abandonner (G6 ajouté). Le compte de `editeur.cjs` incluait
  la ligne finale.

### Limites acceptées

- La gomme traînée sur un texte ou un numéro posé sur une encre troue l'encre cachée
  dessous : invisible tant que l'objet la couvre, le trou se voit si on le déplace. Seul
  l'appui la protège.
- Les passages de gomme ne sont pas simplifiés, et le rendu gardé d'une encre gommée a pour
  clé un `JSON.stringify` de l'objet à chaque peinture : 17 ms par geste sur une charge
  synthétique (30 traits de 6 000 points), loin d'un usage réel.
- Le texte est dans le presse-papiers dès le début du surlignage, mais la fenêtre de
  sélection garde le clavier jusqu'à sa fin : un Ctrl + V tapé pendant ces 0,5 à 0,8 s est
  perdu, et un second Ctrl + Maj + T est ignoré.
- Sur un texte penché, la bande d'une ligne est le rectangle droit qui couvre ses mots
  tournés : plus haute que la ligne.

### Non vérifié

- Rien n'a été essayé sur un vrai Windows par une personne : gomme, barre, flèche courbée,
  largeur d'ouverture, surlignage de Copier le texte, à l'utilisateur d'après la liste
  unique des tests à faire.
- Aucun essai Windows ne lit un texte penché : le sens de `TextAngle` suit la
  documentation.
- La fin de l'étape (14) de l'essai 13 (Flou à la touche B, Bloc noir, relecture) n'a
  tourné que dans le passage GitHub de la demande de fusion.
- Les pages de l'éditeur et de la sélection sont essayées dans Chromium sous Linux, pas
  dans WebView2.

## MVP, version 0.6 « Capture défilante »

Construite sur la branche `version-0.6` puis sur la branche du fil, la 0.6 porte la version
à 0.6.0. Contrat : `contrat-0.6-capture-defilante.md`. Ses décisions viennent d'une sonde
jouée deux fois sur la machine Windows de GitHub, qui n'est pas dans `main`.

Ce qu'elle apporte (fonction Pro, ouverte par la licence) :

- La capture défilante (piste C « Mixte », choisie par l'utilisateur) : Ctrl + Maj + 5 ou
  le menu de l'icône, puis un clic sur la page ou un tracé. La page défile seule, l'aperçu
  grandit à côté, et un cran de molette reprend la main. La capture finit seule en bas, à
  16 384 px, ou par « Terminer » ou Entrée. Échap annule. L'en-tête collant n'est gardé
  qu'une fois.
- Sans licence, l'app se comporte comme la 0.5.4. Seule trace : les réglages passent en
  version 5 (clé `raccourcis.defilante`). Une version plus ancienne les lit, mais ne les
  écrit plus (dit dans les notes de version).

Chaque chantier a été écrit par un agent sur le contrat, puis relu par un agent distinct
(effort élevé). Les chantiers sont A (images vivantes), B (faire défiler une autre app),
C (recollage), D (cœur), E (fenêtre de contrôle), F (sélection), G (réglages v5 et
raccourci) et H (essai 14). Viennent ensuite :

- une relecture d'ensemble (assemblage, fusion de `main` 0.5.2) ;
- une relecture finale (fusions de `main` 0.5.3 et 0.5.4, derniers correctifs) ;
- une relecture des correctifs de la CI.

Toutes les relectures finales ont été faites par un agent distinct, en effort élevé.
Rapports hors dépôt, un par chantier et par relecture.

### Trouvé par les relectures, corrigé

- C, bloquant : la première version du recollage pouvait rendre une image fausse sans le
  dire. Il a été réécrit par un autre auteur, puis repris trois fois :
  - page clairsemée, seuil d'encre 24, survol d'une ligne, cas WPF immobile ;
  - vue unie jugée sur toute l'image ;
  - listes zébrées.
  Aucune image fausse n'a été trouvée sur du contenu depuis.
- D, bloquant : une boucle d'attente sans retour bloquait la capture. Également corrigés :
  - pas réduits après une mesure ambiguë ;
  - un bas jamais conclu sur un seul grand pas ;
  - l'étalonnage d'un décalage lu biaisé ;
  - « Trop vite » seulement après une molette de l'utilisateur, y compris après un passage à
    la main décidé par l'app.
- B : touches tenues, premier plan, délais d'UI Automation. Pas de décalage lu quand la
  hauteur de la page change pendant la capture (image paresseuse) : avant, la capture calait
  et l'image était coupée.
  Si la page change de hauteur loin du haut et que l'image gardée est unie, la capture
  s'arrête et le dit (« La page a changé. L'image s'arrête ici. ») au lieu de finir en bas
  avec une image coupée.
- E : échelle, destruction, double fin, fenêtre retrouvée.
- H : collision de casse `$M`/`$m` en PowerShell, pointeur, menu web.
- Ensemble : surlignage de l'essai 13 perdu à une fusion, « Annoter » ignoré pendant une
  défilante, menu de l'icône fermé au départ et exclu des captures.

### Trouvé sur Windows, corrigé

- Les exécutables de tests ne démarraient plus (`STATUS_ENTRYPOINT_NOT_FOUND`) :
  `TaskDialogIndirect` manquait, faute de manifeste des contrôles communs v6. `comctl32` est
  maintenant chargé à la demande. Le `.exe` publié garde son manifeste : il est sûr, d'après
  la relecture et les essais 01 à 14.
- Le test « mille prises sans fuite GDI » prenait 0 objet pour un échec. Défaut du test.
- Les pages sous un clic n'étaient jamais trouvées à temps dans l'app réelle (0 page à
  300 ms). Correction : un bassin durable de fils UI Automation, un parcours borné avec
  cache, des pages tardives ajoutées jusqu'à 1,5 s, et un client raté recréé. L'essai 14
  attend la page avant de cliquer.
- `05-plein-ecran.ps1` : `$taille` masquait `$TAILLE` (casse ignorée), et le contrôle
  passait toujours.

Vérifications GitHub : tests du cœur (894) et d'architecture (2) ; douze essais de pages,
`ECHECS : 0` ; essais Windows 01 à 14 sur le vrai `.exe`. Le passage 101 a été le premier
entièrement vert ; c'est le dernier passage de la demande de fusion qui fait foi.

### Limites acceptées

- Haut de page blanc et décalage lu encore biaisé : l'image peut avoir de 30 à 60 lignes de
  blanc en trop. Le texte n'est jamais coupé ni doublé.
- Haut de page blanc et bande insérée au-dessus de la vue pendant le premier pas : l'image
  peut avoir la hauteur de la bande en blanc en trop. Refuser le lu dans ce cas arrêterait
  aussi la page qui grandit par le bas, qui est complète.
- Bande de 200 px insérée dans la première vue : la capture passe à la molette sans
  message ; l'image rendue est exacte, mais courte.
- Une page qui saute hors de portée avant le plus petit pas n'est pas retrouvée. La capture
  s'arrête prudemment.
- Edge et l'Explorateur sont joués par l'essai 14 et seulement notés : ils finissent seuls
  en bas.

### Non vérifié

- Pavé tactile de précision, utilitaires de souris, Chrome, et un vrai PC à un écran 2K ou
  à trois écrans : ces points sont dans la liste unique des tests à faire de l'utilisateur.
- Une page réelle qui change de hauteur sous UI Automation, et un décalage lu biaisé
  réel : simulés seulement.

## Une barre d'annotation plus lisible (version 0.6.2)

Branche `claude/project-thread-9qu6yx`, partie de main après la 0.5.4 ; main y est repris
ensuite (la 0.6, capture défilante, puis la 0.6.1, « Glisser vers une appli »). Demandé par
l'utilisateur le 13/11, sur une capture de la barre avec le Flou déplié : « un peu
brouillon ici ». Trois pistes à essayer sur la vraie barre (`design/barre-claire`, dans
les fichiers du projet, construite à partir des fichiers de la 0.5.4) ; choix de
l'utilisateur le 14/11 : piste A « Calme ».

Ce qu'elle apporte :

- Le choix d'un groupe du pli (variante, taille, intensité) est une pastille claire en
  relief, plus un carré plein : deux choix différents ne se ressemblent plus.
- Aucun outil ne répète le dessin d'une de ses variantes. Le Flou est une goutte ; ses
  variantes, une mosaïque, une tache floue et une bande toujours noire, dans les deux
  thèmes. Le Projecteur est une lampe ; ses variantes gardent la forme de sa lumière.
- L'intensité du Flou se règle en trois points, comme les épaisseurs.
- Un trait fin sépare les variantes des réglages ; avec Bloc noir, il part avec
  l'intensité. Le dernier groupe ne touche plus le bord du pli.
- Un outil replié garde une petite nuance bordée de sa couleur sous son pictogramme, à la
  place du trait rouge qui se lisait comme une faute ; rien pour le Projecteur, qui n'a
  pas de couleur. L'anneau de focus d'un outil passe sous cette nuance.
- L'éditeur s'ouvre à 1096 px logiques au moins quand l'écran le permet (Forme dépliée
  avec son trait : 1078 px, plus le retrait de 16 px).
- Les jetons `editeur-vignette-*` et `editeur-picto-vignette`, inutiles depuis que la
  vignette de prise a laissé place au bouton Glisser (0.6.1), sont retirés.
- Fenêtre étroite : le pli du Flou qui s'élargit (Bloc noir puis Pixeliser) se montre
  encore en entier. L'intensité qui part (Rétablir rend Bloc noir à la zone choisie) rend
  le focus à la variante prise, plus à la page.

Vérifications, ici sous Linux :

- `editeur.cjs` 252 points, `ECHECS : 0` ; `jetons.css` à jour. Sur l'arbre
  fusionné avec main (0.6, puis 0.6.1) : `cargo test --locked`, 894 tests du cœur réussis,
  3 ignorés, et les 2 tests d'architecture ; les douze essais de pages, `ECHECS : 0`
  (`selection.cjs` 267, `reglages.cjs` 124, `historique.cjs` 122, `defilante.cjs` 85,
  `editeur-modele.cjs` 69, `vignette.cjs` 43, `bandeau.cjs` 25, `decompte.cjs` 24,
  `installateur.cjs` 23, `menu.cjs` 22, `souffle.cjs` 12).
- Essais par mutation, chacun fait échouer son essai : la lampe du Projecteur retirée, la
  marge du dernier groupe, `data-encre`, la règle `.bloc-noir` (avec un ancêtre, elle ne
  touche plus le dessin cloné), la hauteur du trait, le filtre de la tache floue, le
  liseré du Bloc noir, la pastille sombre égale à la claire, le bas de la nuance, le focus
  rendu à la variante, `montrerPli` après le changement de largeur et l'anneau d'un outil.
- Passages GitHub de la branche, avant les fusions de main : verts (Windows, dont les essais
  sur le vrai `.exe` ; macOS ; pages). Le passage de la demande de fusion fait foi.

Relecture : la maquette, avant d'être montrée, par un agent distinct de l'auteur (Fable 5.2,
effort élevé), qui a cliqué chaque piste en clair, en sombre et à 400 px ; le code, par un
autre agent distinct (Fable 5.2, effort élevé) : le diff et le code autour, la vraie barre
comparée à la maquette en clair et en sombre, la largeur de Forme aux échelles 1 à 2,25,
les jetons, l'ordre du clavier dans le pli, l'objet choisi, Annuler et Rétablir, les encres
très pâles ou très foncées et « Autre couleur », la CSP.

### Trouvé par les relectures, corrigé

- Maquette : l'intensité changeait de dessin avec la variante (trois points neutres
  maintenant) ; le Projecteur répétait une de ses variantes (lampe) ; la couleur des
  outils repliés restait un trait (petite nuance bordée) ; Bloc noir devenait clair en
  sombre (toujours noir, bordé) ; la pastille du choix manquait de contraste (liseré plus
  net) ; la piste C « Aperçus », la plus faible, est remplacée par C « Teinte » ; la page
  lisait l'arbre de travail au lieu des fichiers de la 0.5.4.
- Code : l'anneau de focus d'un outil barrait le bas de sa nuance (ombre intérieure,
  comme pour un instrument) ; en fenêtre étroite, le bout du pli du Flou restait caché
  quand il s'élargissait (déjà en 0.5.4, 7 px de pire) ; le focus tombait sur la page
  quand Rétablir cachait l'intensité qu'il tenait (déjà en 0.5.4) ; plusieurs essais
  passaient avec la fonction cassée (trait, filtre, liseré, thème sombre, bas de la
  nuance) ; un libellé d'essai et un commentaire étaient périmés.

### Non vérifié

- Rien n'a été essayé sur un vrai Windows par une personne : à l'utilisateur, d'après la
  liste unique des tests à faire.
- Les pages sont essayées dans Chromium sous Linux, pas dans WebView2.
