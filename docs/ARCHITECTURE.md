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
                            journal, chrono, arguments de la ligne de commande ;
                            zone.rs : décisions de la capture d'une zone (fonctions pures)
      raccourcis/           raccourcis globaux (étape 3), Échap global de la sélection
      capture/              figer (appelle systeme::gel), géométrie de la sélection, découpe
                            (étape 4) ; aucun code propre à un système
      sortie/               presse-papiers, PNG, « Récents », enregistrement (étape 6)
      reglages/             fichier de réglages (étape 2)
      fenetres/             crée, place, montre et cache les fenêtres de l'interface :
                            mod.rs (bandeau), place.rs, selection.rs (étape 5)
      systeme/              tous les gestes propres à un système : mod.rs (façade commune),
                            windows.rs, autre.rs (Linux pour le développement, macos.rs ensuite),
                            composition.rs (attendre le compositeur),
                            gel/ (photo du bureau et écran figé natif : mod.rs, windows.rs,
                            autre.rs)
  interface/                ce qu'on voit (web), partagé entre Windows et Mac
    commun/                 jetons du design system (jetons.css), composants partagés
    vignette/               vignette, souffle et bandeau de messages (bandeau.* dès l'étape 3)
    selection/              page de la sélection (étape 5)
    barre/  reglages/       (étape 10)
design/                     jetons et icônes Moulinet, copiés du design system
docs/                       ce fichier, TRAVAIL-VALIDE.md, notices d'essai
essais/                     scénarios PowerShell joués sur la machine Windows de GitHub
                            (NN-*.ps1 ; aides : tenir-raccourci.ps1, temoin.ps1)
  pages/                    essais des pages dans Chromium (Playwright), sur Linux
outils/                     petits scripts Node sans dépendance (generer-jetons.mjs)
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
3. **Tout ce qui dépend du système vit dans `systeme/`**, et seulement là (`systeme/gel/`
   compris). Ailleurs, `capture/` et `sortie/` compris : aucun `cfg` qui parle du système
   (`windows`, `unix`, `target_os`…), aucun appel à la crate `windows`, aucun chemin écrit
   « à la Windows ». Le code doit compiler sur Windows, macOS et Linux ;
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

Chaque changement d'état écrit une ligne de journal `etat Repos -> Gel (DemandeCapture
zone)`. La capture d'une zone suit `Repos -> Gel -> Selection -> Decoupe -> Repos` (§3.6).
Ajouté à l'assemblage : `DecoupeImpossible` (`Decoupe -> Repos`), quand les pixels de la
zone n'ont pas pu être lus, et `SortieSansBilan` (`Decoupe -> Repos`), quand la sortie ne
rend aucun bilan dans son délai. Tant que la vignette (étape 9) n'existe pas, la découpe finit
par `ImagePreteSansVignette`.

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
Si `BREEZYSHOT_ESSAI_CAPTURABLE=1` (lu par `systeme::essai_capturable()`, essais seulement),
le bandeau n'est pas exclu des captures (`bandeau exclu des captures: non (essai)`), pour
qu'un essai prouve par une copie d'écran qu'il est réellement peint.

Le dossier Images est résolu mais optionnel (`Option`) : s'il est introuvable, un
enregistrement demandé échoue avec sa cause, la copie se fait quand même. Le journal ne
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
- Lignes du journal sur lesquelles les essais s'appuient : `demarrage BreezyShot <version>`,
  `systeme windows build <n>`, `echelle par ecran v2: oui|non|sans objet`, `icone installee`
  (ou `icone non installee: …`), `icone visible|invisible`, `pret` (tout est en place), `seconde instance refusee`,
  `--quitter recu: fermeture demandee`, `arret propre`, `--quitter: aucune instance en cours`.
- `app/interface/index.html` est une page vide : elle n'existe que parce que Tauri veut un
  dossier d'interface non vide.

### 3.5 Icône et menu

- Icône Moulinet monochrome : `notif-blanc` sur une barre des tâches sombre, `notif-sombre`
  sur une barre claire, choisie d'après le thème de la barre des tâches (pas celui des applis)
  et mise à jour quand il change. Jamais la version couleur dans la zone de notification.
- Chaque entrée de capture du menu lance la même capture que le raccourci de son mode, mais
  après `ATTENTE_MENU` (350 ms) : un petit fil dort puis poste `lancer_capture` sur le fil
  principal, pour que le menu, que Windows fait disparaître en fondu, ne soit pas sur la
  photo. Un raccourci n'attend jamais. Si l'état n'est plus Repos ni Vignette à l'échéance,
  la table ignore la demande. Journal : `menu: capturer <mode>` à la réception.
- Menu (clic droit) : seulement ce qui marche. Étape 1 : « Quitter BreezyShot ». Étape 7 :
  une entrée par mode construit, dans l'ordre `ORDRE_MENU` (« Capturer une zone »,
  [« Capturer une fenêtre » à l'étape 8], « Capturer l'écran »), un séparateur, « Quitter
  BreezyShot ». La liste est construite par `entrees_menu` à partir de `ACTIONS_CONSTRUITES` :
  un mode construit y entre sans autre code. Identifiants `capturer_<mode>`.
  Le raccourci suit une tabulation dans le texte de l'entrée (aligné à droite par les menus
  Windows) ; sans raccourci retenu, l'entrée reste, sans raccourci affiché.
  Les entrées « Captures récentes », « Vider les récentes », « Réglages » arrivent avec leurs
  étapes.
- Info-bulle : « BreezyShot ».

### 3.6 Capture d'une zone (assemblage des étapes 4 à 6)

Le câblage est dans `application/mod.rs` ; les décisions (que faire de chaque signal, quel
message, quel dossier, quelles lignes de journal) sont des fonctions pures de
`application/zone.rs`, testées. Les modules ne se connaissent toujours pas.

**Fils.** Le raccourci arrive sur le fil principal (global-hotkey : `WM_HOTKEY` dans la
procédure de sa fenêtre cachée, créée par le plugin dans `setup`). Le menu de l'icône aussi
(événement de la boucle). Les commandes de la page (`#[tauri::command]` sans `async`)
tournent sur le fil principal, comme les signaux de fin de la Sélection. `traiter` repose
sur le fil principal tout ce qui touche à la capture et arriverait d'ailleurs. Attention :
`run_on_main_thread` appelé depuis le fil principal exécute tout de suite, il ne poste pas.

