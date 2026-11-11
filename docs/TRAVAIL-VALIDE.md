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
