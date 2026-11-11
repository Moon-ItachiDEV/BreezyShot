# BreezyShot : architecture du code

Ce fichier est le contrat pour celles et ceux qui codent. Il applique le document
« Workflow et architecture » (https://claude.ai/artifact/JokzWxJctMc6K3RDjU9Brr), qui fait foi
pour le parcours d'une capture, et précise ce que ce document laissait ouvert.
Quand une règle change, on la change ici dans le même commit que le code.

## 1. Arborescence

```
.github/workflows/app.yml   fabrication du .exe, tests et essais à chaque envoi
app/
  package.json              ne sert qu'à figer la version de l'outil Tauri (pas de bundler)
  coeur/                    la partie sans affichage : crate Rust « breezyshot », dossier Tauri
    Cargo.toml  build.rs  tauri.conf.json  capabilities/  icons/
    src/
      main.rs               point d'entrée, ne fait qu'appeler breezyshot::lancer()
      lib.rs                déclare les modules
      application/          chef d'orchestre : démarrage, table des états, icône et menu,
                            journal, chrono, arguments de la ligne de commande
      raccourcis/           raccourcis globaux (étape 3)
      capture/              photo des écrans : windows.rs, puis macos.rs (étape 4)
      sortie/               découpe, presse-papiers, fichiers, « Récents » (étape 6)
      reglages/             fichier de réglages (étape 2)
      fenetres/             crée, place, montre et cache les fenêtres de l'interface
      systeme/              tous les gestes propres à un système : mod.rs (façade commune),
                            windows.rs, autre.rs (Linux pour le développement, macos.rs ensuite)
  interface/                ce qu'on voit (web), partagé entre Windows et Mac
    commun/                 jetons du design system (jetons.css), composants partagés
    vignette/               vignette, souffle et bandeau de messages (bandeau.* dès l'étape 3)
    selection/  barre/  reglages/   (étapes 5, 10)
design/                     jetons et icônes Moulinet, copiés du design system
docs/                       ce fichier, TRAVAIL-VALIDE.md, notices d'essai
essais/                     scénarios PowerShell joués sur la machine Windows de GitHub
```

Écart assumé avec le document de workflow : les dossiers de modules sont sous `app/coeur/src/`
(convention Cargo) et non directement sous `app/coeur/`.

## 2. Règles qui tiennent l'ensemble

1. **Seule l'Application connaît les autres modules.** `raccourcis`, `capture`, `sortie`,
   `reglages` et `fenetres` ne s'importent jamais entre eux et n'importent jamais `application`.
   Ils peuvent tous utiliser `systeme` (boîte à outils) ; `systeme` ne connaît aucun module.
   Un test (`cargo test`) lit les sources et échoue si un `use crate::…` interdit apparaît.
2. **L'Application appelle, les modules signalent.** Un module expose des fonctions et renvoie
   ses événements par un canal ou une fonction de rappel reçue à sa création, avec son propre
   type de signal (ex. `raccourcis::Signal`). Jamais de module à module.
3. **Tout ce qui dépend du système vit dans `systeme/`** (et, à partir de l'étape 4, dans
   `capture/`). Ailleurs : aucun `#[cfg(windows)]`, aucun appel à la crate `windows`, aucun
   chemin écrit « à la Windows ». Le code doit compiler sur Windows, macOS et Linux ;
   `autre.rs` fournit des réponses neutres.
4. **Tout se compte en pixels réels.** Le cœur ne manipule que des pixels physiques. Seules
   les fenêtres convertissent avec l'échelle de leur écran. L'app est déclarée consciente de
   l'échelle de chaque écran (Per-Monitor V2) ; le démarrage l'écrit dans le journal.
5. **L'affichage ne décide rien.** Les pages web montrent et rapportent. Toute décision
   (quel raccourci, quel nom de fichier, quel message) se prend en Rust et se teste sans écran.
6. **Aucun accès au réseau, aucun secret dans les réglages, aucune image dans le journal.**
7. **Une logique = une fonction pure + des tests.** Ce qui touche à Tauri ou au système reste
   mince ; la décision est dans une fonction sans effet de bord, testée par `cargo test`.
8. **Jamais de `unwrap()`/`expect()` hors des tests** sur une donnée qui vient du disque, du
   système ou de l'utilisateur. Une erreur redescend au Repos avec une ligne de journal, et
   un message si l'utilisateur doit le savoir. L'app ne doit pas pouvoir bloquer l'écran.
9. **Français partout** : identifiants sans accents (`lancer_capture`, `Etat::Repos`),
   commentaires et messages en français simple. Les textes visibles suivent le ton du design
   system (« Copié dans le presse-papiers », `Ctrl + Maj + 1`, « Impr. écran »).

Habitudes qui préparent les lots suivants (feuille de route, à respecter dès maintenant) :
états et passages listés à un seul endroit sous forme de table ; raccourcis rangés par nom
d'action ; « Récents » lu et écrit par `sortie` seulement, aucun autre module ne fabrique un
chemin de capture ; tout ce qui quitte l'app passera par une seule fonction de `sortie` ;
les boutons de la vignette viendront d'une liste d'actions fournie par l'Application ; la
Sélection rendra une zone ou une fenêtre sans savoir ce qu'on en fera ; rien ne suppose quelle
méthode a pris l'image (GDI aujourd'hui).

