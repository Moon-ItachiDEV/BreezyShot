Version d'essai, non signée : Windows peut demander une confirmation au premier lancement.

Ouvrez le fichier téléchargé, puis Installer. Pour désinstaller : Paramètres › Applications
installées.

- Zone : Impr. écran (ou Ctrl + Maj + 1), glisser, relâcher : la zone est dans le presse-papiers.
- Écran : Ctrl + Maj + 2 : tout l'écran sous la souris est dans le presse-papiers.
- Fenêtre : Ctrl + Maj + 3, clic sur la fenêtre entourée de bleu : elle est dans le presse-papiers, coins arrondis compris.
- Refaire la dernière zone : Ctrl + Maj + 4 (ou le menu de l'icône) : la même zone, sans
  rien tracer. Sans zone depuis le lancement, ou si l'écran a changé, la sélection s'ouvre.

Enregistrer dans Images › BreezyShot : bouton « Enregistrer » de la vignette, ou par le réglage.

Annoter : bouton « Annoter » de la vignette. Flèche, stylo, surligneur, gomme ; « + » :
texte, forme, numéro, flou, projecteur. Recadrer : tirer un coin de l'image. « Copier »
copie et ferme ; « … » : Enregistrer sous, Glisser vers une appli.

Échap ou clic droit annule. L'icône près de l'horloge (clic gauche ou droit) ouvre le menu,
d'où Réglages et Quitter. Relancer BreezyShot (icône épinglée à la barre des tâches) ouvre la
capture de zone, comme Impr. écran.

Réglages › Capture : retardateur (3, 5 ou 10 s ; Échap ou la croix l'annule), Inclure le
pointeur dans l'image.

Réglages (menu de l'icône, « Réglages ») : rubriques dans le volet de gauche. Copie, dossier,
nom des fichiers, raccourcis (cliquer puis taper le nouveau), thème, lancement au démarrage de
Windows. Raccourcis › « Impr. écran ouvre BreezyShot » : la touche Impr. écran lance BreezyShot
au lieu de l'Outil Capture de Windows. Tout s'applique tout de suite.

Licence (0.5) : Réglages › Général, dernière ligne « Licence » : coller la licence, puis
« Activer ». La licence d'essai est dans le fichier `essais/licence-essai.txt`. La ligne dit
alors « Pro activé » ; « Retirer » l'enlève. Sans redémarrer. Sans licence, l'app est celle
de la 0.4.

Avec la licence :

- Historique : Ctrl + Maj + H (ou « Historique » dans le menu de l'icône) : les 50 dernières
  captures. Copier, Enregistrer, Annoter, Supprimer (Ctrl + Z l'annule pendant 5 s),
  glisser une capture vers une appli ; « Plus d'options » : Tout effacer.
- Copier le texte : Ctrl + Maj + T (ou le menu de l'icône), glisser autour du texte,
  relâcher : le texte est dans le presse-papiers. Dans l'éditeur : bouton « Texte », choisir
  les mots, « Copier » ou « Tout copier ». Le texte est lu dans les langues installées dans
  Windows ; sans aucune, Copier le texte n'apparaît pas.

Essai 13 (Windows) : licence absente, refusée, activée par un vrai clic sur « Activer »,
puis retirée ; Copier le texte sur un témoin anglais en Segoe UI 20 px (`temoin-texte.ps1`),
presse-papiers exact, zone blanche, « Réessayer » ; historique ouvert par Ctrl + Maj + H et
par le menu : Copier, Enregistrer, Supprimer et Ctrl + Z, Annoter et « Texte » dans
l'éditeur, glisser, Tout effacer, trois fermetures, mémoire sur 20 ouvertures ; migration
de la 0.4 ; machine sans langue (`BREEZYSHOT_ESSAI_SANS_TEXTE`). La langue en-US doit être
présente sur la machine : sans elle, l'essai échoue.

Essai 14 (Windows, 900 s au plus) : capture défilante sur un témoin WPF dont chaque rangée
de pixels porte son numéro (`temoin-defilant.ps1`) ; l'image finale est relue rangée par
rangée (chaque rangée une fois, dans l'ordre) et comparée au pixel à la page de référence
que le témoin écrit en BMP. Sans licence : rien de la défilante, Ctrl + Maj + 5 libre.
Avec : lancée par Ctrl + Maj + 5 et par le menu, clic sur la page après la liste des pages, fin
seule en bas, presse-papiers exact, budgets (départ, mesure, fin, annonce, mémoire ; vitesse
écrite, en échec sous 800 px/s, cible 1 500) ; molette qui reprend la main, « Trop vite »,
Entrée ; Échap, appuis ignorés, « Terminer », en-tête collant, petite zone agrandie, limite
de 16 384 px, `--quitter` ; fenêtre de contrôle absente des copies d'écran (sentinelle),
voile vu en cas capturable. Le focus de la sélection pendant la création de la fenêtre de
contrôle est dit par les lignes `[focus]`. Échap et Entrée partent en raccourci global, une
fois leur réservation au journal ; un point en échec ramène l'app au repos (lignes
`[repos]`). Edge et l'Explorateur sont joués et notés seulement ; les suites d'images pour
l'assemblage (résumés de lignes `resume …`, paires `suite … paire`) et les lignes `vivant:`
des tests sont dans la sortie.

Le journal : `%LOCALAPPDATA%\BreezyShot\Journal\breezyshot.log`.