**Séquence** (fil principal, sauf la sortie) :

1. `lancer_capture(Zone)` : `DemandeCapture` → Gel. Un appui pendant Gel, Sélection ou
   Découpe est ignoré par la table (`capture deja en cours: appui ignore`) : aucun second
   `figer` tant qu'une capture est en cours.
2. (Début commun à tous les modes, `photographier`, avec l'étape 3.) Le bandeau est caché s'il montre un message (`Bandeau::cacher`), puis
   `systeme::attendre_composition(100 ms)` (deux `DwmFlush` dans un petit fil ; le fil
   principal n'attend jamais plus de 100 ms, sinon `composition non attendue (100 ms): photo
   prise quand meme`) : il n'est ni sur la photo ni
   au-dessus de l'écran figé.
3. `capture::figer()`. Erreur → `GelImpossible`, bandeau d'erreur.
4. `systeme::gel::montrer_ecran_fige`. La garde de l'écran existe dès avant. Erreur →
   `GelImpossible`, bandeau d'erreur.
5. `PhotoPrise(Zone)` → Selection. La capture en cours (photo, garde) est rangée, puis
   `Selection::montrer(Demande { bureau, ecrans, dessous: poignee_ecran_fige(), cote_min:
   capture::COTE_MIN })`. Un signal de fin arrivé pendant `montrer` la trouve donc.
   Erreur → tout cacher, `SelectionImpossible`, bandeau d'erreur.
6. Échap est réservé en global (`raccourcis::reserver_echap`) ; s'il ne peut pas l'être,
   une ligne de journal, et la page gère Échap quand elle a le focus.
7. Signaux : `Prete` → rien. `Zone` : la zone est décidée d'abord, encore en Sélection
   (`zone::decider`) : `borner_a_un_ecran(rect_entre(depart, arrivee), depart, ecrans)`
   puis `zone_valide`. Absente ou trop petite → tout caché, `SelectionAnnulee` depuis
   Sélection, sans message. Valide → écran figé caché dans le même tour que la sélection,
   Échap libéré, `ZoneChoisie`, `capture::decouper` (l'écran est déjà libre) puis la
   sortie. `Annulee` (toute cause),
   Échap global → `SelectionAnnulee`, sans message. `Muette`, `Impossible` →
   `SelectionMuette`, `SelectionImpossible`, bandeau d'erreur. Écrans changés, session
   verrouillée → `EcransChanges`, `SessionVerrouillee`, sans message. Un signal d'un autre
   affichage que celui en cours est ignoré.
8. Sortie dans un fil `sortie` : `sortie::sortir` avec les réglages lus au démarrage
   (sauf l'enregistrement : `zone::doit_enregistrer(réglage, VIGNETTE_CONSTRUITE)`, vrai
   tant que la vignette n'existe pas, versions 0.1 et 0.2 ; le réglage lu n'est pas
   modifié et le fichier de réglages n'est jamais écrit pour cela), puis
   retour sur le fil principal : journal, `ImagePreteSansVignette` → Repos, annonce dans
   le bandeau. Dossier affiché : « Images › BreezyShot » quand le réglage est absent ou
   vide, sinon `sortie::nom_affiche(dossier)`. « Récents » : `<local>/Recents`.
   Chaque sortie a un numéro. Le fil rend toujours un bilan : une panique de `sortir`
   donne un bilan d'échec (`zone::bilan_d_echec`). Un petit fil tient une échéance de
   10 s (`zone::DELAI_SORTIE`) et s'arrête dès que le bilan est rendu ; s'il tombe,
   `sortie sans bilan`, `SortieSansBilan` → Repos, bandeau « Impossible de capturer ». Un
   bilan qui n'est pas celui attendu (capture précédente, ou arrivé après l'échéance) est
   ignoré (`sortie <n>: bilan perime ignore`).

**Messages.** Erreur avant la sortie (gel, écran figé, sélection, découpe, sortie sans
bilan) : « Impossible de capturer », titre seul, sans cause ; le cas va au journal
(`capture impossible: <code>`). Après la sortie : l'annonce de `sortie::annonce`, telle quelle. Une
annulation n'affiche rien.

**Filets de sécurité** (l'écran ne doit jamais rester couvert) :

- `GardeEcran`, tenue par la capture en cours : sa destruction cache l'écran figé et la
  sélection et libère Échap. Elle est détruite à chaque fin et chaque erreur, par
  `lancer_capture` tant qu'elle n'est pas rangée, et à l'arrêt. Une fois rangée dans le
  `Mutex` de l'Application, une panique ne la détruit pas : ce qui libère alors l'écran,
  c'est la mort du processus (une panique dans une procédure de fenêtre `extern "system"`
  arrête le processus ; ailleurs sur le fil principal, tao la relance et l'app s'arrête),
  et Windows détruit ses fenêtres.
- `garder_ecran_fige(5 s)` au démarrage : fil principal bloqué avec l'écran figé montré →
  `ecran fige: fil principal bloque, arret force`, journal vidé, `exit(1)`. Rien de long
  sur le fil principal pendant que l'écran figé est montré : la découpe vient après.
- `quand_bureau_change` au démarrage : son rappel (dans la procédure de la fenêtre figée)
  ne fait que poster l'événement (par un petit fil, puisque `run_on_main_thread` exécuterait
  sur place).
- Toute décision montrer ou cacher se prend sur le fil principal. Aucun `figer` juste
  après un masquage posté depuis un autre fil.
- À l'arrêt : capture en cours détruite (garde), Échap libéré, `Selection::fermer`,
  `detruire_ecran_fige`, `liberer_image`.

**Démarrage**, après le bandeau : `preparer_ecran_fige`, la garde, `quand_bureau_change`,
`Selection::creer` puis `app.manage`. Les quatre commandes de la sélection sont dans
`invoke_handler`. Un échec est écrit au journal ; une capture répond alors « Impossible de
capturer ».

**Journal** (contrat des essais) : `ecran fige pret`, `ecran fige fenetre <hwnd>`,
`ecran fige: verrouillage de session suivi: oui|non`, `selection prete a servir`,
`chrono gel photo <ms> total <ms> visible <ms>` (`visible` : de la réception du raccourci
à la fin de `montrer_ecran_fige`), `echap global reserve|libere|presse` (ou `echap global
non reserve: …`), `zone choisie <x>,<y> <l>x<h>`, `sortie copie <faite|non demandee|echec>
recent <ok|echec> fichier <fait|non demande|echec>`, `recents: echec (…)`, `sortie annonce
reussite|erreur: <titre>`, `chrono decoupe <ms> sortie <ms> relache-annonce <ms>`,
`capture impossible: <photo|ecran_fige|selection|muette|lecture|non_disponible|
sortie_sans_bilan>`, `sortie sans bilan`, `sortie <n>: bilan perime ignore`,
`menu: capturer <mode>`. Aucun chemin de capture complet, aucun contenu.

### 3.7 Capture plein écran (étape 7)

`Ctrl + Maj + 2` (ou « Capturer l'écran » au menu) garde **l'écran où se trouve la
souris**, sans écran figé ni sélection : Gel → Découpe → Sortie → bandeau. Rien ne
s'affiche par-dessus l'écran avant le bandeau. Séquence de `lancer_capture(PleinEcran)`
(fil principal) :

1. `DemandeCapture(PleinEcran)` : ignoré hors Repos et Vignette (la table).
2. La souris est lue tout de suite, avant de cacher le bandeau et avant la photo, en pixels
   réels : `systeme::position_souris()`, sinon `cursor_position` de Tauri (le repli du
   bandeau), arrondie par `zone::point_souris`.
3. Début commun (`photographier`) : bandeau caché, compositeur attendu (100 ms au plus),
   `capture::figer()`. Erreur → `GelImpossible` → Repos, « Impossible de capturer ».
4. L'écran, par la fonction pure `capture::ecran_de_capture(souris, ecrans)` : celui qui
   contient la souris (`Origine::Souris`) ; sinon (souris inconnue ou sur aucun écran)
   le principal (`Principal`) ; sinon le premier (`Premier`) ; liste vide → `GelImpossible`,
   `capture impossible: photo`.
5. `PhotoPrise(PleinEcran)` → Découpe. Aucun `montrer_ecran_fige`, aucune sélection, aucun
   Échap réservé, aucune `GardeEcran`, aucune `CaptureEnCours` rangée.
6. `capture::decouper(&gel, ecran.rect)` puis `lancer_sortie` (le même que la zone).
   Erreur de lecture → `DecoupeImpossible`, bandeau.
7. Bilan : `finir_sortie`, inchangé (`ImagePreteSansVignette` → Repos, annonce).

L'image a la taille de l'écran choisi en pixels réels, barre des tâches comprise.

**Journal** (contrat de l'essai 05) : `plein ecran <x>,<y> <l>x<h>
<souris|principal|premier>` après le choix de l'écran, `chrono gel photo <ms> total <ms>`
(sans `visible` : rien n'est montré), puis les lignes de sortie de §3.6 ;
`relache-annonce` se compte ici depuis la réception de la demande.

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
- **On n'écrase jamais ce qu'on n'a pas pu lire ou mettre de côté.** Si le fichier est présent
  mais illisible, ou abîmé sans pouvoir être renommé (verrou, lecture seule), l'app tourne
  avec les valeurs par défaut et ces réglages sont « protégés » : `ecrire` les refuse
  (`FichierProtege`), `Reglages::enregistrable()` le dit à l'avance. Un nouveau `charger`
  réussi lève la protection.
- Sauvegardes de fichiers abîmés : la sauvegarde qu'on vient de faire n'est jamais supprimée
  par le nettoyage, et le nettoyage ne touche qu'aux fichiers dont le nom a exactement le
  format produit par le module. Un fichier de plus de 1 Mio est traité comme abîmé.
- Accepté : un très grand nombre dans un champ inconnu peut perdre en précision à la
  réécriture (l'option qui l'éviterait s'applique à tout le programme, Tauri compris).

## 5. Raccourcis (`raccourcis/`, étape 3)

- **Notation** dans le fichier : modificateurs `Ctrl`, `Maj`, `Alt`, `Win` (et `Cmd`, `Option`
  pour le Mac) puis une touche, séparés par `+`, sans tenir compte de la casse ni des espaces.
  Touches : `A`–`Z`, `0`–`9`, `F1`–`F24`, `ImprEcran`, `Espace`, `Entree`, `Tab`, `Echap`,
  `Suppr`, `Inser`, `Debut`, `Fin`, `PagePrec`, `PageSuiv`, flèches `Haut` `Bas` `Gauche`
  `Droite`. Une touche seule n'est acceptée que pour `ImprEcran` et `F1`–`F24`. Sont refusés
  aussi : `Maj` comme seul modificateur (sauf avec `F1`–`F24` ou `ImprEcran`), parce que cela
  bloquerait la frappe, et une courte liste fixe de raccourcis du système (`Ctrl+C`, `Ctrl+V`,
  `Ctrl+X`, `Ctrl+Z`, `Ctrl+A`, `Ctrl+S`, `Alt+F4`, `Alt+Tab`, `Alt+Espace`, `Ctrl+Alt+Suppr`,
  `Win+L`).
- **Affichage** pour l'utilisateur : `Ctrl + Maj + 1`, `Impr. écran`.
- **Choix** (fonction pure) : pour chaque action, on parcourt les candidats ; on écarte un
  candidat invalide, un candidat `ImprEcran` quand Windows le réserve à l'Outil Capture
  (`systeme::impr_ecran_reserve()`), un candidat déjà pris dans cette session par une autre
  action, et un candidat que le système refuse. Le bilan dit, par action, le raccourci retenu
  (ou aucun) et pourquoi chaque candidat écarté l'a été. Une liste vide veut dire « pas de
  raccourci voulu » : ni raccourci ni message.
- **Seules les actions qui existent sont réservées.** L'Application tient la liste des actions
  dont la capture est construite (`ACTIONS_CONSTRUITES` : `zone` à l'étape 3, `plein_ecran`
  à l'étape 7, `fenetre` quand son étape arrive) et ne donne que celles-là au choix. BreezyShot ne prend jamais à une autre
  application un raccourci qui ne ferait rien.
- Le module ne connaît pas le type des réglages : l'Application lui passe une table
  `action → candidats` construite à partir d'eux.
- **`systeme::impr_ecran_reserve()`** (Windows) : valeur `PrintScreenKeyForSnippingEnabled` de
  `HKCU\Control Panel\Keyboard`. Présente : réservée si différente de 0. Absente : réservée
  si la version de Windows est 22631 (23H2) ou plus, sinon libre. Hors Windows : libre.
- **Message au démarrage** (bandeau, genre « info »), seulement si le premier candidat
  d'une action n'a pas été retenu. **Un seul message par démarrage** (le bandeau n'en montre
  qu'un à la fois) : d'abord une action restée sans raccourci, sinon la première action, dans
  l'ordre zone, plein écran, fenêtre, dont le premier candidat n'a pas été retenu. Un candidat
  recopié dans un message est coupé à 40 caractères. **Une seule fois par situation** : l'Application garde
  la dernière situation annoncée (action, candidat écarté, raison) dans `<local>/etat.json`
  et ne réaffiche le message que si elle change. Quand tout est normal, elle l'efface.
  La décision « annoncer ou non » est une fonction pure, testée.
  Textes (une ligne de titre, détail vide, aucune astuce ; `X` = affichage du premier
  candidat, coupé à 40 caractères, `Y` = affichage du raccourci retenu). La raison précise
  (réservée par Windows, prise par une autre application…) reste dans le journal et dans
  `Situation.raison`, pas dans le message :
  - Impr. écran réservée par Windows ou refusée par le système : « Impr. écran est prise.
    Raccourci : Y » ;
  - autre candidat refusé par le système ou déjà pris par une autre action : « X est pris.
    Raccourci : Y » ;
  - candidat invalide : « « X » n'est pas un raccourci valide. Raccourci : Y » ;
  - aucun candidat libre (genre « erreur », reste jusqu'à la croix) : « Aucun raccourci
    disponible », sans détail.
- **Signal** : `Signal::Presse(action)` et `Signal::Echap`.
- **Échap global** : `reserver_echap(app, rappel)` le réserve le temps d'une sélection,
  `liberer_echap(app)` le rend. Le plugin fait la réservation sur le fil principal et
  l'attend.
- **Réservation** (`raccourcis/reservation.rs`, seule partie qui touche au plugin
  `tauri-plugin-global-shortcut`) : `essayer` = `vers_systeme` puis `on_shortcut` ; un appui
  (état « pressé » seulement) est traduit par `Bilan::action_de`. Tout est libéré à l'arrêt
  (`unregister_all`).
- Lignes du journal (contrat des essais) : `raccourci zone: <forme canonique>` ou
  `raccourci zone: aucun`, `raccourci zone ecarte: <candidat> (<raison>)` par candidat écarté,
  `raccourci presse: zone`, `raccourcis liberes`, `annonce raccourci: <raison>` (code de la
  situation : `reserve_par_le_systeme`, `refuse_par_le_systeme`…) ou
  `annonce raccourci: deja faite`, rien si tout est normal. `etat.json` :
  `{ "annonce_raccourci": { "action", "candidat", "raison" } }`, les autres champs gardés.
- À l'appui, l'Application appelle `lancer_capture(mode)` (§3.6).

## 6. Fenêtres et bandeau (`fenetres/`, `interface/vignette/bandeau.*`)

- Le bandeau est une fenêtre préparée au démarrage et cachée : transparente, sans bordure,
  sans ombre système, toujours au-dessus, absente de la barre des tâches, **ne prend jamais le
  focus ni le clavier**, exclue des captures d'écran. Elle laisse passer tous les clics, sauf
  sur la pilule d'un message qui porte une croix (voir plus bas).
- **Un message tient en une ligne de titre ; seule l'erreur peut porter une cause en seconde
  ligne ; aucune astuce.** Le `detail` est vide pour une réussite et pour une info.
- `fenetres::Bandeau::afficher(Message { genre, titre, detail, actions })`, genres `Reussite`,
  `Info`, `Erreur`. `actions` est une liste `{ id, libelle }` fournie par l'Application
  (souvent vide). Un nouveau message remplace l'ancien tout de suite.
- `Bandeau::cacher()` (fil principal) retire tout de suite le message en cours et cache la
  fenêtre ; il rend vrai s'il y avait un message. Sert avant une capture.
- **Arguments WebView2** : toutes les fenêtres reçoivent la même constante
  `fenetres::ARGUMENTS_NAVIGATEUR` (`additional_browser_args`), le bandeau comme la
  sélection : WebView2 refuse une seconde fenêtre aux arguments différents. Ce sont ceux
  de wry par défaut, plus `CalculateNativeWinOcclusion` coupé (une page cachée reste vivante
  et se repeint).
- Place : centré en haut de la zone utile de l'écran où se trouve la souris, à 64 px du haut
  (sous la future barre de capture). Position calculée en pixels réels avec l'échelle de cet
  écran. La fenêtre mesure 560 × 220 pixels logiques (pilule de 460 × 108 au plus et place
  de son ombre), collée en haut de la zone utile ; la page pose la pilule à 64 px.
- **Sans focus, sous Windows** : la fenêtre est créée `focused(false)` et `focusable(false)`
  (tao pose `WS_EX_NOACTIVATE`). Elle n'est jamais montrée par `show()` de Tauri, car tao y
  appelle `ShowWindow(SW_SHOW)` qui active la fenêtre : `systeme::montrer_sans_activer`
  (`SW_SHOWNOACTIVATE`). tao croit donc la fenêtre cachée, et son `hide()` ne fait rien
  (`apply_diff` sort quand aucun drapeau ne change) : on cache par `systeme::cacher_sans_tao`
  (`SW_HIDE`). Pour la même raison, les clics traversants passent par
  `systeme::traverser_clics` (mêmes styles que tao, posés sans lui faire réappliquer les
  styles). À la création, `systeme::preparer_fenetre_outil` retire `WS_EX_APPWINDOW` (posé
  par tao pour une fenêtre sans parent) et pose `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE` : ni
  barre des tâches ni Alt+Tab. Le HWND est écrit au journal (`bandeau fenetre <hwnd>`).
  L'exclusion des captures est relue auprès du système et écrite au journal.
- **Verrous et fils** : l'état du bandeau n'est jamais verrouillé pendant un appel à Tauri
  ou un `emit` ; `afficher` peut venir de n'importe quel fil, les gestes sur la fenêtre
  partent sur le fil principal. Un geste « cliquable » posté par la surveillance relit
  l'état : il ne s'applique qu'au message en cours, s'il porte une croix.
- **Surveillance** : position de la souris par `systeme` (`GetCursorPos`), place et échelle
  de la fenêtre relues à chaque tour par `systeme::rectangle_fenetre`, avec le HWND gardé
  dans `Bandeau` à la création (aucune méthode de fenêtre Tauri sur ce fil : chacune attend
  le fil principal) ; toutes les 30 ms quand la souris est à moins de 300 px réels de la
  pilule, toutes les 60 ms sinon. Elle s'arrête quand le message n'est
  plus en cours, à l'arrêt de l'app et à la destruction de la fenêtre.
- **Filet de sécurité** (l'app ne gêne jamais l'écran si la page meurt), décision pure
  `cacher_d_office` : une réussite non terminée après 5 s (`DELAI_REUSSITE_MAX`) est
  cachée d'office ; un message à croix sans `bandeau_zone` valide 3 s après l'affichage
  (`DELAI_ZONE_MAX`) aussi (`bandeau cache d'office <id>`). Les délais partent de l'envoi à
  une page prête, pas de la mise en attente (premier lancement de WebView2 lent). Un
  `bandeau_pret` répété renvoie le message en cours, en oubliant la zone et le signal du
  précédent envoi (`EnCours::renvoyer`) : le délai de 3 s repart. `bandeau_zone` refuse une valeur non finie et borne le
  rectangle à la fenêtre (0..560 × 0..220 pixels CSS).
- **Annonce retenue une fois montrée** : la situation n'est écrite dans `etat.json` qu'au
  premier `bandeau_zone` valide de ce message (`fenetres::Signal::MessageMontre`, ligne
  `annonce montree <id>: situation retenue`), pas à l'envoi.
- **Page prête** : la page appelle `bandeau_pret {}` une fois son écoute de
  `bandeau:afficher` en place ; un message demandé avant attend ce moment. L'annonce du
  démarrage n'est jamais demandée depuis `setup` : `demarrer` la poste
  (`Evenement::AnnonceDemarrage`) pour un tour de la boucle déjà lancée. Depuis `setup`,
  la création de la sélection (WebView2) fait tourner une boucle imbriquée où
  `bandeau_pret` arrive ; un message montré là n'a jamais reçu son `bandeau_zone`
  (essai 03 (b), caché d'office au bout de 3 s).
- Aspect, d'après la maquette complète (https://claude.ai/artifact/AeUuBXPRC8U1AMiqg8dhTe) :
  fond opaque `acrylic-solid`, bordure `acrylic-stroke`, ombre `shadow-28`, pastille ronde
  de 20 px. Suit le thème clair ou sombre de Windows.
  - **Réussite** : pilule sur une ligne (rayon `radius-pill`), coche sur `success`, titre seul,
    coupé par « … » s'il est trop long. Part seule après 2,5 s.
  - **Erreur** (variante A choisie le 11/11) : pilule élargie (rayon 16 px, 460 px au plus),
    « ! » sur `danger`, titre, cause en seconde ligne seulement si elle est utile, boutons d'action éventuels et une
    croix. Reste affichée jusqu'à un clic sur la croix ou sur une action.
  - **Info** (forme de l'erreur A, validée par le fil design le 11/11) : « i » sur `accent`,
    titre seul (sans détail), pas de bouton d'action, reste affichée jusqu'à un clic sur la croix.
  - Titre sur 2 lignes au plus, cause sur 3 lignes au plus : la pilule a une hauteur maximale
    connue, la croix reste toujours visible.
  - Les textes d'un message sont toujours posés comme du texte, jamais comme du HTML. La page
    refuse un message sans `id`, sans genre connu ou sans titre.
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
- Lignes du journal (contrat des essais) : `bandeau pret`, `bandeau affiche <id> <genre>
  ecran <x>,<y> <largeur>x<hauteur> echelle <e>`, `bandeau zone <id> <x>,<y>
  <largeur>x<hauteur>` (pixels réels d'écran), `bandeau action <id> <action>`,
  `bandeau termine <id>`, `bandeau exclu des captures: oui|non`.
- Droits : `capabilities/bandeau.json` ne vise que la fenêtre `bandeau` et ne permet que
  l'écoute d'événements et les quatre commandes du bandeau (déclarées dans `build.rs`).
- La page prévient le cœur quand le message a fini de disparaître (`bandeau_termine { id }`) ;
  le cœur cache alors la fenêtre et arrête de surveiller la souris. Un message remplacé par un
  autre ne reçoit pas de `bandeau_termine` : pour le cœur, afficher un message termine le
  précédent. Une action ne part qu'une fois par message.
- Les pages n'utilisent ni bundler ni framework : HTML, CSS et modules JavaScript simples,
  `window.__TAURI__` exposé. Les couleurs, durées et courbes viennent de `commun/jetons.css`,
  généré à partir de `design/jetons.json` ; aucune valeur du design system n'est recopiée à
  la main dans une page.

## 7. Vérification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` : sur Windows (machine
  GitHub), et un `cargo check` + `cargo test` sur macOS pour garder le Mac ouvert.
- `node outils/generer-jetons.mjs --verifier` : `jetons.css` est à jour. `node
  essais/pages/bandeau.cjs` : la page du bandeau dans Chromium, servie avec la CSP de l'app
  (job `pages`, Linux).
- Essai 03 sous Windows, en plus des raccourcis : style étendu du bandeau (outil, sans
  `APPWINDOW`, sans activation), `IsWindowVisible` faux après `bandeau termine`,
  `WindowFromPoint` à côté de la pilule (pas le bandeau) et sur la croix (le bandeau),
  copie d'écran de la pilule exclue des captures (rien) et, avec
  `BREEZYSHOT_ESSAI_CAPTURABLE=1`, peinte (fond `acrylic-solid` à ±6 par canal ;
  `bandeau-windows.png` et `bandeau-windows-large.png` dans les résultats).
- Essai 04 sous Windows (`04-zone.ps1`), sur le .exe avec `BREEZYSHOT_ESSAI_CAPTURABLE=1`
  et une fenêtre témoin (`temoin.ps1`, quatre carrés de couleurs connues) : chronos,
  fenêtres pendant la sélection (couverture, ordre, premier plan, styles étendus), glisser
  dans les quatre sens comparé pixel par pixel (presse-papiers lu dans un fil STA, PNG de
  « Récents », fichier enregistré), Échap, clic droit, clic simple, F5, Ctrl+P, Alt+F4,
  perte de focus, second appui ignoré, deux captures rapprochées sans le bandeau, dossier
  impossible, fenêtres et fils stables, arrêt propre. Plusieurs écrans, échelles mêlées,
  application figée et verrouillage réel ne se vérifient pas sur la machine GitHub.
- Essai 05 sous Windows (`05-plein-ecran.ps1`), même cadre : `Ctrl + Maj + 2`, lignes
  d'état et `plein ecran … souris`, écran figé et sélection jamais visibles (guettés toutes
  les 2 ms pendant et après), PNG enregistré et presse-papiers à la taille exacte de l'écran
  principal avec le témoin à sa place, zone du bandeau identique dans deux captures de
  suite, second appui ignoré, réglages non réécrits, arrêt propre. Le choix entre plusieurs
  écrans (tests purs seulement) et le délai du menu (aucun clic sur l'icône) ne s'y
  vérifient pas.
- `essais/` : un script par étape (`01-socle.ps1`, `02-reglages.ps1`, `03-raccourci.ps1`,
  `04-zone.ps1`, `05-plein-ecran.ps1`),
  lancés par `essais/lancer.ps1` sur le .exe de release. Chaque essai démarre l'app avec
  `BREEZYSHOT_DOSSIER_ESSAI`, agit (lancer une seconde instance, simuler une touche, abîmer un
  fichier), puis lit le journal et les fichiers. Un essai qui échoue fait échouer GitHub.
- Chaque exécution note la mémoire privée au repos (l'app et ses processus de moteur web) dans
  le résumé GitHub. À partir de l'étape 5, elle note aussi le chrono de la sélection.
- Une étape n'est finie que relue par un agent distinct et inscrite dans `TRAVAIL-VALIDE.md`.

## 8. Gel (`systeme/gel/`, `capture/`, étape 4)

Décision de vitesse (11/11, mesurée chez l'utilisateur sur 3 écrans) : l'écran figé
affiché par une fenêtre native est visible en 87 ms de médiane, contre 634 ms par une page
web. Donc l'écran figé ne passe jamais par le moteur web.

- **Une photo, une fenêtre.** Un seul `BitBlt` de tout le bureau virtuel (`SRCCOPY |
  CAPTUREBLT`) dans une image GDI en mémoire, réutilisée tant que la taille du bureau ne
  change pas. Une seule fenêtre native couvre tout le bureau virtuel. Les pixels ne sont
  lus qu'à la découpe, et seulement pour la zone (`ImageFigee::lire` : `GetDIBits` 32 bits,
  BGRA → RGBA, alpha 255).
- **Façade** (`systeme::gel`, aucun `cfg` hors du choix du fichier) : `Rect` (pixels réels,
  bords droit et bas exclus), `EcranInfo`, `FenetreInfo`, `Pixels`, `ImageFigee` (opaque :
  `rect`, `duree_photo`, `lire`), `figer_bureau`, `ecrans`, `fenetres_visibles`,
  `preparer_ecran_fige`, `montrer_ecran_fige`, `cacher_ecran_fige`, `detruire_ecran_fige`,
  `poignee_ecran_fige`, `liberer_image`, `garder_ecran_fige`, `quand_bureau_change`
  (`Changement::Ecrans | SessionVerrouillee`), `surveillance_session`. Hors Windows :
  réponses neutres (`figer_bureau` dit « non disponible »).
- **Une seule image à la fois** : une `ImageFigee` dépassée par une photo plus récente
  refuse de se lire ou de se montrer. `figer_bureau` refuse tant que l'écran figé est
  montré (ou encore visible après un masquage posté) : on ne remplace jamais l'image
  affichée.
- **Fenêtre de l'écran figé** : classe `BreezyShotEcranFige`, `WS_POPUP`, `WS_EX_TOPMOST |
  WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`, sans fondu, fond jamais effacé,
  `WM_MOUSEACTIVATE` → `MA_NOACTIVATE`. Créée une fois, cachée, sur le fil principal.
  `montrer` (fil principal seulement) la pose sur le bureau et la peint tout de suite ; sans
  image à peindre, elle se cache. `cacher_ecran_fige` est sûr de partout (posté hors du
  fil principal). Elle se cache d'elle-même quand les écrans changent (`WM_DISPLAYCHANGE`)
  ou que la session se verrouille, puis appelle le rappel de `quand_bureau_change`.
- **Garde** : un fil vérifie toutes les 500 ms, tant que l'écran figé est montré, que son
  fil répond (`SendMessageTimeoutW(WM_NULL)`, 1 s) ; après le délai donné sans réponse, il
  appelle le rappel de `garder_ecran_fige`, une fois par blocage.
- **Capture** (`capture/`, sans code système) : `figer() -> Gel { image, bureau, ecrans,
  duree_photo, duree_totale }` (la liste des fenêtres se demande à part, `capture::fenetres`,
  pour le mode fenêtre) ; `decouper(&gel, zone) -> Pixels` ; géométrie pure :
  `rect_entre(a, b)` (les deux coins inclus : de (0, 0) à (1919, 1079), 1920 × 1080),
  `ecran_sous`, `borner_a_un_ecran(zone, depart, ecrans)` (une zone reste sur l'écran du
  départ), `COTE_MIN = 3`, `zone_valide`. Erreurs : `NonDisponible`, `Photo`, `ZoneVide`,
  `ZoneHorsBureau`, `Lecture`.
- Mémoire : l'image du bureau reste allouée entre deux captures (3 écrans 1080p = 24 Mo),
  rendue à l'arrêt (`liberer_image`). Aucune écriture sur le disque, aucune image au journal.
- Tests Windows (`cargo test` sur la machine GitHub, un par un) : photo au pixel près d'une
  fenêtre de quatre carrés, écran figé montré sans prendre le premier plan puis caché,
  200 photos sans fuite d'objets GDI.

## 9. Sélection (`fenetres/selection.rs`, `interface/selection/`, étape 5)

Ce que voit l'utilisateur : tous les écrans figés et assombris ; il glisse, la zone redevient
claire, entourée d'un trait fin ; il relâche, tout disparaît d'un coup. Rien d'autre : ni
viseur, ni taille, ni poignées, ni texte, ni animation d'entrée. Curseur en croix. Maj :
carré. Espace tenue : la zone se déplace. Échap ou clic droit : annuler. Un clic, ou moins de
3 pixels réels de côté, n'est pas une zone : la sélection continue. La loupe n'est pas
construite (le réglage existe, sans effet).

- **Une fenêtre Tauri transparente**, préparée au démarrage et cachée, posée juste au-dessus
  de l'écran figé et couvrant tout le bureau virtuel. Elle ne dessine que le voile (jeton
  `--scrim`) et le trait (1 px CSS `--selection-stroke`, à l'intérieur). Elle ne reçoit
  jamais les pixels. Sans bordure, sans ombre, toujours au-dessus, absente de la barre des
  tâches et d'Alt+Tab (`WS_EX_TOOLWINDOW`, sans `WS_EX_APPWINDOW`), exclue des captures
  (sauf essai). Contrairement au bandeau, **elle prend le focus** : il faut le clavier pour
  Maj et Espace. Journal : `selection fenetre <hwnd>`.
- **Interface** : `Selection::creer(app, rappel)`, `montrer(Demande { bureau, ecrans,
  dessous, cote_min }) -> Result<id>` (fil principal), `cacher()` (sûr de partout, sans
  signal), `visible()`, `fermer()`. `Signal` : `Prete { id, duree }`, `Zone { id, depart,
  arrivee }` (pixels réels du bureau), `Annulee { id, cause }`, `Muette { id }`,
  `Impossible { id, cause }`. Quatre commandes, permises à la seule fenêtre `selection`
  (`capabilities/selection.json`) : `selection_chargee`, `selection_prete`,
  `selection_terminee`, `selection_annulee`.
- **Montrer**, sans rien attendre de la page : noter la fenêtre au premier plan ; poser la
  fenêtre sur le bureau, la montrer, lui donner le premier plan (par `systeme` sous Windows,
  jamais par tao, qui réécrirait les styles) ; placer l'écran figé juste en dessous ;
  relire le rectangle réel et le reposer tant qu'il diffère, deux fois au plus
  (`PLACEMENTS_MAX = 3` poses en tout ; `selection montree <id>
  voulu <x,y,l,h> reel <x,y,l,h> echelle <e>`) ; envoyer la charge `selection:montrer { id,
  numero, largeur, hauteur, ecrans, cote_min }`, tout en pixels réels relatifs à la fenêtre ;
  armer le délai de 2 s (`DELAI_PRETE`).
- **Une seule échelle, celle de la page.** Rust ne parle à la page qu'en pixels réels. La
  page retient `devicePixelRatio` s'il explique la taille réelle à 1 pixel CSS près, sinon
  elle mesure (`visualViewport`, à défaut `innerWidth`), et répond `selection_prete { id,
  numero, largeur, hauteur, echelle, kx, ky, attente }`. `controler_page` refuse une échelle
  non finie, hors de [0,5 ; 5], différente de plus de 2 % entre les axes, ou qui n'explique
  pas la taille réelle. Les coins de `selection_terminee` arrivent en pixels réels.
- **Numéro de charge** : chaque nouvelle charge (premier envoi, fenêtre relue différente)
  prend le numéro suivant, que la page renvoie partout. Ancien numéro : `prete` non
  contrôlée, `terminee` ignorée, `annulee` gardée. Après un `resize`, la page n'est plus
  prête et le dit (`attente`) : elle reçoit toujours une charge, de même numéro si le
  rectangle réel n'a pas changé.
- **Un seul signal de fin par affichage** (le premier gagne ; un `id` qui n'est pas celui
  en cours est ignoré). Avant le rappel d'un signal de fin, le module rend le premier plan,
  cache sa fenêtre et prévient la page (`selection:cachee`) ; le rappel est appelé sur le
  fil principal, dans le même tour. Perte du focus après l'avoir eu : `Annulee`. Pas de
  `selection_prete` en 2 s : `Muette`. Donnée invalide de la page : `Impossible`.
- **Données de la page jamais crues** : `vers_bureau` (pixel de la fenêtre → pixel du
  bureau, `None` si non fini ou hors de la fenêtre), `vers_page`, `controler_page`,
  `decider_zone`, `cause_depuis_nom`, testées.
- Journal : `selection chargee`, `selection montree …`, `chrono selection prete <ms> page
  <l>x<h> echelle <kx> page-dpr <d> tao <s>`, `selection fin <id> zone|annulee
  <cause>|muette|impossible <cause>`.
- L'Application réserve Échap en global pendant la sélection (§3.6) : la page n'a pas
  besoin du focus pour être annulable.

## 10. Sortie (`sortie/`, étape 6)

Tout ce qui quitte l'app passe par une seule fonction, `sortir(&Pixels, &Demande) -> Bilan`.
« Récents » n'est lu et écrit que par `sortie`. Le module n'écrit jamais au journal : il rend
son `Bilan`, et `annonce(&Bilan, &Demande)` dit quoi montrer.

- `Demande { copier, enregistrer, dossier_enregistrement, dossier_affiche, dossier_recents,
  modele_nom, maintenant }`, chemins absolus résolus par l'Application. `Bilan { copie:
  Faite | NonDemandee | Echec, recent: Result<chemin>, enregistrement: Fait | NonDemande |
  Echec { dossier, cause } }`.
- **Ordre** : valider les pixels (sinon tout en échec, sans panique) ; la copie part dans
  un fil à part ; pendant ce temps, encoder le PNG une fois, écrire « Récents », enregistrer
  si demandé. Une étape qui échoue n'empêche pas les suivantes. `sortir` bloque : jamais sur
  le fil principal.
- **Presse-papiers** : crate `arboard` (Windows, macOS, Linux) ; occupé : on réessaie
  pendant 250 ms au plus, puis échec avec sa cause.
- **PNG** : crate `png`, RGBA 8 bits, alpha 255, sans métadonnées, compression rapide.
- **Fichiers** : écrits dans un temporaire du même dossier, puis lien dur (jamais
  d'écrasement ; repli sur réservation et renommage). Dossier créé s'il manque. Dossier
  vide ou relatif : échec, rien d'écrit. Restes `.breezyshot-*.tmp` de plus d'une heure
  supprimés.
- **Nom** : `{date}` → `2026-11-11`, `{heure}` → `19.40.12` ; caractères interdits sous
  Windows remplacés par `-` ; pas d'espace ni de point à la fin ; nom vide ou réservé
  (`CON`, `NUL`, `COM1`…) → `BreezyShot <date> <heure>` ; 120 caractères et 200 octets au
  plus ; collision : ` (2)`, ` (3)`…
- **Récents** : `<local>/Recents/AAAAMMJJ-HHMMSS-mmm.png` (heure UTC, `_2` en cas de
  collision) ; on garde les 50 plus récents (`RECENTS_GARDES`) ; le nettoyage ne touche
  qu'aux noms de ce format exact, jamais au fichier qu'on vient d'écrire.
- **Annonces** (une ligne de titre ; seule une erreur a une cause) : « Copié dans le
  presse-papiers » ; « Enregistré dans <dossier affiché> » ; « Copié et enregistré » ;
  enregistrement en échec : « Impossible d'enregistrer » / cause en clair ; copie en échec
  seule : « Impossible de copier » / cause ; copie ratée mais fichier enregistré :
  « Impossible de copier » / « La capture est enregistrée dans <dossier affiché>. » ; rien
  demandé : « Capture prise ». Aujourd'hui, « Capture prise » n'arrive que si copie et
  enregistrement sont éteints et la vignette (pas encore construite) allumée : les réglages
  ne rallument la copie que si les trois sont éteints. Seul « Récents » en échec : la réussite est annoncée, l'échec
  va au journal (par l'Application).
- `sortie` n'importe que `crate::systeme` (`Pixels` vient de `systeme::gel`).