### Licence et fonctions Pro

Ce dépôt est public, sous licence FSL-1.1-MIT (`LICENSE.md`). Il contient l'app gratuite
complète. Les fonctions Pro (historique et texte reconnu, fonds et masquage automatique,
capture déroulante, vidéo et GIF : lots 3 à 6 de la feuille de route) n'y seront jamais.
Rien de tout cela n'est construit pour l'instant ; voici seulement la place réservée :

- Le code Pro vivra dans un dépôt privé séparé, sous la forme d'une crate `breezyshot-pro`
  (et de ses pages) déposée dans `app/pro/`, dossier ignoré par git ici.
- Il se branchera comme n'importe quel module : sur l'Application, et nulle part ailleurs,
  derrière une option de compilation `pro`. Sans cette option, l'app compile, passe ses
  essais et fonctionne : aucun module public ne dépend du Pro, et aucun `if pro` n'est
  dispersé dans le code public.
- Conséquence dès maintenant : tout ce qu'un lot futur devra étendre (table des états, liste
  des actions de la vignette, entrées du menu, raccourcis par nom d'action) est une liste
  que l'Application assemble, pas un cas écrit en dur dans un module.

### Ne pas se fermer la porte du Microsoft Store

Un paquet MSIX (signé gratuitement par le Store) est faisable plus tard, mais une app
empaquetée voit ses écritures dans AppData et dans le registre redirigées vers un endroit
privé. Trois précautions suffisent, dès maintenant :

- tous les chemins passent par `systeme::dossiers()`, et l'app n'écrit jamais à côté du .exe ;
- l'app lit le registre de l'utilisateur mais n'y écrit jamais d'état à elle ;
- « démarrer avec le système » (étape 10) restera derrière une seule fonction de `systeme`,
  pour pouvoir choisir plus tard entre l'entrée de démarrage classique et celle d'un paquet.

## 3. Application

### 3.1 États (`application/etats.rs`)

Six états : `Repos`, `Gel`, `Selection`, `Decoupe`, `Souffle`, `Vignette`. Les passages sont
une **table constante unique** `(Etat, Declencheur) -> Etat` et une fonction pure
`suivant(etat, declencheur) -> Option<Etat>` (`None` = déclencheur ignoré dans cet état).
Ajouter un état, c'est ajouter des lignes. La table suit le tableau « États » du workflow :
un raccourci de capture ne lance une capture que depuis `Repos` (et depuis `Vignette`, où il
ferme la vignette puis relance) ; il est ignoré ailleurs.

Tant que la Capture n'existe pas (étapes 1 à 3) : `Repos --DemandeCapture(mode)--> Gel
--GelImpossible--> Repos`, avec un message dans le bandeau. Chaque changement d'état écrit une
ligne de journal `etat Repos -> Gel (DemandeCapture zone)`.

Modes : `zone`, `plein_ecran`, `fenetre`. Ce sont aussi les **noms d'action** des raccourcis.

### 3.2 Journal (`application/journal.rs`)

- Façade `log` (`log::info!` etc.) : les modules écrivent sans connaître l'Application.
- Fichier `<local>/Journal/breezyshot.log`, une ligne par événement :
  `2026-11-11T18:40:12.345+01:00 INFO  raccourcis  message`.
- Rotation : 1 Mo par fichier, 5 fichiers au plus (`breezyshot.log`, `.1` … `.4`).
- Jamais d'image, jamais le contenu du presse-papiers. Les chemins de fichiers sont permis.
- Les essais automatiques lisent ce fichier : les lignes citées dans `essais/` sont un contrat,
  on ne les reformule pas sans adapter l'essai.

### 3.3 Dossiers

Résolus à un seul endroit (`systeme::dossiers()`), jamais reconstruits ailleurs :

| Quoi | Windows | Règle |
| --- | --- | --- |
| Réglages | `%APPDATA%\BreezyShot\reglages.json` | dossier de configuration de l'utilisateur |
| Local (Journal, Recents) | `%LOCALAPPDATA%\BreezyShot\` | dossier de données locales |
| Images | dossier Images connu de Windows (peut être dans OneDrive) `\BreezyShot` | étape 6 |

Si la variable d'environnement `BREEZYSHOT_DOSSIER_ESSAI` est définie, tout va dessous
(`reglages/`, `local/`, `images/`) : les essais ne touchent jamais aux vrais dossiers.

Le dossier Images est résolu mais optionnel (`Option`) jusqu'à l'étape 6. Le journal ne
dépend que du dossier local (`systeme::dossier_journal()`) : un autre dossier introuvable
ne l'empêche pas. Si le dossier local est introuvable ou non inscriptible, le journal va
dans le dossier temporaire du système (`systeme::dossier_secours()`), sous le nom
`breezyshot-secours.log`.

### 3.4 Ligne de commande et instance unique

- Une seule instance. Lancer `breezyshot.exe` une seconde fois ne fait rien de visible : la
  première écrit `seconde instance refusee` dans le journal, la seconde sort avec le code 0.
- `breezyshot.exe --quitter` : demande à l'instance en cours de se fermer proprement (libère
  les raccourcis, retire l'icône, écrit `arret propre`), puis sort. Sans instance en cours,
  sort tout de suite avec le code 0. C'est le moyen qu'utilisent les essais pour fermer l'app.
- Aucune autre option pour l'instant.
- Au démarrage, après `icone installee`, l'app vérifie que l'icône est réellement dans la
  zone de notification (son rectangle est connu du système) et écrit `icone visible` ou
  `icone invisible` (toujours `invisible` sous Linux, où Tauri ne donne pas ce rectangle).

### 3.5 Icône et menu

- Icône Moulinet monochrome : `notif-blanc` sur une barre des tâches sombre, `notif-sombre`
  sur une barre claire, choisie d'après le thème de la barre des tâches (pas celui des applis)
  et mise à jour quand il change. Jamais la version couleur dans la zone de notification.
- Menu (clic droit) : seulement ce qui marche. Étape 1 : « Quitter BreezyShot ». Étape 3 :
  « Capturer une zone » avec le raccourci actif affiché, un séparateur, « Quitter BreezyShot ».
  Les entrées « Captures récentes », « Vider les récentes », « Réglages » arrivent avec leurs
  étapes.
- Info-bulle : « BreezyShot ».

## 4. Réglages (`reglages/`, étape 2)

Module pur : il reçoit le dossier où écrire, ne connaît ni Tauri ni les autres modules.

```json
{
  "version": 1,
  "raccourcis": {
    "zone": ["ImprEcran", "Ctrl+Maj+1"],
    "plein_ecran": ["Ctrl+Maj+2"],
    "fenetre": ["Ctrl+Maj+3"]
  },
  "copier_apres_capture": true,
  "enregistrer_apres_capture": false,
  "vignette": { "afficher": true, "duree_s": 6, "coin": "bas_gauche" },
  "selection": { "loupe": false },
  "demarrer_avec_systeme": false,
  "dossier_enregistrement": null,
  "modele_nom": "BreezyShot {date} à {heure}",
  "animations": "systeme",
  "theme": "systeme"
}
```

- `raccourcis` : par **nom d'action**, une liste de candidats par ordre de préférence. Le
  premier qui peut être réservé est utilisé : le repli n'est pas un cas particulier. Les
  réglages gardent des chaînes sans les interpréter (c'est le module `raccourcis` qui les lit).
  Les valeurs par défaut dépendent du système (`ImprEcran` n'existe pas sur Mac).
- `dossier_enregistrement: null` = Images › BreezyShot. `coin` : `bas_gauche`, `bas_droite`,
  `haut_gauche`, `haut_droite`. `animations` : `systeme`, `normales`, `reduites`.
  `theme` : `systeme`, `clair`, `sombre`.
- **Fichier absent** : créé avec les valeurs par défaut.
- **Fichier illisible** (pas du JSON, ou pas un objet) : renommé `reglages.abime-<horodatage>.json`
  (on ne détruit jamais ce que l'utilisateur avait), puis recréé par défaut. On garde au plus
  3 fichiers abîmés.
- **Valeur invalide dans un fichier lisible** (mauvais type, valeur inconnue, durée hors de
  1 à 60 s) : la valeur par défaut remplace cette valeur seulement, avec un avertissement.
- **Champ inconnu** : conservé tel quel à la réécriture.
- **Version plus ancienne** : mise à niveau pas à pas par une liste de fonctions
  (`v0 -> v1`, puis `v1 -> v2` un jour) qui complète sans rien effacer, puis réécriture.
  Un fichier sans champ `version` est une version 0.
- **Version plus récente que l'app** : on lit ce qu'on comprend et on ne réécrit pas le fichier.
- **Au moins une sortie active** : si copie, enregistrement et vignette sont tous éteints, la
  copie est rallumée à la lecture, et `ecrire` refuse cette combinaison.
- Écriture atomique (fichier temporaire puis renommage), JSON indenté, UTF-8, fin de ligne finale.
- Interface : `charger(dossier, plateforme) -> (Reglages, Vec<Constat>)`, `ecrire(dossier,
  &Reglages) -> Result`, `Reglages::par_defaut(plateforme)`. La `Plateforme` (Windows, Mac,
  Autre) vient de `systeme` par l'Application : elle ne change que les raccourcis par défaut
  (ceux du Mac sont provisoires). Les constats (créé, abîmé, mis à niveau, valeur remplacée,
  version plus récente) sont rendus à l'Application, qui les écrit au journal. Le module
  n'affiche rien lui-même.
- Précisions arrêtées à l'écriture du module : le fichier n'est réécrit que si le contenu
  corrigé diffère de ce qui a été lu (un fichier juste mais mis en forme autrement n'est pas
  touché) ; une liste de candidats vide veut dire « pas de raccourci pour cette action » ; un
  fichier présent mais illisible pour le système (droits, verrou) n'est ni renommé ni réécrit ;
  un fichier en lecture seule n'est pas réécrit ; `ecrire` refuse des réglages lus dans un
  fichier d'une version plus récente, pour ne jamais faire redescendre le numéro de version.

## 5. Raccourcis (`raccourcis/`, étape 3)

- **Notation** dans le fichier : modificateurs `Ctrl`, `Maj`, `Alt`, `Win` (et `Cmd`, `Option`
  pour le Mac) puis une touche, séparés par `+`, sans tenir compte de la casse ni des espaces.
  Touches : `A`–`Z`, `0`–`9`, `F1`–`F24`, `ImprEcran`, `Espace`, `Entree`, `Tab`, `Echap`,
  `Suppr`, `Inser`, `Debut`, `Fin`, `PagePrec`, `PageSuiv`, flèches `Haut` `Bas` `Gauche`
  `Droite`. Une touche seule n'est acceptée que pour `ImprEcran` et `F1`–`F24`.
- **Affichage** pour l'utilisateur : `Ctrl + Maj + 1`, `Impr. écran`.
- **Choix** (fonction pure) : pour chaque action, on parcourt les candidats ; on écarte un
  candidat invalide, un candidat `ImprEcran` quand Windows le réserve à l'Outil Capture
  (`systeme::impr_ecran_reserve()`), un candidat déjà pris dans cette session par une autre
  action, et un candidat que le système refuse. Le bilan dit, par action, le raccourci retenu
  (ou aucun) et pourquoi chaque candidat écarté l'a été. Une liste vide veut dire « pas de
  raccourci voulu » : ni raccourci ni message.
- **Seules les actions qui existent sont réservées.** L'Application tient la liste des actions
  dont la capture est construite (`zone` à l'étape 3, `plein_ecran` et `fenetre` quand leur
  étape arrive) et ne donne que celles-là au choix. BreezyShot ne prend jamais à une autre
  application un raccourci qui ne ferait rien.
- Le module ne connaît pas le type des réglages : l'Application lui passe une table
  `action → candidats` construite à partir d'eux.
- **`systeme::impr_ecran_reserve()`** (Windows) : valeur `PrintScreenKeyForSnippingEnabled` de
  `HKCU\Control Panel\Keyboard`. Présente : réservée si différente de 0. Absente : réservée
  si la version de Windows est 22631 (23H2) ou plus, sinon libre. Hors Windows : libre.
- **Messages au démarrage** (bandeau, genre « info »), seulement si le premier candidat
  d'une action n'a pas été retenu, et **une seule fois par situation** : l'Application garde
  la dernière situation annoncée (action, candidat écarté, raison) dans `<local>/etat.json`
  et ne réaffiche le message que si elle change. Quand tout est normal, elle l'efface.
  La décision « annoncer ou non » est une fonction pure, testée.
  - réservée : « Impr. écran est réservée à l'Outil Capture de Windows » / « BreezyShot
    utilise Ctrl + Maj + 1. Pour la libérer : Paramètres › Accessibilité › Clavier. »
  - prise : « Impr. écran est déjà utilisée par une autre application » / « BreezyShot utilise
    Ctrl + Maj + 1. »
  - aucun candidat libre (genre « erreur », reste jusqu'à la croix) : « Aucun raccourci
    disponible » / « Lance une capture depuis l'icône BreezyShot, près de l'horloge. »
- **Signal** : `Signal::Presse(action)`. Étape 5 ajoutera `reserver_echap` et `Signal::Echap`.
- À l'appui, l'Application appelle `lancer_capture(mode)`. Aux étapes 1 à 3, comme la Capture
  n'existe pas : bandeau « réussite » « Raccourci reçu : capture de zone » (message provisoire,
  remplacé à l'étape 4).

## 6. Fenêtres et bandeau (`fenetres/`, `interface/vignette/bandeau.*`)

- Le bandeau est une fenêtre préparée au démarrage et cachée : transparente, sans bordure,
  sans ombre système, toujours au-dessus, absente de la barre des tâches, **ne prend jamais le
  focus ni le clavier**, exclue des captures d'écran. Elle laisse passer tous les clics, sauf
  sur la pilule d'un message qui porte une croix (voir plus bas).
- `fenetres::Bandeau::afficher(Message { genre, titre, detail, actions })`, genres `Reussite`,
  `Info`, `Erreur`. `actions` est une liste `{ id, libelle }` fournie par l'Application
  (souvent vide). Un nouveau message remplace l'ancien tout de suite.
- Place : centré en haut de la zone utile de l'écran où se trouve la souris, à 64 px du haut
  (sous la future barre de capture). Position calculée en pixels réels avec l'échelle de cet
  écran.
- Aspect, d'après la maquette complète (https://claude.ai/artifact/AeUuBXPRC8U1AMiqg8dhTe) :
  fond opaque `acrylic-solid`, bordure `acrylic-stroke`, ombre `shadow-28`, pastille ronde
  de 20 px. Suit le thème clair ou sombre de Windows.
  - **Réussite** : pilule sur une ligne (rayon `radius-pill`), coche sur `success`, titre seul.
    Part seule après 2,5 s.
  - **Erreur** (variante A choisie le 11/11) : pilule élargie (rayon 16 px, 460 px au plus),
    « ! » sur `danger`, titre, cause sur une seconde ligne, boutons d'action éventuels et une
    croix. Reste affichée jusqu'à un clic sur la croix ou sur une action.
  - **Info** (forme de l'erreur A, validée par le fil design le 11/11) : « i » sur `accent`,
    pas de bouton d'action, reste affichée jusqu'à un clic sur la croix.
- Mouvement : entrée 300 ms `courbe-ressort` depuis 12 px plus haut, sortie fondu 167 ms.
  Avec « Réduire les animations » : fondus de 120 ms.
- Clics : la page donne au cœur le rectangle de la pilule (`bandeau_zone`). Tant qu'un message
  à croix est affiché, le cœur surveille la souris et ne rend la fenêtre cliquable que
  lorsqu'elle est sur la pilule ; le reste du temps, et partout ailleurs, les clics traversent.
  La page rapporte un clic par `bandeau_action { id, action }` (`"fermer"` pour la croix).
  Une action est décidée par l'Application, jamais par la page. C'est le cœur qui décide,
  d'après le genre du message, s'il surveille la souris : dès qu'une réussite remplace un
  message à croix, la fenêtre redevient entièrement traversée par les clics, sans attendre la
  page. Le cœur ignore un `bandeau_zone`, un `bandeau_action` ou un `bandeau_termine` dont
  l'`id` n'est pas celui du message en cours.
- La page prévient le cœur quand le message a fini de disparaître (`bandeau_termine { id }`) ;
  le cœur cache alors la fenêtre et arrête de surveiller la souris.
- Les pages n'utilisent ni bundler ni framework : HTML, CSS et modules JavaScript simples,
  `window.__TAURI__` exposé. Les couleurs, durées et courbes viennent de `commun/jetons.css`,
  généré à partir de `design/jetons.json` ; aucune valeur du design system n'est recopiée à
  la main dans une page.

## 7. Vérification

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` : sur Windows (machine
  GitHub), et un `cargo check` + `cargo test` sur macOS pour garder le Mac ouvert.
- `essais/` : un script par étape (`01-socle.ps1`, `02-reglages.ps1`, `03-raccourci.ps1`),
  lancés par `essais/lancer.ps1` sur le .exe de release. Chaque essai démarre l'app avec
  `BREEZYSHOT_DOSSIER_ESSAI`, agit (lancer une seconde instance, simuler une touche, abîmer un
  fichier), puis lit le journal et les fichiers. Un essai qui échoue fait échouer GitHub.
- Chaque exécution note la mémoire privée au repos (l'app et ses processus de moteur web) dans
  le résumé GitHub. À partir de l'étape 5, elle note aussi le chrono de la sélection.
- Une étape n'est finie que relue par un agent distinct et inscrite dans `TRAVAIL-VALIDE.md`.
