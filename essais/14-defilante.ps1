# Version 0.6 « Capture défilante » (contrat 0.6, §8, §9, §16.3 et §18) : la défilante de
# bout en bout sur le vrai .exe, avec de vraies entrées clavier et souris, sur un témoin WPF
# (temoin-defilant.ps1) dont chaque rangée de pixels porte son numéro : l'image finale se
# relit rangée par rangée, sans tolérance sur l'ordre (aucun doublon, aucun trou).
# Chaque attente lit une ligne nouvelle du journal (ou guette une fenêtre, un élément UI
# Automation, la position écrite par le témoin) avec une borne, et dit ce qu'elle attendait.
# Garde (essais 12 et 13) : un appui de souris ne part que si le point visé est sur une
# fenêtre de l'app ; une touche n'est qu'un raccourci global que l'app tient (Entrée et
# Échap seulement après `entree globale reservee` / `echap global reserve` sans libération
# depuis : la touche va à l'app quel que soit le premier plan, comme dans l'essai 13) ; un
# cran de molette seulement si le point est sur une fenêtre permise (témoin, Edge,
# Explorateur de l'essai), jamais sur l'app ni sur une console. Ctrl+Maj+5, tant que l'app
# ne le tient pas, n'est jamais tapé : l'essai le tient lui-même.
# Un point qui échoue ramène l'app au repos (Échap global si la sélection ou la course le
# tient, sinon l'app relancée) et le témoin en haut, pour que les suivants restent valables.
#
# L0, sans licence (fichier de réglages v4) : `licence absente` ; aucune ligne `raccourci
#     defilante` ; menu web de l'icône de 9 positions (séparateurs comptés) sans « Capture
#     défilante » ; Ctrl+Maj+5 libre ;
#     `--capturer defilante` → `--capturer: mode non construit` ; fichier passé en v5
#     (`defilante` = Ctrl+Maj+5, une ligne « de la version 4 a la version 5 ») ; aucune
#     fenêtre `defilante`. Mémoire au repos notée.
# Cas A (licence, non capturable : les fenêtres de l'app sont exclues des copies d'écran ;
#     copie, vignette « toujours » ; un témoin de 600 × 6 000 en 40,80 et une sentinelle) :
# (L1) `raccourci defilante: Ctrl+Maj+5` ; menu : « Capture défilante<tab>Ctrl + Maj + 5 »
#     à l'indice 5 (après les trois captures et un séparateur) ; aucune fenêtre `defilante`
#     au repos ; mémoire au repos notée.
# Le menu de l'icône est une fenêtre web (0.5.2) : ouvert, lu, cliqué et fermé par les aides
# de commun.ps1 (Ouvrir-Menu-Icone, Cliquer-Ligne-Menu, Fermer-Menu-Icone), comme l'essai 13.
# (A1) Ctrl+Maj+5 : `DemandeCapture defilante`, `selection montree … mode defilante`,
#     `chrono selection prete`, `defilante pages <n>`, `selection <id>: pages proposees <n>`
#     ou `selection <id>: pages retenues pour la premiere charge` (alors n est celui de
#     `defilante pages`, n ≥ 1), `defilante fenetre <h>` (ou `<h> retrouvee`, R1 « créée ou
#     retrouvée »), `defilante page prete` ; le premier plan relevé
#     toutes les 20 ms pendant la visée et dit dans la sortie (lignes [focus], noté
#     seulement : le terminal de la machine peut prendre le premier plan sans faute de
#     l'app) ; vérifié : aucune `annulee focus_perdu` ; le clic attend la liste (400 ms
#     après `pages proposees`) ; lignes `defilante pages` et `defiler pages:` écrites.
# (A2) clic réel au centre de la page : `defilante zone` = page du témoin (2 px près),
#     `page agrandie non` ; écran figé caché ≤ 450 ms après le relâcher (guetté toutes les
#     2 ms ; l'ordre « caché avant la première prise » noté, les deux horloges diffèrent) ;
#     fenêtre de contrôle : WS_EX_TOOLWINDOW, WS_EX_NOACTIVATE, WS_EX_TOPMOST, sans
#     WS_EX_APPWINDOW, traversée (WS_EX_TRANSPARENT) loin de la pilule, `exclue des
#     captures: oui` ; jamais au premier plan pendant la course.
# (A3) `defilante fin en_bas` sans aucune action, sans `main` ni `repli: molette` ; le
#     témoin au bas de sa page ; suite des moyens notée (`defilante repli: <nouveau>
#     (<ancien> sans effet|impossible|cran trop grand|trop loin)`) ; aucun pas sauté pour
#     une touche ou un bouton tenu (l'essai n'en tient aucun pendant l'auto).
# (A4) PNG de « Récents » 600 × 6 000, relu rangée par rangée (chaque rangée de la page une
#     fois, dans l'ordre) et égal au pixel (± 8 par canal) à la page de référence que le
#     témoin écrit en BMP ; presse-papiers égal au PNG au bit près ; `vignette entree` ;
#     fenêtre de contrôle cachée à la fin.
# (A5) la sentinelle copiée pendant la course, fenêtre de contrôle visible : moins de 2 %
#     des points différents d'avant (la fenêtre n'est pas dans les images) ; jamais défilée.
# (A9) budgets : départ ≤ 500 ms (page de contrôle prête avant le clic, sinon noté) ;
#     mesure médiane ≤ 15 ms ; fin → assemblée ≤ 150 ms (`fin-assemblee` de la ligne
#     `chrono`, depuis la fin reçue : §16.3) ; relâcher → annonce ≤ 1 500 ms ; vitesse
#     mesurée sur les positions du témoin, écrite, en échec sous 800 px/s seulement (1 500
#     px/s : la cible, notée) ; mémoire pendant la course et l'assemblage ≤ repos + 70 Mo +
#     2,5 × RGBA. Prise : la première notée (pas de médiane au journal) ; lignes `vivant:`.
# (E) lancée par le menu : `menu: capturer defilante` ; pendant la course Ctrl+Maj+2 et
#     Ctrl+Maj+5 : `raccourci presse: plein_ecran` puis `raccourci presse: defilante`, chacun
#     suivi de `raccourci ignore pendant la defilante` (R2 : rien ne s'ouvre sur la zone) ; Échap : `annulee echap`, `entree globale liberee`,
#     `echap global libere`, `Defilement -> Repos`, aucun PNG, aucune fenêtre de l'app visible.
#     Échap et Entrée partent en raccourci global, une fois leur réservation au journal.
# (C) la molette reprend la main : un cran de l'essai pendant l'auto → `main (molette)`,
#     puis la page ne bouge plus seule (1 s) ; trois crans lents (la page avance de trois
#     crans) ; 30 crans d'un coup → `trop vite`, message lu par UI Automation dans la
#     fenêtre de contrôle ; crans vers le haut un par un → `repris`, message caché ; Entrée
#     → `entree globale presse`, `fin entree … perdues ≥ 1` ; PNG relu exact du haut de la
#     page à la dernière image gardée, et égal à la référence (hauteur − 450 : une position
#     du témoin, entre celle d'avant la rafale et le haut de la rafale).
# (S) suites d'images pour le chantier C (l'essai fait défiler et copie l'écran) : par
#     rangée, le résumé de C (16 blocs), le gris du fond, l'encre et la porteuse, en texte
#     dans la sortie ; décalage vrai lu sur le témoin, score D12.
# Cas B (licence, BREEZYSHOT_ESSAI_CAPTURABLE=1) :
# (B1) témoin -Immobile tracé exactement, le pointeur laissé au centre de la page jusqu'au
#     repli final (la molette de l'app ne part que le pointeur sur la zone) : voile de la
#     fenêtre de contrôle vu sur la sentinelle (≥ 90 % des points non noirs changés :
#     contrôle positif de A5) ; `defilante repli: molette` seul (les trois moyens sans
#     effet ; « molette retenue, jamais sans effet » dit à part) ; clic réel sur « Terminer »
#     (UI Automation, souris vue sur la pilule) : `fin bouton`, `fin-assemblee` ≤ 150 ms, PNG
#     600 × 450 exact et égal à la référence, la page n'a pas bougé.
#     Avant « Terminer », un clic sur l'icône : `menu pendant la defilante: quitter
#     seulement`, le menu montre « Quitter BreezyShot » seul ; un second clic sur l'icône le
#     ferme (pas Échap : l'app le tient pendant la course) ; ni `menu: …`, ni issue, l'app
#     tourne encore.
# (B2) témoin -EnTete tracé sur toute la fenêtre : `fixes haut 40 bas 0`, PNG 600 × 6 040,
#     l'en-tête seulement sur les rangées 0 à 39, égal aux références (en-tête, page).
# (B3) zone tracée de 80 × 60 : `defilante zone` de 120 × 120 autour d'elle, `agrandie oui` ;
#     Échap : `annulee echap`.
# (B4) témoin -Lignes 400 (20 000 px) : des pages proposées (sinon un échec qui le dit, avec
#     la ligne `defiler pages:` ; sa durée notée avec sa marge sur 300 ms), `fin limite`,
#     PNG 600 × 16 384 exact et égal à la référence.
# (B5) --quitter pendant la course : `annulee arret`, `arret propre`.
# Cas N (Edge et l'Explorateur, R10) : joués et notés seulement, jamais en échec ; suites
#     d'images pour le chantier C.
# Fin de chaque course : aucune ligne WARN ou ERROR de la défilante (`defilante`, `defiler`,
# `entree globale`, `echap global`). Fin de chaque cas : aucune ligne `fin deja rendue`,
# `fil arrete (panique)`, `annulee focus_perdu`.
#
# Non vérifiable sur GitHub (un écran 1024 × 768 à 100 %) : 125 et 150 %, plusieurs écrans,
# un pavé tactile, l'allure, AZERTY. Non fait ici : le témoin sans UI Automation, Annoter
# depuis la vignette à la limite, dix défilantes de suite (voir le rapport du chantier H).
# Hors Windows : démarrage, `licence absente`, `--quitter`.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '14-defilante' -Exe $Exe -Sortie $Sortie | Out-Null

$script:Auxiliaires = [System.Collections.Generic.List[object]]::new()
$script:Pwsh = (Get-Process -Id $PID).Path
$script:ProfilEdge = $null
$script:proc = $null
# La licence d'essai, telle que rangée par le chantier L (une ligne, blancs retirés).
$LICENCE = (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'licence-essai.txt') -Raw).Trim()
$FORMAT_RECENT = '^\d{8}-\d{6}-\d{3}(_\d+)?\.png$'
$TEXTE_TROP_VITE = 'Trop vite. Remontez un peu.'

# Motifs du journal (contrat §8), par clé : les compteurs d'avant un geste permettent
# d'attendre « une ligne de plus ».
$MOTIFS = [ordered]@{
    Gel        = 'etat (Repos|Vignette) -> Gel \(DemandeCapture defilante\)$'
    Montree    = 'selection montree \d+ '
    Prete      = 'chrono selection prete \d+ '
    DPages     = 'defilante pages \d+ en \d+ ms$'
    Fenetre    = 'defilante fenetre \d+( retrouvee)?$'
    Exclue     = 'defilante exclue des captures: '
    PagePrete  = 'defilante page prete en \d+ ms$'
    Choisi     = 'etat Selection -> Defilement \(DefilementChoisi\)$'
    Zone       = 'defilante zone '
    Premiere   = 'defilante premiere image '
    Moyen      = 'defilante moyen '
    Repli      = 'defilante repli: '
    Main       = 'defilante main \(molette\)$'
    TropVite   = 'defilante trop vite$'
    Repris     = 'defilante repris$'
    Fixes      = 'defilante fixes haut '
    Issue      = 'defilante (fin |annulee |impossible: )'
    Assemblee  = 'defilante assemblee \d+x\d+ en \d+ ms$'
    Chrono     = 'chrono defilante relache-premiere '
    Decoupe    = 'chrono decoupe \d+ sortie \d+ relache-annonce \d+'
    Copie      = 'sortie copie '
    Fini       = 'etat Defilement -> Decoupe \(DefilementFini\)$'
    Annule     = 'etat Defilement -> Repos \(DefilementAnnule\)$'
    EntreeR    = 'entree globale reservee$'
    EntreeL    = 'entree globale liberee$'
    EntreeP    = 'entree globale presse$'
    EchapP     = 'echap global presse$'
    EchapL     = 'echap global libere$'
    EchapR     = 'echap global reserve$'
    DefPages   = 'defiler pages: '
    DefMesure  = 'defiler pages (detail|tardives|fin): '
    Avert      = '^\S+\s+(WARN|ERROR)\s+\S+\s+(defilante|defiler|entree globale|echap global)'
    Ignore     = 'raccourci ignore pendant la defilante$'
    Presse     = 'raccourci presse: '
    Vignette   = 'vignette entree \d+$'
    Interdites = 'defilante \d+: fin deja rendue|defilante: fil arrete \(panique\)|annulee focus_perdu'
}

# --- Aides (même forme que l'essai 13) ---------------------------------------------------

function Dossier-Cas([string]$Nom) {
    $d = Join-Path $script:Essai.Dossier $Nom
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    return $d
}

function Lignes-Journal([string]$Dossier) {
    return @((Lire-Journal -Dossier $Dossier) -split "`n" | ForEach-Object { $_.TrimEnd("`r") } | Where-Object { $_ -ne '' })
}

function Derniere-Ligne([string]$Motif, [string]$Dossier) {
    $trouvee = $null
    foreach ($l in (Lignes-Journal $Dossier)) { if ($l -match $Motif) { $trouvee = $l } }
    return $trouvee
}

function Compter-Motif([string]$Motif, [string]$Dossier) {
    $n = 0
    foreach ($l in (Lignes-Journal $Dossier)) { if ($l -match $Motif) { $n++ } }
    return $n
}

# Le message d'une ligne, sans horodatage, niveau ni module (ni chemin ni contenu : §8).
function Message([string]$Ligne) {
    if (-not $Ligne) { return '<aucune ligne>' }
    return ($Ligne -replace '^\S+\s+\S+\s+\S+\s+', '')
}

function Attendre-Motif([string]$Motif, [string]$Dossier, [double]$DelaiS, [string]$Quoi, [int]$Fois = 1) {
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        if ((Compter-Motif -Motif $Motif -Dossier $Dossier) -ge $Fois) {
            return (Derniere-Ligne -Motif $Motif -Dossier $Dossier)
        }
        Start-Sleep -Milliseconds 30
    } while ([DateTime]::UtcNow -lt $limite)
    $vu = Compter-Motif -Motif $Motif -Dossier $Dossier
    throw "$Quoi : rien dans le journal en $DelaiS s (motif « $Motif », attendu $Fois fois, vu $vu) ; $(Diagnostic-Journal $Dossier)"
}

function Attendre-Suivante([string]$Motif, [string]$Dossier, [int]$Deja, [string]$Quoi, [double]$DelaiS = 10) {
    return Attendre-Motif -Motif $Motif -Dossier $Dossier -DelaiS $DelaiS -Quoi $Quoi -Fois ($Deja + 1)
}

# Les 40 dernières lignes utiles, sans horodatage (ni chemin ni contenu : contrat §8).
function Diagnostic-Journal([string]$Dossier) {
    $l = @(Lignes-Journal $Dossier | Where-Object { $_ -match 'licence|defilante|defiler|selection|raccourci|menu|etat |entree|echap|sortie|vignette|capture|WARN|ERROR' } | Select-Object -Last 40 | ForEach-Object { Message $_ })
    if ($l.Count -eq 0) { return '(journal : aucune ligne utile)' }
    return 'journal : ' + ($l -join ' | ')
}

# L'heure d'une ligne du journal (premier mot, `2026-11-11T18:40:12.345+01:00`).
function Heure-Ligne([string]$Ligne) {
    $mot = ($Ligne -split '\s+')[0]
    return [DateTimeOffset]::Parse($mot, [Globalization.CultureInfo]::InvariantCulture)
}

# Compteurs de tous les motifs de $MOTIFS (une lecture du journal).
function Compteurs([string]$Dossier) {
    $lignes = Lignes-Journal $Dossier
    $c = @{}
    foreach ($k in $MOTIFS.Keys) {
        $n = 0
        foreach ($l in $lignes) { if ($l -match $MOTIFS[$k]) { $n++ } }
        $c[$k] = $n
    }
    return $c
}
# Les lignes du motif $Cle venues après les compteurs $Base.
function Apres([string]$Dossier, [string]$Cle, $Base) {
    $t = @(Lignes-Journal $Dossier | Where-Object { $_ -match $MOTIFS[$Cle] })
    if ($t.Count -le $Base[$Cle]) { return @() }
    return @($t[$Base[$Cle]..($t.Count - 1)])
}
function Attendre-Cle([string]$Dossier, [string]$Cle, $Base, [string]$Quoi, [double]$DelaiS = 10) {
    return Attendre-Motif -Motif $MOTIFS[$Cle] -Dossier $Dossier -DelaiS $DelaiS -Quoi $Quoi -Fois ($Base[$Cle] + 1)
}
# Un nombre lu dans une ligne par une expression à un groupe ; -1 si absent.
function Nombre([string]$Ligne, [string]$Motif) {
    if ($Ligne -and $Ligne -match $Motif) { return [int]$Matches[1] }
    return -1
}

function Fichiers-Format([string]$Dossier) {
    if (-not (Test-Path -LiteralPath $Dossier)) { return @() }
    return @(Get-ChildItem -LiteralPath $Dossier -File | Where-Object { $_.Name -match $FORMAT_RECENT } | Sort-Object Name)
}
function Recents([string]$Dossier) { return @(Fichiers-Format (Join-Path $Dossier 'local/Recents') | ForEach-Object { $_.FullName }) }

# Réglages en version 4 (ceux de la 0.5) : l'app les passe en version 5 au démarrage.
function Ecrire-Reglages([string]$Dossier, [bool]$Vignette) {
    $r = [ordered]@{
        version                   = 4
        raccourcis                = [ordered]@{
            zone = @('Ctrl+Maj+1'); plein_ecran = @('Ctrl+Maj+2'); fenetre = @(); derniere_zone = @()
            historique = @('Ctrl+Maj+H'); texte = @('Ctrl+Maj+T')
        }
        copier_apres_capture      = $true
        enregistrer_apres_capture = $false
        dossier_enregistrement    = (Join-Path $Dossier 'images')
        vignette                  = [ordered]@{ afficher = $Vignette; duree_s = 0; coin = 'bas_droite' }
        animations                = 'reduites'
        selection                 = [ordered]@{ loupe = $false }
        capture                   = [ordered]@{ retardateur_s = 0; pointeur = $false }
        demarrer_avec_systeme     = $false
        modele_nom                = 'BreezyShot {date} à {heure}'
        theme                     = 'systeme'
    }
    $d = Join-Path $Dossier 'reglages'
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $Dossier 'images') | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $d 'reglages.json'), ($r | ConvertTo-Json -Depth 5), [System.Text.UTF8Encoding]::new($false))
}
function Lire-Reglages([string]$Dossier) {
    return (Get-Content -LiteralPath (Join-Path $Dossier 'reglages/reglages.json') -Raw -Encoding utf8 | ConvertFrom-Json)
}
# licence.txt avant le démarrage, comme le range le chantier L (UTF-8 sans BOM).
function Poser-Licence([string]$Dossier) {
    New-Item -ItemType Directory -Force -Path (Join-Path $Dossier 'reglages') | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $Dossier 'reglages/licence.txt'), $LICENCE, [System.Text.UTF8Encoding]::new($false))
}

function Demarrer-Cas([string]$Dossier, [string]$Quoi, [switch]$Capturable, [switch]$AvecLicence) {
    if ($Capturable) { $env:BREEZYSHOT_ESSAI_CAPTURABLE = '1' }
    $prets = Compter-Motif -Motif 'application +pret$' -Dossier $Dossier
    try { $proc = Demarrer-App -Dossier $Dossier }
    finally { Remove-Item Env:BREEZYSHOT_ESSAI_CAPTURABLE -ErrorAction SilentlyContinue }
    if ($IsWindows) { [BreezyShotEssai.Defilante]::PidApp = $proc.Id }
    $null = Attendre-Motif -Motif 'application +pret$' -Dossier $Dossier -DelaiS 30 -Quoi "$Quoi : démarrage" -Fois ($prets + 1)
    if ($AvecLicence) {
        $null = Attendre-Motif -Motif 'licence active ' -Dossier $Dossier -DelaiS 10 -Quoi "$Quoi : licence active"
    }
    return $proc
}

function Fermer-Cas($Processus, [string]$Dossier, [string]$Quoi) {
    $arrets = Compter-Motif -Motif 'arret propre$' -Dossier $Dossier
    $q = Quitter-App -DelaiS 15 -Dossier $Dossier
    if (-not $q.Fini) { throw "$Quoi : --quitter n'a pas répondu en 15 s" }
    if (-not $Processus.WaitForExit(15000)) { throw "$Quoi : l'app ne s'est pas fermée en 15 s" }
    $null = Attendre-Motif -Motif 'arret propre$' -Dossier $Dossier -DelaiS 3 -Quoi "$Quoi : arrêt" -Fois ($arrets + 1)
}

# Un point de l'essai dans son propre try : un échec est noté, l'app revient au repos et le
# témoin en haut (Revenir-Au-Repos), et l'essai continue.
function Point([string]$Nom, [scriptblock]$Corps) {
    try { & $Corps }
    catch {
        Verifier $Nom $false "$_" | Out-Null
        if ($IsWindows) { Revenir-Au-Repos $Nom }
    }
}

# Fin d'un cas : aucune ligne qui ne doit jamais paraître (§8, F).
function Verifier-Interdites([string]$Dossier, [string]$Quoi) {
    $l = @(Lignes-Journal $Dossier | Where-Object { $_ -match $MOTIFS['Interdites'] } | ForEach-Object { Message $_ })
    Verifier "$Quoi aucune ligne « fin deja rendue », « fil arrete (panique) », « annulee focus_perdu »" ($l.Count -eq 0) ($l -join ' | ') | Out-Null
}
# Fin d'une course : aucune ligne WARN ou ERROR de la défilante depuis les compteurs $Cpt.
function Verifier-Avertissements([string]$Dossier, $Cpt, [string]$Quoi) {
    $l = @(Apres $Dossier 'Avert' $Cpt | ForEach-Object { Message $_ })
    Verifier "$Quoi aucune ligne WARN ou ERROR de la défilante" ($l.Count -eq 0) "$($l.Count) ligne(s) : $($l -join ' | ')" | Out-Null
}

# Les lignes `vivant: …` (mesures du chantier A) et les notes des tests de B, recopiées de
# la sortie de `cargo test` (étape Tests, BREEZYSHOT_SORTIE_TESTS), avec les durées des
# suites de tests.
function Recopier-Tests {
    $f = $env:BREEZYSHOT_SORTIE_TESTS
    if (-not $f -or -not (Test-Path -LiteralPath $f)) {
        Write-Host '[note] sortie des tests absente (BREEZYSHOT_SORTIE_TESTS) : lignes « vivant: » non recopiées'
        return
    }
    $pris = @(Get-Content -LiteralPath $f -Encoding utf8 | Where-Object {
            $_ -match '^vivant: ' -or $_ -match "^fenetre d'essai sans ScrollPattern" -or
            $_ -match '^faire defiler les fenetres inactives' -or $_ -match '^test result: '
        })
    Write-Host "--- sortie des tests (cargo test) : $($pris.Count) ligne(s) retenue(s) ---"
    foreach ($l in $pris) { Write-Host "tests | $l" }
    Noter-Mesure -Nom 'lignes « vivant: » recopiées des tests' -Valeur (@($pris | Where-Object { $_ -match '^vivant: ' }).Count)
}

try {
    Recopier-Tests
    if (-not $IsWindows) {
        $d = Dossier-Cas 'commun'
        Ecrire-Reglages -Dossier $d -Vignette $false
        $proc = Demarrer-App -Dossier $d
        $null = Attendre-Motif -Motif 'application +pret$' -Dossier $d -DelaiS 30 -Quoi 'démarrage'
        $null = Attendre-Motif -Motif 'licence absente$' -Dossier $d -DelaiS 5 -Quoi 'licence au démarrage'
        Verifier '(L0) licence absente au démarrage' $true | Out-Null
        Fermer-Cas $proc $d 'commun'
        Verifier '--quitter → arret propre' $true | Out-Null
        Write-Host '[saute] le reste : sauté hors Windows'
    }
    else {
        Add-Type -AssemblyName UIAutomationClient
        Add-Type -AssemblyName UIAutomationTypes
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms
        Add-Type -Namespace BreezyShotEssai -Name Defilante -MemberDefinition @'
[DllImport("user32.dll")]
static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
[DllImport("user32.dll")]
static extern uint MapVirtualKey(uint code, uint type);
[DllImport("user32.dll")]
static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
[DllImport("user32.dll")]
public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SetProcessDpiAwarenessContext(IntPtr contexte);
[DllImport("user32.dll")]
public static extern bool IsWindow(IntPtr h);
[DllImport("user32.dll")]
public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")]
public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")]
public static extern bool SetForegroundWindow(IntPtr h);
[StructLayout(LayoutKind.Sequential)]
public struct POINT { public int X; public int Y; }
[StructLayout(LayoutKind.Sequential)]
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
[DllImport("user32.dll")]
public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
public static extern IntPtr GetWindowLongPtr(IntPtr h, int index);
[DllImport("user32.dll")]
public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
[DllImport("user32.dll")]
public static extern IntPtr SendMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
[DllImport("user32.dll")]
public static extern bool SetWindowPos(IntPtr h, IntPtr apres, int x, int y, int l, int ht, uint drapeaux);
[DllImport("user32.dll", CharSet = CharSet.Unicode)]
static extern int GetWindowText(IntPtr h, System.Text.StringBuilder t, int n);
[DllImport("user32.dll", CharSet = CharSet.Unicode)]
static extern int GetClassName(IntPtr h, System.Text.StringBuilder t, int n);
[DllImport("user32.dll")]
static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
delegate bool Rappel(IntPtr h, IntPtr p);
[DllImport("user32.dll")]
static extern bool EnumWindows(Rappel f, IntPtr p);
[DllImport("user32.dll")]
static extern IntPtr WindowFromPoint(POINT p);
[DllImport("user32.dll")]
static extern IntPtr GetAncestor(IntPtr h, uint quoi);
[DllImport("user32.dll")]
static extern bool GetCursorPos(out POINT p);

// Gardes. Les appuis de souris ne partent que vers l'app (PidApp) ; les touches ne sont que
// des raccourcis globaux que l'app tient (RegisterHotKey : la touche ne va à aucune
// fenêtre) ; la molette de l'essai (celle « de l'utilisateur ») ne part que vers une
// fenêtre d'un processus permis (témoins, Edge, Explorateur de l'essai), jamais vers l'app
// ni vers une console : un cran reçu ailleurs ferait défiler une fenêtre de la machine.
public static int PidApp;
public static int[] PidsPermis = new int[0];
static int PidDe(IntPtr h) { uint q; GetWindowThreadProcessId(h, out q); return (int)q; }
public static int Pid(IntPtr h) { return PidDe(h); }
static bool Permis(int pid) {
    if (pid == 0) return false;
    foreach (int p in PidsPermis) if (p == pid) return true;
    return false;
}
public static string Decrire(IntPtr h) {
    if (h == IntPtr.Zero) return "aucune fenêtre";
    string nom = "?";
    try { nom = System.Diagnostics.Process.GetProcessById(PidDe(h)).ProcessName; } catch { }
    return Classe(h) + " (" + (long)h + "), processus " + nom;
}
static void Garder(IntPtr h, string quoi) {
    if (PidApp == 0 || h == IntPtr.Zero || PidDe(h) != PidApp)
        throw new InvalidOperationException(quoi + " (" + Decrire(h) + ")");
}
public static IntPtr Sous(int x, int y) {
    POINT p; p.X = x; p.Y = y;
    return GetAncestor(WindowFromPoint(p), 2);
}

const uint RELACHEE = 0x2;
const uint ETENDUE = 0x1;
static uint Drapeaux(byte vk) { return (vk >= 0x21 && vk <= 0x28) || vk == 0x2D || vk == 0x2E ? ETENDUE : 0u; }
static void Bas(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), Drapeaux(vk), UIntPtr.Zero); }
static void Haut(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), Drapeaux(vk) | RELACHEE, UIntPtr.Zero); }
// Un raccourci global de l'app (tenu par elle) : il ne vise aucune fenêtre, il part sans garde.
public static void AppuyerGlobal(byte[] mods, byte touche) {
    foreach (var m in mods) Bas(m);
    Bas(touche);
    System.Threading.Thread.Sleep(30);
    Haut(touche);
    for (int i = mods.Length - 1; i >= 0; i--) Haut(mods[i]);
    System.Threading.Thread.Sleep(30);
}
public static void Poser(int x, int y) { SetCursorPos(x, y); mouse_event(0x1, 0, 0, 0, UIntPtr.Zero); }
public static void BoutonBas() {
    POINT p; GetCursorPos(out p);
    Garder(GetAncestor(WindowFromPoint(p), 2), "appui de souris en " + p.X + "," + p.Y + " non envoyé : ce point n'est pas sur une fenêtre de l'app");
    mouse_event(0x2, 0, 0, 0, UIntPtr.Zero);
}
public static void BoutonHaut() { mouse_event(0x4, 0, 0, 0, UIntPtr.Zero); }
// Pose la souris, puis attend que ce point soit sur une fenêtre de l'app (la fenêtre de
// contrôle ne reçoit les clics que lorsque sa surveillance a vu la souris sur la pilule).
public static bool PoserSurApp(int x, int y, int ms) {
    Poser(x, y);
    DateTime fin = DateTime.UtcNow.AddMilliseconds(ms);
    do {
        IntPtr h = Sous(x, y);
        if (PidApp != 0 && h != IntPtr.Zero && PidDe(h) == PidApp) return true;
        System.Threading.Thread.Sleep(20);
    } while (DateTime.UtcNow < fin);
    return false;
}
public static void Cliquer(int x, int y) {
    Poser(x, y); System.Threading.Thread.Sleep(40);
    BoutonBas(); System.Threading.Thread.Sleep(60); BoutonHaut();
}
[DllImport("winmm.dll")]
static extern uint timeBeginPeriod(uint ms);
[DllImport("winmm.dll")]
static extern uint timeEndPeriod(uint ms);
// La fenêtre `h` relevée toutes les 2 ms (pause d'une milliseconde, horloge du système
// réglée à 1 ms le temps du guet : aucun cœur tenu pendant qu'on mesure l'app).
static int Guetter(IntPtr h, int maxMs) {
    var sw = System.Diagnostics.Stopwatch.StartNew();
    if (!IsWindowVisible(h)) return -1;
    timeBeginPeriod(1);
    try {
        long prochain = 2;
        while (sw.ElapsedMilliseconds <= maxMs) {
            while (sw.ElapsedMilliseconds < prochain) System.Threading.Thread.Sleep(1);
            if (!IsWindowVisible(h)) return (int)sw.ElapsedMilliseconds;
            prochain = sw.ElapsedMilliseconds + 2;
        }
        return maxMs + 1;
    }
    finally { timeEndPeriod(1); }
}
// Clic réel (gardé), puis la fenêtre `h` guettée toutes les 2 ms depuis le relâcher : rend
// le délai (ms) au bout duquel elle est cachée ; -1 si elle ne l'était déjà plus au
// relâcher, maxMs + 1 si elle l'est encore à maxMs.
public static int CliquerPuisGuetter(int x, int y, IntPtr h, int maxMs) {
    Poser(x, y); System.Threading.Thread.Sleep(40);
    BoutonBas(); System.Threading.Thread.Sleep(60); BoutonHaut();
    return Guetter(h, maxMs);
}
static void Deplacer(int x0, int y0, int x1, int y1, int pas, int ms) {
    for (int i = 1; i <= pas; i++) {
        Poser(x0 + (x1 - x0) * i / pas, y0 + (y1 - y0) * i / pas);
        System.Threading.Thread.Sleep(ms);
    }
}
// Glisser réel (zone de x0,y0 à x1,y1 bornes comprises), puis la fenêtre `h` guettée.
public static int TracerPuisGuetter(int x0, int y0, int x1, int y1, IntPtr h, int maxMs) {
    Poser(x0, y0); System.Threading.Thread.Sleep(80);
    BoutonBas(); System.Threading.Thread.Sleep(80);
    Deplacer(x0, y0, x1, y1, 20, 15);
    System.Threading.Thread.Sleep(80);
    BoutonHaut();
    return Guetter(h, maxMs);
}
// La molette de « l'utilisateur » : n crans (delta 120 vers le haut, -120 vers le bas) au
// point (x, y), sans la marque de l'app, `ms` entre deux crans (0 : d'un coup). Chaque cran
// ne part que si le point est sur une fenêtre d'un processus permis.
public static void Molette(int x, int y, int n, int delta, int ms) {
    Poser(x, y);
    for (int i = 0; i < n; i++) {
        IntPtr h = Sous(x, y);
        if (!Permis(PidDe(h)))
            throw new InvalidOperationException("cran de molette en " + x + "," + y + " non envoyé : ce point n'est pas sur une fenêtre permise (" + Decrire(h) + ")");
        mouse_event(0x0800, 0, 0, unchecked((uint)delta), UIntPtr.Zero);
        if (ms > 0) System.Threading.Thread.Sleep(ms);
    }
}
public static string Titre(IntPtr h) {
    var t = new System.Text.StringBuilder(512); GetWindowText(h, t, t.Capacity); return t.ToString();
}
public static string Classe(IntPtr h) {
    var t = new System.Text.StringBuilder(256); GetClassName(h, t, t.Capacity); return t.ToString();
}
public static IntPtr[] Fenetres(int pid, string classe) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, p) => {
        uint q; GetWindowThreadProcessId(h, out q);
        if ((int)q == pid && (classe == null || Classe(h) == classe)) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
// Fenêtres de premier niveau visibles, tous processus, dont le titre commence par `titre`
// (Edge peut ajouter « - Microsoft Edge » ou le nom du profil).
public static IntPtr[] VisiblesTitre(string titre) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, p) => {
        if (IsWindowVisible(h) && Titre(h).StartsWith(titre, StringComparison.Ordinal)) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}

// --- Images (BGRA, 4 octets par pixel, lignes de 4 l octets) -----------------------------
public static int Differences(byte[] a, byte[] b, int l, int h) {
    if (a.Length != b.Length || a.Length != l * h * 4) return -1;
    int n = 0;
    for (int i = 0; i < a.Length; i += 4)
        if (a[i] != b[i] || a[i + 1] != b[i + 1] || a[i + 2] != b[i + 2]) n++;
    return n;
}
// L'indice du premier pixel (ordre des rangées) de rouge > rMin, vert < vMax, bleu > bMin ;
// -1 s'il n'y en a pas.
public static int Premier(byte[] o, int rMin, int vMax, int bMin) {
    for (int i = 0; i + 3 < o.Length; i += 4)
        if (o[i + 2] > rMin && o[i + 1] < vMax && o[i] > bMin) return i / 4;
    return -1;
}
// Part des pixels qui diffèrent de plus de `seuil` sur un canal. Avec `sansNoirs`, seuls
// comptent les pixels de `a` dont R + V + B >= 192 (un voile noir ne change pas le noir).
public static double PartDifferente(byte[] a, byte[] b, int seuil, bool sansNoirs) {
    if (a.Length != b.Length || a.Length == 0) return -1;
    long n = 0, d = 0;
    for (int i = 0; i + 3 < a.Length; i += 4) {
        if (sansNoirs && a[i] + a[i + 1] + a[i + 2] < 192) continue;
        n++;
        if (Math.Abs(a[i] - b[i]) > seuil || Math.Abs(a[i + 1] - b[i + 1]) > seuil || Math.Abs(a[i + 2] - b[i + 2]) > seuil) d++;
    }
    return n == 0 ? -1 : (double)d / n;
}

// Décodeur de la page du témoin (formule de temoin-defilant.ps1) : pour chaque rangée de
// l'image, la rangée y de la page qu'elle montre ; -1 en-tête (damier magenta : vert < 40,
// rouge et bleu > 100) ; -2 bits illisibles (gris entre 240 et 525 de R + V + B) ; -3
// couleur de la ligne fausse (± 24 par canal) ; -4 hors de l'image. `ox`, `oy` : coin de
// la page dans l'image.
public static int[] Decoder(byte[] bgra, int l, int h, int ox, int oy, int n) {
    var lus = new int[n];
    for (int j = 0; j < n; j++) {
        int y = oy + j;
        if (y < 0 || y >= h || ox < 0 || ox + 600 > l) { lus[j] = -4; continue; }
        int b0 = (y * l + ox + 4) * 4;
        int B = bgra[b0], V = bgra[b0 + 1], R = bgra[b0 + 2];
        if (V < 40 && R > 100 && B > 100) { lus[j] = -1; continue; }
        int v = 0; bool lisible = true;
        for (int c = 0; c < 16; c++) {
            int p = (y * l + ox + 240 + 8 * c + 4) * 4;
            int s = bgra[p] + bgra[p + 1] + bgra[p + 2];
            if (s > 240 && s < 525) { lisible = false; break; }
            v = (v << 1) | (s < 384 ? 1 : 0);
        }
        if (!lisible) { lus[j] = -2; continue; }
        int k = v / 50;
        int er = 64 + 20 * (k & 7), ev = 64 + 20 * ((k >> 3) & 7), eb = 64 + 20 * ((k >> 6) & 7);
        if (Math.Abs(R - er) > 24 || Math.Abs(V - ev) > 24 || Math.Abs(B - eb) > 24) { lus[j] = -3; continue; }
        lus[j] = v;
    }
    return lus;
}
// Écart entre les rangées lues et l'attendu : `entete` rangées d'en-tête (-1), puis les
// rangées 0, 1, 2… de la page. Rend "" si tout est juste ; sinon le nombre de rangées
// fausses, les premières, et la nature du premier saut (trou, doublon, retour).
public static string Ecarts(int[] lus, int entete) {
    int faux = 0;
    var premiers = new System.Text.StringBuilder();
    string saut = "";
    for (int j = 0; j < lus.Length; j++) {
        int attendu = j < entete ? -1 : j - entete;
        if (lus[j] == attendu) continue;
        faux++;
        if (faux <= 6) premiers.Append(" " + j + ":" + lus[j]);
    }
    for (int j = entete + 1; j < lus.Length && saut == ""; j++) {
        int a = lus[j - 1], b = lus[j];
        if (a < 0 || b < 0 || b == a + 1) continue;
        if (b > a + 1) saut = "trou de " + (b - a - 1) + " rangée(s) après la rangée " + a + " (image " + (j - 1) + ")";
        else if (b == a) saut = "doublon de la rangée " + a + " (image " + j + ")";
        else saut = "retour de la rangée " + a + " à " + b + " (image " + j + ")";
    }
    if (faux == 0) return "";
    return faux + " rangée(s) fausse(s) sur " + lus.Length + ", premières (image:lu)" + premiers + (saut != "" ? " ; " + saut : "");
}
// L'image (l × h) contre une référence (l × hr) posée à la rangée `oy` de l'image : rend
// { pixels différents de plus de `seuil` sur un canal, x et y (dans l'image) du premier,
// pixels comparés } ; { -1, … } si les largeurs diffèrent ou si la référence déborde.
public static int[] EcartReference(byte[] img, int l, int h, int oy, byte[] reference, int lr, int hr, int seuil) {
    if (l != lr || oy < 0 || oy + hr > h || img.Length < l * h * 4 || reference.Length < lr * hr * 4) return new int[] { -1, -1, -1, 0 };
    int n = 0, px = -1, py = -1;
    for (int y = 0; y < hr; y++) {
        int a = (oy + y) * l * 4, b = y * lr * 4;
        for (int x = 0; x < l; x++, a += 4, b += 4) {
            if (Math.Abs(img[a] - reference[b]) > seuil || Math.Abs(img[a + 1] - reference[b + 1]) > seuil || Math.Abs(img[a + 2] - reference[b + 2]) > seuil) {
                if (n == 0) { px = x; py = oy + y; }
                n++;
            }
        }
    }
    return new int[] { n, px, py, l * hr };
}
// Combien de rangées de valeur -1 (en-tête) au total.
public static int Compter(int[] lus, int valeur) {
    int n = 0;
    foreach (int v in lus) if (v == valeur) n++;
    return n;
}

// Ce que le chantier C tire de chaque rangée (defilement/mod.rs, `decrire` et `fond`,
// pour une image de 768 colonnes au plus : aucune colonne sautée ; aucune bande de droite
// retirée). Par rangée, RESUME valeurs : les 16 blocs du résumé (canaux d'encre du bloc
// rapportés à 255), le gris du fond, les canaux d'encre, porteuse (0 ou 1). L'encre : un
// canal à plus de 3 × `canal` du fond de la rangée (la couleur la plus fréquente de 64
// pixels pris à égale distance, gris par tranches de 8, deux tranches voisines réunies) ;
// porteuse : au moins 6 canaux d'encre, dont au moins 4 qui changent de plus de `canal` à
// 3 rangées au-dessus ou au-dessous. Les paquets de 64 octets de C (le dernier complété
// par le fond) découpent les blocs.
public const int RESUME = 19;
static void Fond(byte[] b, int o, int l, out int c0, out int c1, out int c2) {
    int m = Math.Min(l, 64);
    var pris = new int[64 * 4];
    var tranches = new int[33];
    int q = l / Math.Max(1, m), rr = l % Math.Max(1, m), x = 0, reste = 0;
    for (int p = 0; p < m; p++) {
        int i = o + x * 4;
        int g = ((b[i] + 2 * b[i + 1] + b[i + 2]) / 4) >> 3;
        pris[p * 4] = b[i]; pris[p * 4 + 1] = b[i + 1]; pris[p * 4 + 2] = b[i + 2]; pris[p * 4 + 3] = g;
        tranches[g]++;
        x += q; reste += rr;
        if (reste >= m) { reste -= m; x++; }
    }
    int meilleure = 0;
    for (int j = 1; j < 32; j++) if (tranches[j] + tranches[j + 1] > tranches[meilleure] + tranches[meilleure + 1]) meilleure = j;
    long s0 = 0, s1 = 0, s2 = 0; int c = 0;
    for (int p = 0; p < m; p++) {
        int g = pris[p * 4 + 3];
        if (g == meilleure || g == meilleure + 1) { s0 += pris[p * 4]; s1 += pris[p * 4 + 1]; s2 += pris[p * 4 + 2]; c++; }
    }
    c = Math.Max(1, c);
    c0 = (int)(s0 / c); c1 = (int)(s1 / c); c2 = (int)(s2 / c);
}
public static int[] DecrireC(byte[] bgra, int l, int h, int canal) {
    const int PAQUET = 64, BLOCS = 16, ECART = 3, ENCRE_MIN = 6, CHANGE_MIN = 4;
    int seuil = Math.Min(255, 3 * canal), lon = l * 4;
    int n = (lon + PAQUET - 1) / PAQUET, blocs = Math.Min(BLOCS, n);
    var bornes = new int[BLOCS + 1];
    for (int k = 0; k <= BLOCS; k++) bornes[k] = Math.Min(k, blocs) * n / Math.Max(1, blocs);
    var r = new int[h * RESUME];
    for (int y = 0; y < h; y++) {
        int o = y * lon, od = y >= ECART ? (y - ECART) * lon : o, ob = y + ECART < h ? (y + ECART) * lon : o;
        int f0, f1, f2;
        Fond(bgra, o, l, out f0, out f1, out f2);
        int encre = 0, haut = 0, bas = 0, s = y * RESUME;
        for (int k = 0; k < blocs; k++) {
            int e = 0, fin = Math.Min(bornes[k + 1] * PAQUET, lon);
            for (int i = bornes[k] * PAQUET; i < fin; i++) {
                int c = i & 3;
                if (c == 3) continue;
                int v = bgra[o + i], fc = c == 0 ? f0 : (c == 1 ? f1 : f2);
                if (Math.Abs(v - fc) <= seuil) continue;
                e++;
                if (Math.Abs(v - bgra[od + i]) > canal) haut++;
                if (Math.Abs(v - bgra[ob + i]) > canal) bas++;
            }
            long canaux = Math.Max(1, (bornes[k + 1] - bornes[k]) * PAQUET * 3 / 4);
            r[s + k] = (int)Math.Min(255L, ((long)e * ((255L << 16) / canaux)) >> 16);
            encre += e;
        }
        r[s + BLOCS] = (f0 + 2 * f1 + f2) / 4;
        r[s + BLOCS + 1] = encre;
        r[s + BLOCS + 2] = (encre >= ENCRE_MIN && Math.Max(haut, bas) >= CHANGE_MIN) ? 1 : 0;
    }
    return r;
}
// Les descriptions en texte, une par rangée : les 16 blocs (2 chiffres hexadécimaux
// chacun), puis « g<gris du fond> e<canaux d'encre> p<porteuse> ».
public static string[] TexteResumes(int[] r, int h) {
    var t = new string[h];
    var sb = new System.Text.StringBuilder(48);
    for (int y = 0; y < h; y++) {
        sb.Clear();
        int s = y * RESUME;
        for (int k = 0; k < 16; k++) sb.Append(r[s + k].ToString("x2"));
        sb.Append(" g").Append(r[s + 16]).Append(" e").Append(r[s + 17]).Append(" p").Append(r[s + 18]);
        t[y] = sb.ToString();
    }
    return t;
}
public static int Porteuses(int[] r, int h) {
    int n = 0;
    for (int y = 0; y < h; y++) n += r[y * RESUME + 18];
    return n;
}
// Score D12 d'un décalage d (la rangée y de b montre la rangée y + d de a ; d > 0 : la page
// avance) : part des rangées du recouvrement, 4 rangées de bord exclues de chaque côté, dont
// 90 % des pixels sont égaux à ± 8 par canal. -1 si le recouvrement est trop court.
public static double ScoreD12(byte[] a, byte[] b, int l, int h, int d) {
    int debut = Math.Max(0, -d), fin = Math.Min(h, h - d);
    debut += 4; fin -= 4;
    if (fin - debut < 8) return -1;
    int egales = 0;
    for (int y = debut; y < fin; y++) {
        int ok = 0;
        int pa = ((y + d) * l) * 4, pb = (y * l) * 4;
        for (int x = 0; x < l; x++, pa += 4, pb += 4)
            if (Math.Abs(a[pa] - b[pb]) <= 8 && Math.Abs(a[pa + 1] - b[pb + 1]) <= 8 && Math.Abs(a[pa + 2] - b[pb + 2]) <= 8) ok++;
        if (ok * 10 >= l * 9) egales++;
    }
    return (double)egales / (fin - debut);
}
// Le meilleur décalage au sens de D12 sur |d| <= dmax (sans vérité connue) : rend d, ou
// int.MinValue si aucun n'atteint 97 %.
public static int MeilleurD12(byte[] a, byte[] b, int l, int h, int dmax) {
    int meilleur = int.MinValue; double score = 0.97 - 1e-9;
    for (int d = -dmax; d <= dmax; d++) {
        double s = ScoreD12(a, b, l, h, d);
        if (s > score) { score = s; meilleur = d; }
    }
    return meilleur;
}
// Les décalages classés sur les seuls résumés (DecrireC) : pour chaque d (recouvrement d'au
// moins `minRecouvrement` rangées, 4 rangées de bord exclues), la part des rangées dont les
// 16 blocs sont égaux à ± 8 ; du meilleur au moins bon, à égalité le plus petit |d|.
public static int[] ClasserResumes(int[] ra, int[] rb, int h, int minRecouvrement) {
    var ds = new System.Collections.Generic.List<int>();
    var scores = new System.Collections.Generic.Dictionary<int, double>();
    for (int d = -(h - minRecouvrement); d <= h - minRecouvrement; d++) {
        int debut = Math.Max(0, -d) + 4, fin = Math.Min(h, h - d) - 4;
        if (fin - debut < 8) continue;
        int egales = 0;
        for (int y = debut; y < fin; y++) {
            bool eg = true;
            for (int k = 0; k < 16 && eg; k++) if (Math.Abs(ra[(y + d) * RESUME + k] - rb[y * RESUME + k]) > 8) eg = false;
            if (eg) egales++;
        }
        ds.Add(d); scores[d] = (double)egales / (fin - debut);
    }
    ds.Sort((x, y) => { int c = scores[y].CompareTo(scores[x]); return c != 0 ? c : Math.Abs(x).CompareTo(Math.Abs(y)); });
    return ds.ToArray();
}
'@
        try { [void][BreezyShotEssai.Defilante]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }
        $NATIF = [BreezyShotEssai.Defilante]
        $AE = [System.Windows.Automation.AutomationElement]
        $CT = [System.Windows.Automation.ControlType]

        $MODS = [byte[]]@(0x11, 0x10)          # Ctrl, Maj
        $AUCUN = [byte[]]@()
        $VK_2 = [byte]0x32; $VK_5 = [byte]0x35
        $ECHAP = [byte]0x1B; $ENTREE = [byte]0x0D
        $MOD_CTRL_MAJ = 6                       # RegisterHotKey : MOD_CONTROL | MOD_SHIFT
        $WM_CLOSE = [uint32]0x10
        $WS_EX = @{ TOPMOST = 0x8; TRANSPARENT = 0x20; TOOLWINDOW = 0x80; APPWINDOW = 0x40000; NOACTIVATE = 0x08000000 }
        $utile = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        $ecran = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        # Souris au repos : sur le bureau, à droite, sous la sentinelle (760..1000 × 80..380),
        # loin du témoin (40..640), de la pilule et de l'aperçu.
        $REPOS_X = [int]($utile.Right - 12)
        $REPOS_Y = [int]($utile.Top + $utile.Height / 2 + 40)
        $CRAN = [math]::Max(1, [System.Windows.Forms.SystemInformation]::MouseWheelScrollLines) * 16
        $routage = try { (Get-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name MouseWheelRouting -ErrorAction Stop).MouseWheelRouting } catch { '<absent>' }
        Write-Host "écran $ecran ; zone utile $utile ; molette : $([System.Windows.Forms.SystemInformation]::MouseWheelScrollLines) lignes par cran ($CRAN px dans le témoin) ; MouseWheelRouting $routage"

        # « … <x>,<y> <l>x<h> … » → { X, Y, L, H, CX, CY }.
        function Lire-Rect([string]$Texte) {
            if ($Texte -notmatch '(-?\d+),(-?\d+) (\d+)x(\d+)') { throw "rectangle illisible : $Texte" }
            $r = [pscustomobject]@{ X = [int]$Matches[1]; Y = [int]$Matches[2]; L = [int]$Matches[3]; H = [int]$Matches[4] }
            $r | Add-Member CX ([int]($r.X + [math]::Floor($r.L / 2)))
            $r | Add-Member CY ([int]($r.Y + [math]::Floor($r.H / 2)))
            return $r
        }
        function Texte-Rect($Rect) { return "$($Rect.X),$($Rect.Y) $($Rect.L)x$($Rect.H)" }
        function Hwnd-Journal([string]$Motif, [string]$Dossier) {
            $l = Attendre-Motif -Motif "$Motif (\d+)$" -Dossier $Dossier -DelaiS 5 -Quoi $Motif
            $null = $l -match "$Motif (\d+)$"
            return [IntPtr][long]$Matches[1]
        }
        function Permettre([int[]]$Pids) { [BreezyShotEssai.Defilante]::PidsPermis = $Pids }

        # --- Images ----------------------------------------------------------------------------
        # Une image en octets BGRA : { L, H, O }.
        function Octets-Image($Bitmap) {
            $r = [System.Drawing.Rectangle]::new(0, 0, $Bitmap.Width, $Bitmap.Height)
            $d = $Bitmap.LockBits($r, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
            try {
                $o = [byte[]]::new($d.Stride * $Bitmap.Height)
                [System.Runtime.InteropServices.Marshal]::Copy($d.Scan0, $o, 0, $o.Length)
            }
            finally { $Bitmap.UnlockBits($d) }
            return [pscustomobject]@{ L = $Bitmap.Width; H = $Bitmap.Height; O = $o }
        }
        function Octets-Png([byte[]]$Octets) {
            $flux = [System.IO.MemoryStream]::new($Octets)
            try {
                $img = [System.Drawing.Bitmap]::new($flux)
                try { return Octets-Image $img } finally { $img.Dispose() }
            }
            finally { $flux.Dispose() }
        }
        function Octets-Fichier([string]$Chemin) {
            $img = [System.Drawing.Bitmap]::new($Chemin)
            try { return Octets-Image $img } finally { $img.Dispose() }
        }
        function Copie-Ecran([int]$Gx, [int]$Gy, [int]$Larg, [int]$Haute) {
            $b = [System.Drawing.Bitmap]::new($Larg, $Haute, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
            try {
                $g = [System.Drawing.Graphics]::FromImage($b)
                try { $g.CopyFromScreen($Gx, $Gy, 0, 0, [System.Drawing.Size]::new($Larg, $Haute)) } finally { $g.Dispose() }
                return Octets-Image $b
            }
            finally { $b.Dispose() }
        }
        # Copie de l'écran posée : deux copies de suite égales, à 80 ms d'écart.
        function Copie-Stable([int]$Gx, [int]$Gy, [int]$Larg, [int]$Haute, [string]$Quoi, [double]$DelaiS = 3) {
            $a = Copie-Ecran $Gx $Gy $Larg $Haute
            $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
            do {
                Start-Sleep -Milliseconds 80
                $b = Copie-Ecran $Gx $Gy $Larg $Haute
                if ($NATIF::Differences($a.O, $b.O, $Larg, $Haute) -eq 0) { return $b }
                $a = $b
            } while ([DateTime]::UtcNow -lt $limite)
            throw "$Quoi : l'image de l'écran ne se pose pas en $DelaiS s"
        }
        # Le presse-papiers, dans un fil STA : l'image (octets BGRA), sinon une erreur.
        function Lire-PressePapiers {
            $rs = [runspacefactory]::CreateRunspace()
            $rs.ApartmentState = [System.Threading.ApartmentState]::STA
            $rs.Open()
            try {
                $ps = [powershell]::Create()
                $ps.Runspace = $rs
                [void]$ps.AddScript({
                        Add-Type -AssemblyName System.Windows.Forms, System.Drawing
                        for ($i = 0; $i -lt 10; $i++) {
                            try {
                                $img = [System.Windows.Forms.Clipboard]::GetImage()
                                if ($null -ne $img) {
                                    $m = [System.IO.MemoryStream]::new()
                                    $img.Save($m, [System.Drawing.Imaging.ImageFormat]::Png)
                                    $img.Dispose()
                                    return , $m.ToArray()
                                }
                            } catch { }
                            Start-Sleep -Milliseconds 100
                        }
                        return $null
                    })
                try { $r = @($ps.Invoke()) } finally { $ps.Dispose() }
                foreach ($x in $r) { if ($null -ne $x) { return (Octets-Png ([byte[]]$x.psobject.BaseObject)) } }
                throw 'presse-papiers : aucune image'
            }
            finally { $rs.Dispose() }
        }
        # Le nouveau PNG de « Récents » (absent de $Deja), lisible : { Chemin, Img }.
        function Nouveau-Png([string]$Dossier, [string[]]$Deja, [string]$Quoi) {
            $limite = [DateTime]::UtcNow.AddSeconds(15)
            $dernier = ''
            do {
                $n = @(Recents $Dossier | Where-Object { $Deja -notcontains $_ })
                if ($n.Count -ge 1) {
                    try { return [pscustomobject]@{ Chemin = $n[-1]; Nombre = $n.Count; Img = (Octets-Fichier $n[-1]) } }
                    catch { $dernier = $_.Exception.Message }
                }
                Start-Sleep -Milliseconds 100
            } while ([DateTime]::UtcNow -lt $limite)
            throw "$Quoi : aucun PNG nouveau et lisible dans « Récents » en 15 s $(if ($dernier) { "(dernière erreur : $dernier)" })"
        }
        # L'image relue rangée par rangée contre la page du témoin : $Entete rangées
        # d'en-tête, puis les rangées 0, 1, 2… de la page, exactement.
        function Verifier-Page([string]$Nom, $Image, [int]$Entete, [int]$Hauteur = -1) {
            $lus = $NATIF::Decoder($Image.O, $Image.L, $Image.H, 0, 0, $Image.H)
            $ecart = $NATIF::Ecarts($lus, $Entete)
            $taille = ($Image.L -eq 600) -and (($Hauteur -lt 0) -or ($Image.H -eq $Hauteur))
            $attendu = if ($Hauteur -ge 0) { "600x$Hauteur" } else { '600 de large' }
            Verifier $Nom ($taille -and ($ecart -eq '')) "image $($Image.L)x$($Image.H) (attendu $attendu)$(if ($ecart) { " ; $ecart" })" | Out-Null
            return $lus
        }

        # --- Témoins -----------------------------------------------------------------------
        # L'état écrit par le témoin. Sans fichier lisible : une erreur qui le dit, ou $null
        # avec -SiPresent (le témoin qui démarre).
        function Etat-Temoin([string]$FichierEtat, [switch]$SiPresent) {
            $lignes = $null
            if (Test-Path -LiteralPath $FichierEtat) {
                for ($i = 0; $i -lt 10 -and $null -eq $lignes; $i++) {
                    try { $lignes = [System.IO.File]::ReadAllLines($FichierEtat) } catch { Start-Sleep -Milliseconds 20 }
                }
            }
            if ($null -eq $lignes) {
                if ($SiPresent) { return $null }
                throw "témoin sans état : « $([System.IO.Path]::GetFileName($FichierEtat)) » absent ou illisible"
            }
            $e = [ordered]@{ Hwnd = [IntPtr]::Zero; Pid = 0; Fenetre = $null; Page = $null; EnTete = $null; Contenu = 0; Visible = 0; Echelle = 0.0; Fait = 0; Crans = 0
                Positions = [System.Collections.Generic.List[object]]::new() }
            foreach ($l in $lignes) {
                if ($l -match '^hwnd (\d+)$') { $e.Hwnd = [IntPtr][long]$Matches[1] }
                elseif ($l -match '^pid (\d+)$') { $e.Pid = [int]$Matches[1] }
                elseif ($l -match '^fenetre (.+)$') { $e.Fenetre = Lire-Rect $Matches[1] }
                elseif ($l -match '^page (.+)$') { $e.Page = Lire-Rect $Matches[1] }
                elseif ($l -match '^entete (.+)$') { $e.EnTete = Lire-Rect $Matches[1] }
                elseif ($l -match '^contenu (\d+)$') { $e.Contenu = [int]$Matches[1] }
                elseif ($l -match '^visible (\d+)$') { $e.Visible = [int]$Matches[1] }
                elseif ($l -match '^echelle (\S+)$') { $e.Echelle = [double]::Parse($Matches[1], [Globalization.CultureInfo]::InvariantCulture) }
                elseif ($l -match '^fait (\d+)$') { $e.Fait = [int]$Matches[1] }
                elseif ($l -match '^crans (\d+)$') { $e.Crans = [int]$Matches[1] }
                elseif ($l -match '^position (-?\d+) (\d+)$') { $e.Positions.Add([pscustomobject]@{ P = [int]$Matches[1]; Ms = [long]$Matches[2] }) }
            }
            return [pscustomobject]$e
        }
        # Lance temoin-defilant.ps1 (sans console à lui) et attend son état : { Nom, Aide,
        # Fichier, Hwnd, Pid, Fenetre, Page, EnTete, Bas (position du bas de la page) }.
        function Lancer-Temoin([string]$Nom, [int]$Gx, [int]$Gy, [string[]]$Options = @()) {
            $f = Join-Path $script:Essai.Dossier "temoin-$Nom.txt"
            Remove-Item -LiteralPath $f, "$f.cmd" -Force -ErrorAction SilentlyContinue
            $arguments = @('-NoProfile', '-File', "`"$(Join-Path $PSScriptRoot 'temoin-defilant.ps1')`"", '-X', "$Gx", '-Y', "$Gy", '-Fichier', "`"$f`"") + $Options
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -NoNewWindow -ArgumentList $arguments
            $script:Auxiliaires.Add($aide)
            $limite = [DateTime]::UtcNow.AddSeconds(40)
            $e = $null
            do {
                $e = Etat-Temoin $f -SiPresent
                if ($null -ne $e -and $null -ne $e.Page -and $e.Pid -ne 0) { break }
                if ($aide.HasExited) { throw "témoin $Nom : sorti avant d'écrire son état" }
                Start-Sleep -Milliseconds 100
            } while ([DateTime]::UtcNow -lt $limite)
            if ($null -eq $e -or $null -eq $e.Page) { throw "témoin $Nom : aucun état écrit en 40 s" }
            if ([math]::Abs($e.Echelle - 1.0) -gt 0.001) { throw "témoin $Nom : échelle $($e.Echelle) ; l'essai suppose un écran à 96 ppp" }
            $t = [pscustomobject]@{ Nom = $Nom; Aide = $aide; Fichier = $f; Hwnd = $e.Hwnd; Pid = $e.Pid; Fenetre = $e.Fenetre; Page = $e.Page
                EnTete = $e.EnTete; Bas = ($e.Contenu - $e.Visible) }
            Write-Host "témoin $Nom : fenêtre $(Texte-Rect $t.Fenetre) ; page $(Texte-Rect $t.Page) ; contenu $($e.Contenu) px$(if ($t.EnTete) { " ; en-tête $(Texte-Rect $t.EnTete)" })"
            return $t
        }
        function Arreter-Temoin($Tem) {
            if ($null -eq $Tem) { return }
            try { if (-not $Tem.Aide.HasExited) { $Tem.Aide.Kill(); $null = $Tem.Aide.WaitForExit(5000) } } catch { }
        }
        function Positions($Tem) {
            return @((Etat-Temoin $Tem.Fichier).Positions)
        }
        function Position($Tem) {
            $p = @(Positions $Tem)
            if ($p.Count -eq 0) { return 0 }
            return $p[-1].P
        }
        # La position du témoin quand elle n'a plus bougé depuis $CalmeMs.
        function Attendre-Calme($Tem, [string]$Quoi, [int]$CalmeMs = 300, [double]$DelaiS = 5) {
            $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
            $n = @(Positions $Tem).Count
            $depuis = [DateTime]::UtcNow
            do {
                Start-Sleep -Milliseconds 30
                $k = @(Positions $Tem).Count
                if ($k -ne $n) { $n = $k; $depuis = [DateTime]::UtcNow }
                elseif (([DateTime]::UtcNow - $depuis).TotalMilliseconds -ge $CalmeMs) { return (Position $Tem) }
            } while ([DateTime]::UtcNow -lt $limite)
            throw "$Quoi : la page du témoin bouge encore après $DelaiS s (position $(Position $Tem))"
        }
        # La position change (une ligne de plus) dans le délai ; rend la nouvelle, sinon $null.
        function Attendre-Mouvement($Tem, [int]$Deja, [int]$DelaiMs) {
            $limite = [DateTime]::UtcNow.AddMilliseconds($DelaiMs)
            do {
                $p = @(Positions $Tem)
                if ($p.Count -gt $Deja) { return $p[-1].P }
                Start-Sleep -Milliseconds 15
            } while ([DateTime]::UtcNow -lt $limite)
            return $null
        }
        # Le témoin remonte en haut de sa page (commande « haut <n> »), puis se pose.
        $script:Commande = 0
        function Remonter($Tem, [string]$Quoi) {
            $script:Commande++
            $n = $script:Commande
            [System.IO.File]::WriteAllText("$($Tem.Fichier).cmd", "haut $n")
            $limite = [DateTime]::UtcNow.AddSeconds(5)
            while ((Etat-Temoin $Tem.Fichier).Fait -ne $n) {
                if ([DateTime]::UtcNow -ge $limite) { throw "$Quoi : le témoin $($Tem.Nom) n'a pas fait « haut $n » en 5 s" }
                Start-Sleep -Milliseconds 30
            }
            $p = Attendre-Calme $Tem $Quoi 200 5
            if ($p -ne 0) { throw "$Quoi : le témoin $($Tem.Nom) n'est pas en haut (position $p)" }
        }

        # Un raccourci tenu un instant par l'essai (tenir-raccourci.ps1) : « tenu » si l'app
        # ne le tient pas, « refuse » sinon.
        function Tenir-Ailleurs([int]$Vk, [string]$Nom) {
            $fichier = Join-Path $script:Essai.Dossier "tenu-$Nom-$([guid]::NewGuid().ToString('N').Substring(0, 6)).txt"
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -WindowStyle Hidden -ArgumentList @('-NoProfile', '-File', "`"$(Join-Path $PSScriptRoot 'tenir-raccourci.ps1')`"", '-Vk', $Vk, '-Mods', $MOD_CTRL_MAJ, '-Fichier', "`"$fichier`"")
            $script:Auxiliaires.Add($aide)
            $limite = [DateTime]::UtcNow.AddSeconds(30)
            while (-not (Test-Path -LiteralPath $fichier) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
            $lu = if (Test-Path -LiteralPath $fichier) { (Get-Content -LiteralPath $fichier -Raw).Trim() } else { 'pas de réponse en 30 s' }
            try { if (-not $aide.HasExited) { $aide.Kill(); $null = $aide.WaitForExit(5000) } } catch { }
            return $lu
        }

        # --- UI Automation (essais 08, 12, 13) -----------------------------------------------
        function Elements-Vus([IntPtr]$Racine) {
            try {
                $tous = $AE::FromHandle($Racine).FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
                $noms = @()
                foreach ($el in $tous) {
                    $c = $el.Current
                    if ($c.Name -ne '') { $noms += "$($c.ControlType.ProgrammaticName -replace '^ControlType\.', '') « $($c.Name) »" }
                    if ($noms.Count -ge 40) { break }
                }
                return "$($tous.Count) éléments vus ; nommés : " + ($noms -join ', ')
            } catch { return "UI Automation en échec : $($_.Exception.Message)" }
        }
        function Element($Racine, [string]$Nom, $Type = $null, [double]$DelaiS = 5) {
            $cond = [System.Windows.Automation.PropertyCondition]::new($AE::NameProperty, $Nom)
            if ($null -ne $Type) {
                $cond = [System.Windows.Automation.AndCondition]::new($cond, [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, $Type))
            }
            $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
            do {
                try {
                    $el = $AE::FromHandle([IntPtr]$Racine).FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
                    if ($null -ne $el) { return $el }
                } catch { }
                Start-Sleep -Milliseconds 100
            } while ([DateTime]::UtcNow -lt $limite)
            return $null
        }
        # Vrai si l'élément est montré (présent, à l'écran, avec un rectangle).
        function Montre($el) {
            if ($null -eq $el) { return $false }
            try {
                $c = $el.Current
                return (-not $c.IsOffscreen) -and (-not $c.BoundingRectangle.IsEmpty) -and ($c.BoundingRectangle.Width -gt 0)
            } catch { return $false }
        }
        function Centre-Stable($el) {
            $r = $el.Current.BoundingRectangle
            $limite = [DateTime]::UtcNow.AddSeconds(2)
            do {
                Start-Sleep -Milliseconds 120
                $avant = $r; $r = $el.Current.BoundingRectangle
            } while (($r.X -ne $avant.X -or $r.Y -ne $avant.Y) -and [DateTime]::UtcNow -lt $limite)
            if ($r.IsEmpty -or $r.Width -le 0 -or $el.Current.IsOffscreen) { throw "« $($el.Current.Name) » hors écran ou sans rectangle" }
            return @([int]($r.X + $r.Width / 2), [int]($r.Y + $r.Height / 2))
        }

        # --- Menu de l'icône : fenêtre web, aides de commun.ps1 (comme l'essai 13) ------------
        # Ouvert dans le dossier du cas en cours ($script:Cas) ; rend { Fenetre, Textes, Lus,
        # Lignes, Carte, IdProc, Rect }, `Textes` dans l'ordre, '' pour un séparateur.
        function Ouvrir-Menu([string]$Quoi, [string]$Dossier = $script:Cas.Dossier) {
            return (Ouvrir-Menu-Icone $script:proc.Id $Dossier $Quoi)
        }
        function Fermer-Menu($Menu, [string]$Quoi) { Fermer-Menu-Icone $Menu $Quoi }
        function Cliquer-Entree($Menu, [string]$Texte, [string]$Quoi) {
            Cliquer-Ligne-Menu $Menu $Texte $Quoi
            $NATIF::Poser($REPOS_X, $REPOS_Y)
        }
        # Pendant une défilante, sans Échap (l'app le tient en raccourci global : il annulerait
        # la course) : un second clic sur l'icône referme le menu (`menu ferme`).
        function Fermer-Menu-Par-Icone($Menu, [string]$Dossier, [string]$Quoi) {
            $fermes = Compter-Motif 'menu ferme$' $Dossier
            Start-Sleep -Milliseconds 400   # anti-rebond de l'icône (300 ms)
            Clic-Icone $Menu.IdProc $Quoi
            $null = Attendre-Suivante 'menu ferme$' $Dossier $fermes "$Quoi : menu ferme (second clic sur l'icône)" 3
            $limite = [DateTime]::UtcNow.AddSeconds(2)
            while ($NATIF::IsWindowVisible($Menu.Fenetre) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 30 }
            if ($NATIF::IsWindowVisible($Menu.Fenetre)) { throw "$Quoi : le menu est encore visible 2 s après « menu ferme »" }
        }

        # Fenêtres de premier niveau visibles de l'app (classe et titre), pour un diagnostic.
        function Visibles-App {
            return @($NATIF::Fenetres($script:proc.Id, $null) | Where-Object { $NATIF::IsWindowVisible($_) } | ForEach-Object { "$($NATIF::Classe($_)) « $($NATIF::Titre($_)) »" })
        }

        # --- La défilante, pas à pas ----------------------------------------------------------
        # Lance une défilante (Ctrl+Maj+5 ou menu de l'icône) et attend la visée prête :
        # sélection montrée en mode défilante, page de sélection prête, pages proposées (la
        # page de sélection reçoit la liste ensuite : 400 ms), fenêtre de contrôle créée et sa
        # page prête. Le premier plan est relevé toutes les 20 ms (note du chef de chantier :
        # la création de la fenêtre de contrôle ne doit pas retirer le focus à la sélection).
        # Rend { Id, Selection, Controle, Pages, Focus, Base }.
        function Viser([string]$Dossier, [string]$Par, [string]$Quoi, $Base) {
            $NATIF::Poser($REPOS_X, $REPOS_Y)
            if ($Par -eq 'menu') {
                $menus = Compter-Motif 'menu: capturer defilante$' $Dossier
                $m = Ouvrir-Menu $Quoi $Dossier
                Cliquer-Entree $m 'Capture défilante' $Quoi
                $null = Attendre-Suivante 'menu: capturer defilante$' $Dossier $menus "$Quoi : menu: capturer defilante" 5
            }
            else {
                $presses = Compter-Motif 'raccourci presse: defilante$' $Dossier
                $NATIF::AppuyerGlobal($MODS, $VK_5)
                $null = Attendre-Suivante 'raccourci presse: defilante$' $Dossier $presses "$Quoi : raccourci presse: defilante" 5
            }
            $null = Attendre-Cle $Dossier 'Gel' $Base "$Quoi : DemandeCapture defilante" 5
            $ml = Attendre-Cle $Dossier 'Montree' $Base "$Quoi : sélection montrée" 5
            if ($ml -notmatch 'selection montree (\d+) .* mode defilante ') { throw "$Quoi : sélection hors du mode défilante : $(Message $ml)" }
            $id = [int]$Matches[1]
            $hs = Hwnd-Journal 'selection fenetre' $Dossier
            $releves = [System.Collections.Generic.List[object]]::new()
            $sw = [System.Diagnostics.Stopwatch]::StartNew()
            $msFenetre = -1; $msPrete = -1
            $hc = [IntPtr]::Zero; $pages = -1; $finListe = -1
            while ($true) {
                $fg = $NATIF::GetForegroundWindow()
                if ($releves.Count -eq 0 -or $releves[$releves.Count - 1].H -ne $fg) { $releves.Add([pscustomobject]@{ Ms = $sw.ElapsedMilliseconds; H = $fg }) }
                $lignes = Lignes-Journal $Dossier
                $nb = @{}
                foreach ($k in 'Prete', 'DPages', 'Fenetre', 'PagePrete') {
                    $nb[$k] = @($lignes | Where-Object { $_ -match $MOTIFS[$k] }).Count - $Base[$k]
                }
                $fins = @($lignes | Where-Object { $_ -match "selection fin $id " })
                if ($fins.Count -gt 0) { throw "$Quoi : la sélection $id a fini pendant la visée : $(Message $fins[-1]) ; $(Diagnostic-Journal $Dossier)" }
                # La liste va à la sélection (`pages proposees <n>`), ou part avec sa première
                # charge si elle arrive avant (`pages retenues pour la premiere charge`, n lu
                # alors sur `defilante pages <n>`).
                $prop = @($lignes | Where-Object { $_ -match "selection $id`: pages proposees (\d+)$" })
                if ($prop.Count -gt 0 -and $pages -lt 0) { $pages = Nombre $prop[-1] 'pages proposees (\d+)$' }
                $retenues = @($lignes | Where-Object { $_ -match "selection $id`: pages retenues pour la premiere charge$" })
                if ($retenues.Count -gt 0 -and $pages -lt 0 -and $nb['DPages'] -ge 1) {
                    $pages = Nombre (@($lignes | Where-Object { $_ -match $MOTIFS['DPages'] })[-1]) 'defilante pages (\d+) en '
                    Write-Host "[note] $Quoi : pages retenues pour la première charge de la sélection $id ($pages d'après « defilante pages »)"
                }
                if ($nb['Fenetre'] -ge 1 -and $msFenetre -lt 0) {
                    $msFenetre = $sw.ElapsedMilliseconds
                    $l = @($lignes | Where-Object { $_ -match $MOTIFS['Fenetre'] })[-1]
                    $hc = [IntPtr][long](Nombre $l 'defilante fenetre (\d+)( retrouvee)?$')
                    if ($l -match ' retrouvee$') { Write-Host "[note] $Quoi : fenêtre de contrôle retrouvée (R1), non créée" }
                }
                if ($nb['PagePrete'] -ge 1 -and $msPrete -lt 0) { $msPrete = $sw.ElapsedMilliseconds }
                if ($nb['Prete'] -ge 1 -and $nb['DPages'] -ge 1 -and $pages -ge 0 -and $msFenetre -ge 0 -and $msPrete -ge 0) {
                    if ($finListe -lt 0) { $finListe = $sw.ElapsedMilliseconds + 400 }
                    if ($sw.ElapsedMilliseconds -ge $finListe) { break }
                }
                if ($sw.ElapsedMilliseconds -gt 10000) {
                    $manque = @()
                    if ($nb['Prete'] -lt 1) { $manque += 'chrono selection prete' }
                    if ($nb['DPages'] -lt 1) { $manque += 'defilante pages' }
                    if ($pages -lt 0) { $manque += "selection $id`: pages proposees (ou retenues pour la premiere charge)" }
                    if ($msFenetre -lt 0) { $manque += 'defilante fenetre' }
                    if ($msPrete -lt 0) { $manque += 'defilante page prete' }
                    throw "$Quoi : visée pas prête en 10 s (manque : $($manque -join ', ')) ; $(Diagnostic-Journal $Dossier)"
                }
                Start-Sleep -Milliseconds 20
            }
            # Le focus : la sélection l'a-t-elle eu, et l'a-t-elle gardé après la création de
            # la fenêtre de contrôle ?
            $eu = @($releves | Where-Object { $_.H -eq $hs })
            $premier = if ($eu.Count -gt 0) { $eu[0].Ms } else { -1 }
            $perte = @($releves | Where-Object { $premier -ge 0 -and $_.Ms -gt $premier -and $_.H -ne $hs })
            $suite = ($releves | ForEach-Object { "$($_.Ms) ms : $(if ($_.H -eq $hs) { 'sélection' } elseif ($_.H -eq $hc -and $hc -ne [IntPtr]::Zero) { 'FENÊTRE DE CONTRÔLE' } else { $NATIF::Decrire($_.H) })" }) -join ' ; '
            $focus = [pscustomobject]@{
                Eu        = ($premier -ge 0)
                Perdu     = ($perte.Count -gt 0)
                PerduApres = (@($perte | Where-Object { $_.Ms -ge $msFenetre }).Count -gt 0)
                Texte     = "premier plan relevé de « selection montree » au clic : $suite ; fenêtre de contrôle créée à $msFenetre ms, sa page prête à $msPrete ms"
            }
            if (-not $focus.Eu) {
                Write-Host "[focus] $Quoi : la sélection $id n'a jamais eu le premier plan pendant la visée : l'effet de la fenêtre de contrôle sur le focus n'est pas observable ici. $($focus.Texte)"
            }
            elseif ($focus.Perdu) {
                Write-Host "[focus] $Quoi : la sélection $id a PERDU le premier plan $(if ($focus.PerduApres) { 'après' } else { 'avant' }) la création de la fenêtre de contrôle. $($focus.Texte)"
            }
            else {
                Write-Host "[focus] $Quoi : la sélection $id a gardé le premier plan pendant toute la visée, création de la fenêtre de contrôle comprise. $($focus.Texte)"
            }
            # Les pages : ce que l'app a cherché (B : `defiler pages:` ; E et F : `defilante pages`).
            $dp = @(Apres $Dossier 'DefPages' $Base)
            $dl = @(Apres $Dossier 'DPages' $Base)
            $lignePages = if ($dp.Count) { Message $dp[-1] } else { '<aucune ligne « defiler pages: »>' }
            Write-Host "[pages] $Quoi : $lignePages ; $(if ($dl.Count) { Message $dl[-1] }) ; $pages page(s) proposée(s) à la sélection $id"
            # Les mesures de la recherche (lancement, COM et client, chaque fenêtre : classe,
            # ms, éléments ; tardives ; fin) : recopiées ici, et de nouveau par Attendre-Pages.
            $mesures = @(Lignes-Mesure $Dossier $Base)
            foreach ($m in $mesures) { Write-Host "[pages] $Quoi : $m" }
            return [pscustomobject]@{ Id = $id; Selection = $hs; Controle = $hc; Pages = $pages; Focus = $focus; Base = $Base; LignePages = $lignePages; Dossier = $Dossier; Mesures = $mesures }
        }

        # Les lignes de mesure de la recherche des pages venues après les compteurs $Base
        # (`defiler pages:`, `defiler pages detail|tardives|fin:`), sans horodatage.
        function Lignes-Mesure([string]$Dossier, $Base) {
            # Les deux motifs sont disjoints : les lignes d'avant $Base sont les premières.
            $t = @(Lignes-Journal $Dossier | Where-Object { $_ -match $MOTIFS['DefPages'] -or $_ -match $MOTIFS['DefMesure'] })
            $deja = $Base['DefPages'] + $Base['DefMesure']
            if ($t.Count -le $deja) { return @() }
            return @($t[$deja..($t.Count - 1)] | ForEach-Object { Message $_ })
        }

        # Le nombre de pages proposées à la sélection de $Visee : le plus grand n de
        # `selection <id>: pages proposees <n>` (la liste grandit quand UI Automation répond
        # en retard), ou, si la liste a été retenue pour la première charge, celui de
        # `defilante pages <n>` ; -1 sans rien.
        function Pages-Proposees($Visee) {
            $lignes = Lignes-Journal $Visee.Dossier
            $n = -1
            foreach ($l in $lignes) {
                if ($l -match "selection $($Visee.Id)`: pages proposees (\d+)$") { $n = [math]::Max($n, [int]$Matches[1]) }
            }
            if (@($lignes | Where-Object { $_ -match "selection $($Visee.Id)`: pages retenues pour la premiere charge$" }).Count -gt 0) {
                foreach ($l in @(Apres $Visee.Dossier 'DPages' $Visee.Base)) { $n = [math]::Max($n, (Nombre $l 'defilante pages (\d+) en ')) }
            }
            return $n
        }

        # Avant un clic sur une page : la page doit avoir été proposée à la sélection (ligne du
        # journal), attendue $DelaiS s au plus après la visée prête (les pages d'UI Automation
        # peuvent suivre la première liste, jusqu'à 1,5 s après son début) ; venue en retard,
        # 200 ms de plus pour que la page de sélection la prenne (une image). Met à jour
        # $Visee.Pages et $Visee.Mesures, écrit les mesures venues depuis la visée ; rend le
        # nombre de pages.
        function Attendre-Pages($Visee, [string]$Quoi, [double]$DelaiS = 2) {
            $n = Pages-Proposees $Visee
            if ($n -lt 1) {
                $sw = [System.Diagnostics.Stopwatch]::StartNew()
                while ($n -lt 1 -and $sw.Elapsed.TotalSeconds -lt $DelaiS) {
                    Start-Sleep -Milliseconds 30
                    $n = Pages-Proposees $Visee
                }
                if ($n -ge 1) {
                    Start-Sleep -Milliseconds 200
                    Write-Host "[pages] $Quoi : $n page(s) proposée(s) en retard, $($sw.ElapsedMilliseconds) ms après la visée prête"
                }
            }
            $Visee.Pages = [math]::Max($Visee.Pages, $n)
            $mesures = @(Lignes-Mesure $Visee.Dossier $Visee.Base)
            if ($mesures.Count -gt $Visee.Mesures.Count) {
                foreach ($m in $mesures[$Visee.Mesures.Count..($mesures.Count - 1)]) { Write-Host "[pages] $Quoi : $m" }
            }
            $Visee.Mesures = $mesures
            return $Visee.Pages
        }

        # Un clic sur une page n'a de sens que si des pages ont été proposées (attendues 2 s
        # au plus, Attendre-Pages) : sinon un échec qui le dit, avec les mesures de la
        # recherche. La durée de la première liste (B, budget de 300 ms) est notée avec sa marge.
        function Exiger-Pages($Visee, [string]$Quoi) {
            $n = Attendre-Pages $Visee $Quoi
            $ms = Nombre $Visee.LignePages ' en (\d+) ms$'
            $retard = Nombre $Visee.LignePages 'en retard (\d+)\)'
            if ($ms -ge 0) { Noter-Mesure -Nom "$Quoi recherche des pages (budget de B 300 ms ; marge = 300 - durée ; en retard $retard)" -Valeur "$ms ms, marge $(300 - $ms) ms" }
            if ($n -lt 1) {
                $mesures = if ($Visee.Mesures.Count) { $Visee.Mesures -join ' | ' } else { $Visee.LignePages }
                throw "$Quoi : aucune page proposée à la sélection en 2 s après la visée prête : le clic au centre du témoin ne viserait rien$(if ($ms -gt 300) { ' (recherche des pages hors budget)' }) ; mesures : $mesures"
            }
        }

        # --- Retour au repos, touches tenues par l'app -------------------------------------
        # L'état de l'app : la dernière ligne `etat <avant> -> <après> (…)` ; Repos sans ligne.
        function Etat-App([string]$Dossier) {
            $l = Derniere-Ligne 'etat \w+ -> \w+ ' $Dossier
            if ($l -and $l -match 'etat \w+ -> (\w+) ') { return $Matches[1] }
            return 'Repos'
        }
        # Vrai si la dernière réservation de la touche n'a pas été libérée depuis.
        function Touche-Tenue([string]$Dossier, [string]$Touche) {
            if ($Touche -eq 'Echap') { $r = 'EchapR'; $l = 'EchapL' } else { $r = 'EntreeR'; $l = 'EntreeL' }
            return (Compter-Motif $MOTIFS[$r] $Dossier) -gt (Compter-Motif $MOTIFS[$l] $Dossier)
        }
        # Échap ou Entrée, envoyée en raccourci global une fois que l'app la tient
        # (RegisterHotKey : la touche va à l'app quel que soit le premier plan, comme dans
        # l'essai 13) ; attendue 3 s au plus. Le premier plan du moment est écrit.
        function Appuyer-Reservee([string]$Dossier, [string]$Touche, [string]$Quoi) {
            $limite = [DateTime]::UtcNow.AddSeconds(3)
            while (-not (Touche-Tenue $Dossier $Touche) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 30 }
            $fg = $NATIF::GetForegroundWindow()
            $plan = "$($NATIF::Decrire($fg)) « $($NATIF::Titre($fg)) »"
            if (-not (Touche-Tenue $Dossier $Touche)) {
                throw "$Quoi : $Touche non tenue par l'app en 3 s (réservation absente ou libérée) : la touche n'est pas envoyée ; premier plan $plan ; $(Diagnostic-Journal $Dossier)"
            }
            $NATIF::AppuyerGlobal($AUCUN, $(if ($Touche -eq 'Echap') { $ECHAP } else { $ENTREE }))
            Write-Host "[touche] $Quoi : $Touche envoyée en raccourci global (tenue par l'app) ; premier plan $plan"
        }
        # Après l'échec d'un point : la souris au repos, l'app ramenée au repos (Échap global
        # si la sélection ou la course le tient ; sinon, ou sans effet en 5 s, l'app relancée
        # dans le dossier du cas), puis le témoin du cas en haut.
        $script:Cas = $null
        function Revenir-Au-Repos([string]$Quoi) {
            $casCourant = $script:Cas
            if ($null -eq $casCourant) { return }
            try {
                $NATIF::Poser($REPOS_X, $REPOS_Y)
                $NATIF::BoutonHaut()
                $repos = { (-not $script:proc.HasExited) -and ((Etat-App $casCourant.Dossier) -in 'Repos', 'Vignette') }
                $dit = "état $(Etat-App $casCourant.Dossier)"
                if (-not (& $repos)) {
                    $echapEnvoye = $false
                    $limite = [DateTime]::UtcNow.AddSeconds(5)
                    while (-not (& $repos) -and -not $script:proc.HasExited -and [DateTime]::UtcNow -lt $limite) {
                        if (-not $echapEnvoye -and (Touche-Tenue $casCourant.Dossier 'Echap')) { $NATIF::AppuyerGlobal($AUCUN, $ECHAP); $echapEnvoye = $true; $dit += ', Échap global' }
                        Start-Sleep -Milliseconds 50
                    }
                }
                if (-not (& $repos)) {
                    $dit += ", app relancée (état $(Etat-App $casCourant.Dossier))"
                    Arreter-Tout
                    $script:proc = Demarrer-Cas -Dossier $casCourant.Dossier -Quoi "$Quoi (relance)" -Capturable:([bool]$casCourant.Capturable) -AvecLicence:([bool]$casCourant.AvecLicence)
                    $script:EcranFige = Hwnd-Journal 'ecran fige fenetre' $casCourant.Dossier
                    if ($casCourant.AvecLicence) { $null = Attendre-Motif 'raccourci defilante: ' $casCourant.Dossier 10 "$Quoi (relance) : raccourci defilante" }
                }
                $t = $script:t
                if ($null -ne $t -and -not $t.Aide.HasExited) { Remonter $t "$Quoi (retour au repos)"; $dit += ', témoin en haut' }
                Write-Host "[repos] $Quoi : retour au repos : $dit"
            }
            catch { Write-Host "[repos] $Quoi : retour au repos impossible : $_" }
        }

        # --- Pages de référence (BMP écrits par le témoin lui-même, même formule) --------------
        $REFS = Join-Path $script:Essai.Dossier 'reference'
        $script:References = @{}
        function Ecrire-Reference([int]$NbLignes) {
            if (Test-Path -LiteralPath (Join-Path $REFS "page-$NbLignes.bmp")) { return }
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -NoNewWindow -ArgumentList @('-NoProfile', '-File', "`"$(Join-Path $PSScriptRoot 'temoin-defilant.ps1')`"", '-Png', "`"$REFS`"", '-Lignes', "$NbLignes")
            if (-not $aide.WaitForExit(120000) -or -not (Test-Path -LiteralPath (Join-Path $REFS "page-$NbLignes.bmp"))) { throw "page de référence de $NbLignes lignes non écrite en 120 s" }
        }
        function Reference([string]$Nom) {
            if (-not $script:References.ContainsKey($Nom)) { $script:References[$Nom] = Octets-Fichier (Join-Path $REFS $Nom) }
            return $script:References[$Nom]
        }
        # L'image, à partir de sa rangée $Oy, contre la référence $Nom (ses $Image.H - $Oy
        # premières rangées au plus) : égale à ± 8 par canal, pixel pour pixel.
        function Verifier-Reference([string]$Quoi, $Image, [int]$Oy, [string]$Nom) {
            $ref = Reference $Nom
            $hr = [math]::Min($ref.H, $Image.H - $Oy)
            $e = $NATIF::EcartReference($Image.O, $Image.L, $Image.H, $Oy, $ref.O, $ref.L, $hr, 8)
            $detail = if ($e[0] -lt 0) { "comparaison impossible : image $($Image.L)x$($Image.H), référence $($ref.L)x$($ref.H), à partir de la rangée $Oy" }
            else { "$($e[0]) pixel(s) différents de plus de 8 sur $($e[3]) comparés (rangées $Oy à $($Oy + $hr - 1) de l'image contre $Nom)$(if ($e[0] -gt 0) { " ; le premier en $($e[1]),$($e[2])" })" }
            Verifier "$Quoi égal à la référence $Nom (± 8 par canal)" ($e[0] -eq 0 -and $hr -ge 1) $detail | Out-Null
        }

        # Le choix : clic réel au point $Clic, ou glisser réel $Trace (x0, y0, x1, y1, bornes
        # comprises) ; l'écran figé guetté depuis le relâcher ; la souris posée ensuite au
        # repos, ou en $PoserApres. Un geste pris pour une ancienne charge est refait une
        # fois, et dit. Rend { Zone, Sorte, Agrandie, CacheMs,
        # PretAvant, Ligne } ; $null avec -SansReprise si le clic ne vise aucune page.
        function Choisir([string]$Dossier, $Visee, [string]$Quoi, [int[]]$Clic = $null, [int[]]$Trace = $null, [switch]$SansReprise, [int[]]$PoserApres = $null) {
            $Cpt = $Visee.Base
            $pretAvant = @(Apres $Dossier 'PagePrete' $Cpt).Count -ge 1
            $motifIgnore = "selection $($Visee.Id)`: (clic d'une ancienne charge \d+ ignore|clic sur aucune fenetre ignore|zone d'une ancienne charge \d+ ignoree|zone trop petite ignoree)$"
            $ms = -2; $zl = $null
            for ($tour = 1; $tour -le 2 -and $null -eq $zl; $tour++) {
                $ignores = Compter-Motif $motifIgnore $Dossier
                if ($null -ne $Clic) { $ms = $NATIF::CliquerPuisGuetter($Clic[0], $Clic[1], $script:EcranFige, 2500) }
                else { $ms = $NATIF::TracerPuisGuetter($Trace[0], $Trace[1], $Trace[2], $Trace[3], $script:EcranFige, 2500) }
                if ($null -ne $PoserApres) { $NATIF::Poser($PoserApres[0], $PoserApres[1]) } else { $NATIF::Poser($REPOS_X, $REPOS_Y) }
                $limite = [DateTime]::UtcNow.AddSeconds(3)
                $ignore = $null
                do {
                    $z = @(Apres $Dossier 'Zone' $Cpt)
                    if ($z.Count -ge 1) { $zl = $z[-1]; break }
                    if ((Compter-Motif $motifIgnore $Dossier) -gt $ignores) { $ignore = Derniere-Ligne $motifIgnore $Dossier; break }
                    Start-Sleep -Milliseconds 30
                } while ([DateTime]::UtcNow -lt $limite)
                if ($null -ne $zl) { break }
                if ($null -ne $ignore -and $SansReprise) { Write-Host "[note] $Quoi : geste ignoré ($(Message $ignore))"; return $null }
                if ($null -ne $ignore -and $tour -eq 1) {
                    Write-Host "[note] $Quoi : premier geste ignoré ($(Message $ignore)), refait 600 ms plus tard"
                    Start-Sleep -Milliseconds 600
                    continue
                }
                throw "$Quoi : aucune ligne « defilante zone » après le geste $(if ($ignore) { "($(Message $ignore))" }) ; $(Diagnostic-Journal $Dossier)"
            }
            $msg = Message $zl
            if ($msg -notmatch 'defilante zone (-?\d+,-?\d+ \d+x\d+) (page|trace) agrandie (oui|non)$') { throw "$Quoi : ligne illisible : $msg" }
            return [pscustomobject]@{ Zone = (Lire-Rect $Matches[1]); Sorte = $Matches[2]; Agrandie = $Matches[3]; CacheMs = $ms; PretAvant = $pretAvant; Ligne = $msg }
        }

        # Attend l'issue de la course (fin, annulée, impossible), en relevant le premier plan
        # (jamais la fenêtre de contrôle) et, avec -Memoire, la mémoire de l'app et de sa
        # WebView2. Rend { Ligne, Message, MemMax, Releves, ControleDevant, Plans }.
        function Attendre-Issue([string]$Dossier, $Cpt, [string]$Quoi, [double]$DelaiS, [IntPtr]$Controle, [switch]$Memoire) {
            $sw = [System.Diagnostics.Stopwatch]::StartNew()
            $max = 0.0; $n = 0; $devant = 0; $plans = @{}; $prochaine = 0
            do {
                $l = @(Apres $Dossier 'Issue' $Cpt)
                if ($l.Count -ge 1) {
                    return [pscustomobject]@{ Ligne = $l[0]; Message = (Message $l[0]); MemMax = $max; Releves = $n; ControleDevant = $devant; Plans = (@($plans.Keys) -join ' ; ') }
                }
                $fg = $NATIF::GetForegroundWindow()
                if ($Controle -ne [IntPtr]::Zero -and $fg -eq $Controle) { $devant++ }
                $plans[$NATIF::Decrire($fg)] = $true
                if ($Memoire -and $sw.ElapsedMilliseconds -ge $prochaine) {
                    $mo = Memoire-Privee-Mo -Processus $script:proc
                    if ($mo -gt $max) { $max = $mo }
                    $n++
                    $prochaine = $sw.ElapsedMilliseconds + 800
                }
                Start-Sleep -Milliseconds 30
            } while ($sw.Elapsed.TotalSeconds -lt $DelaiS)
            throw "$Quoi : ni fin, ni annulation, ni impossibilité en $DelaiS s ; $(Diagnostic-Journal $Dossier)"
        }

        # Après une fin : assemblée (avec -Memoire, la mémoire relevée jusqu'à elle), chrono,
        # Défilement -> Découpe, la fenêtre de contrôle cachée, copie, chrono de la découpe,
        # et le nouveau PNG de « Récents ». Rend { Assemblee, Chrono, Copie, Decoupe, Png,
        # MemMax, Releves }.
        function Attendre-Sortie([string]$Dossier, $Cpt, [string[]]$PngAvant, [string]$Quoi, [IntPtr]$Controle = [IntPtr]::Zero, [switch]$Memoire) {
            $max = 0.0; $releves = 0; $a = $null
            $limite = [DateTime]::UtcNow.AddSeconds(15)
            do {
                $l = @(Apres $Dossier 'Assemblee' $Cpt)
                if ($l.Count -ge 1) { $a = $l[0]; break }
                if ($Memoire) {
                    $mo = Memoire-Privee-Mo -Processus $script:proc
                    if ($mo -gt $max) { $max = $mo }
                    $releves++
                }
                Start-Sleep -Milliseconds 60
            } while ([DateTime]::UtcNow -lt $limite)
            if ($null -eq $a) { throw "$Quoi : rien dans le journal en 15 s (motif « $($MOTIFS['Assemblee']) ») ; $(Diagnostic-Journal $Dossier)" }
            $c = Attendre-Cle $Dossier 'Chrono' $Cpt "$Quoi : chrono defilante" 5
            $null = Attendre-Cle $Dossier 'Fini' $Cpt "$Quoi : etat Defilement -> Decoupe" 5
            if ($Controle -ne [IntPtr]::Zero) {
                $fin = [DateTime]::UtcNow.AddSeconds(2)
                while ($NATIF::IsWindowVisible($Controle) -and [DateTime]::UtcNow -lt $fin) { Start-Sleep -Milliseconds 20 }
                Verifier "$Quoi fenêtre de contrôle cachée à la fin (2 s au plus)" (-not $NATIF::IsWindowVisible($Controle)) ($NATIF::Decrire($Controle)) | Out-Null
            }
            $cp = Attendre-Cle $Dossier 'Copie' $Cpt "$Quoi : sortie copie" 15
            $dc = Attendre-Cle $Dossier 'Decoupe' $Cpt "$Quoi : chrono decoupe" 15
            $png = Nouveau-Png $Dossier $PngAvant $Quoi
            Write-Host "[chrono] $Quoi : $(Message $a) ; $(Message $c) ; $(Message $dc)"
            return [pscustomobject]@{ Assemblee = (Message $a); Chrono = (Message $c); Copie = (Message $cp); Decoupe = (Message $dc); Png = $png; MemMax = $max; Releves = $releves }
        }

        # Les moyens de la course (`defilante moyen`, `repli:`, `main`), dans l'ordre.
        function Moyens([string]$Dossier, $Cpt) {
            $l = @(Lignes-Journal $Dossier | Where-Object { $_ -match $MOTIFS['Moyen'] -or $_ -match $MOTIFS['Repli'] -or $_ -match $MOTIFS['Main'] })
            $avant = $Cpt['Moyen'] + $Cpt['Repli'] + $Cpt['Main']
            if ($l.Count -le $avant) { return '<aucun>' }
            return (@($l[$avant..($l.Count - 1)] | ForEach-Object { Message $_ }) -join ' → ')
        }

        # Styles étendus de la fenêtre de contrôle (souris loin de la pilule).
        function Verifier-Styles([IntPtr]$Controle, [string]$Quoi) {
            $ex = [long]$NATIF::GetWindowLongPtr($Controle, -20)
            $ok = (($ex -band $WS_EX.TOOLWINDOW) -ne 0) -and (($ex -band $WS_EX.NOACTIVATE) -ne 0) -and (($ex -band $WS_EX.TOPMOST) -ne 0) -and (($ex -band $WS_EX.APPWINDOW) -eq 0) -and (($ex -band $WS_EX.TRANSPARENT) -ne 0)
            Verifier "$Quoi fenêtre de contrôle : outil, sans activation, au-dessus, sans APPWINDOW, traversée loin de la pilule" $ok ('style étendu 0x{0:X8}' -f $ex) | Out-Null
        }

        # --- Suites d'images pour le chantier C -------------------------------------------------
        # Écrit dans la sortie les résumés de lignes de chaque image et, pour chaque paire, le
        # vrai décalage (lu sur le témoin si l'image se décode, sinon le meilleur au sens de
        # D12), son score D12, et son rang parmi les décalages classés sur les seuls résumés.
        function Ecrire-Suite([string]$Nom, $Serie, [switch]$Decodable) {
            $n = $Serie.Count
            if ($n -lt 2) { Write-Host "[note] suite $Nom : $n image(s), rien à écrire"; return }
            $l = $Serie[0].L; $h = $Serie[0].H
            Write-Host "suite $Nom debut : $n images $($l)x$($h) ; une rangée = `decrire` de C, écart permis 8 : 16 blocs du résumé (canaux d'encre du bloc rapportés à 255, paquets de 64 octets), 2 chiffres hexadécimaux chacun, puis g<gris du fond> e<canaux d'encre> p<porteuse 0|1> ; paire i-j : la rangée y de j montre la rangée y+d de i"
            $res = [System.Collections.Generic.List[object]]::new()
            for ($i = 0; $i -lt $n; $i++) {
                $r = $NATIF::DecrireC($Serie[$i].O, $l, $h, 8)
                $res.Add($r)
                $t = $NATIF::TexteResumes($r, $h)
                $sb = [System.Text.StringBuilder]::new()
                for ($y = 0; $y -lt $h; $y++) { [void]$sb.Append("resume $Nom $i $y $($t[$y])`n") }
                Write-Host -NoNewline $sb.ToString()
                Write-Host "suite $Nom image $i porteuses $($NATIF::Porteuses($r, $h)) sur $h"
            }
            $justes = 0; $premiers = 0; $dans8 = 0; $vus = 0
            for ($i = 0; $i -lt $n - 1; $i++) {
                $a = $Serie[$i]; $b = $Serie[$i + 1]
                $vrai = $null; $source = ''
                if ($Decodable) {
                    $la = $NATIF::Decoder($a.O, $l, $h, 0, 0, $h); $lb = $NATIF::Decoder($b.O, $l, $h, 0, 0, $h)
                    for ($y = 0; $y -lt $h; $y++) { if ($la[$y] -ge 0 -and $lb[$y] -ge 0) { $vrai = $lb[$y] - $la[$y]; $source = 'témoin'; break } }
                }
                if ($null -eq $vrai) {
                    $d = $NATIF::MeilleurD12($a.O, $b.O, $l, $h, $h - 32)
                    if ($d -ne [int]::MinValue) { $vrai = $d; $source = 'meilleur D12' }
                }
                $cl = $NATIF::ClasserResumes($res[$i], $res[$i + 1], $h, 32)
                $tete = ($cl | Select-Object -First 8) -join ','
                if ($null -eq $vrai) { Write-Host "suite $Nom paire $i-$($i + 1) vrai inconnu ; resumes premiers $tete"; continue }
                $score = $NATIF::ScoreD12($a.O, $b.O, $l, $h, $vrai)
                $rang = [array]::IndexOf($cl, [int]$vrai) + 1
                $vus++
                if ($score -ge 0.97) { $justes++ }
                if ($rang -eq 1) { $premiers++ }
                if ($rang -ge 1 -and $rang -le 8) { $dans8++ }
                Write-Host ("suite $Nom paire $i-$($i + 1) vrai $vrai ($source) d12 {0:N3} resumes-rang $rang premiers $tete" -f $score)
            }
            Write-Host "suite $Nom fin : $vus paire(s) à décalage connu ; D12 ≥ 97 % au vrai décalage : $justes ; vrai décalage premier sur les résumés : $premiers ; dans les 8 premiers : $dans8"
            Noter-Mesure -Nom "suite $Nom : paires justes D12 / vrai premier des résumés / dans les 8 premiers / paires" -Valeur "$justes / $premiers / $dans8 / $vus"
        }
        # La suite d'un témoin, d'Edge ou de l'Explorateur : $Combien images de la zone $ZoneSuite, $Crans
        # crans de molette de l'essai entre deux (point $Ou), souris posée loin ensuite.
        function Prendre-Suite($ZoneSuite, [int[]]$Ou, [int]$Combien, [int]$Crans, [string]$Quoi) {
            $images = [System.Collections.Generic.List[object]]::new()
            for ($i = 0; $i -lt $Combien; $i++) {
                $NATIF::Poser($REPOS_X, $REPOS_Y)
                $images.Add((Copie-Stable $ZoneSuite.X $ZoneSuite.Y $ZoneSuite.L $ZoneSuite.H "$Quoi image $i"))
                if ($i -lt $Combien - 1) { $NATIF::Molette($Ou[0], $Ou[1], $Crans, -120, 40) }
            }
            $NATIF::Poser($REPOS_X, $REPOS_Y)
            return $images
        }

        # ===================== L0 : sans licence =================================================
        try {
            $d0 = Dossier-Cas 'l0'
            Ecrire-Reglages -Dossier $d0 -Vignette $false
            $script:proc = Demarrer-Cas -Dossier $d0 -Quoi '(L0)'
            $script:Cas = [pscustomobject]@{ Dossier = $d0; Capturable = $false; AvecLicence = $false }
            Point '(L0) sans licence' {
                Verifier '(L0) licence absente' ((Compter-Motif 'licence absente$' $d0) -eq 1) | Out-Null
                Verifier '(L0) aucune ligne « raccourci defilante »' ((Compter-Motif 'raccourci defilante[: ]' $d0) -eq 0) (Message (Derniere-Ligne 'raccourci defilante' $d0)) | Out-Null
                $m = Ouvrir-Menu '(L0)'
                $sans = @($m.Textes | Where-Object { $_.StartsWith('Capture défilante', [StringComparison]::Ordinal) }).Count -eq 0
                Verifier '(L0) menu de 9 positions (séparateurs comptés) sans « Capture défilante »' ($sans -and ($m.Textes.Count -eq 9)) $m.Lus | Out-Null
                Fermer-Menu $m '(L0)'
                Verifier '(L0) Ctrl+Maj+5 libre (l''essai le tient)' ((Tenir-Ailleurs ([int]$VK_5) 'cinq-l0') -eq 'tenu') | Out-Null
                $recus = Compter-Motif '--capturer recu: defilante$' $d0
                $non = Compter-Motif '--capturer: mode non construit$' $d0
                $r = Lancer-Et-Attendre -Arguments @('--capturer', 'defilante') -DelaiS 15 -Dossier $d0
                if (-not $r.Fini) { throw '(L0) --capturer defilante : la seconde instance ne sort pas en 15 s' }
                $null = Attendre-Suivante '--capturer recu: defilante$' $d0 $recus '(L0) --capturer recu: defilante' 5
                $null = Attendre-Suivante '--capturer: mode non construit$' $d0 $non '(L0) --capturer: mode non construit' 5
                Verifier '(L0) --capturer defilante → « mode non construit », aucune capture' ((Compter-Motif 'DemandeCapture defilante' $d0) -eq 0) | Out-Null
                $f = Lire-Reglages $d0
                $def = if ($f.raccourcis.PSObject.Properties.Name -contains 'defilante') { @($f.raccourcis.defilante) -join ',' } else { '<absent>' }
                $mises = Compter-Motif 'reglages mis a niveau de la version 4 a la version 5$' $d0
                Verifier '(L0) réglages passés en v5 : defilante = Ctrl+Maj+5, une ligne « de la version 4 a la version 5 »' (($f.version -eq 5) -and ($def -eq 'Ctrl+Maj+5') -and ($mises -eq 1)) "version $($f.version), defilante $def, lignes $mises" | Out-Null
                Verifier '(L0) aucune fenêtre « defilante »' ((Compter-Motif $MOTIFS['Fenetre'] $d0) -eq 0) | Out-Null
            }
            Start-Sleep -Seconds 2   # la mémoire se pose (aucune ligne ne le dit)
            Noter-Mesure -Nom '(L0) mémoire privée au repos sans licence (app et WebView2 ; 0.5 : 174,1 Mo)' -Valeur (Memoire-Privee-Mo -Processus $script:proc) -Unite 'Mo'
            Verifier-Interdites $d0 '(L0)'
            Fermer-Cas $script:proc $d0 '(L0)'
        }
        catch { Verifier '(L0) cas sans erreur inattendue' $false "$_" | Out-Null }
        finally { Arreter-Tout }

        # ===================== Cas A : licence, non capturable ===================================
        $sentinelle = $null; $t = $null
        try {
            $da = Dossier-Cas 'a'
            Ecrire-Reglages -Dossier $da -Vignette $true
            Poser-Licence $da
            Ecrire-Reference 120
            $sentinelle = Lancer-Temoin 'sentinelle' 760 80 @('-Largeur', '240', '-Hauteur', '300')
            $t = Lancer-Temoin 'page' 40 80
            Permettre @($t.Pid, $sentinelle.Pid)
            $script:proc = Demarrer-Cas -Dossier $da -Quoi '(A)' -AvecLicence
            $script:Cas = [pscustomobject]@{ Dossier = $da; Capturable = $false; AvecLicence = $true }
            $script:EcranFige = Hwnd-Journal 'ecran fige fenetre' $da
            $null = Attendre-Motif 'raccourci defilante: ' $da 10 '(L1) raccourci defilante'
            Start-Sleep -Seconds 3   # la mémoire se pose (aucune ligne ne le dit)
            $reposA = Memoire-Privee-Mo -Processus $script:proc
            Noter-Mesure -Nom '(L1) mémoire privée au repos avec licence (app et WebView2 ; 0.5 : 190,3 Mo)' -Valeur $reposA -Unite 'Mo'

            Point '(L1) licence' {
                Verifier '(L1) raccourci defilante: Ctrl+Maj+5' ((Compter-Motif 'raccourci defilante: Ctrl\+Maj\+5$' $da) -eq 1) (Message (Derniere-Ligne 'raccourci defilante' $da)) | Out-Null
                $m = Ouvrir-Menu '(L1)'
                $ok = ($m.Textes.Count -ge 6) -and ($m.Textes[5] -ceq "Capture défilante`tCtrl + Maj + 5")
                Verifier '(L1) menu : « Capture défilante<tab>Ctrl + Maj + 5 » à l''indice 5 (séparateur compté)' $ok $m.Lus | Out-Null
                Fermer-Menu $m '(L1)'
                Verifier '(L1) aucune fenêtre « defilante » au repos' ((Compter-Motif $MOTIFS['Fenetre'] $da) -eq 0) | Out-Null
            }

            # ---------- (A1) à (A9) : la défilante par le raccourci, seule jusqu'en bas -------
            Point '(A) défilante automatique' {
                Remonter $t '(A)'
                $avantS = Copie-Ecran $sentinelle.Fenetre.X $sentinelle.Fenetre.Y $sentinelle.Fenetre.L $sentinelle.Fenetre.H
                $avantPng = Recents $da
                $nPos = @(Positions $t).Count
                $Cpt = Compteurs $da
                $Visee = Viser $da 'raccourci' '(A1)' $Cpt
                $null = Attendre-Pages $Visee '(A1)'
                Verifier "(A1) visée prête : sélection en mode défilante, $($Visee.Pages) page(s) proposée(s), fenêtre de contrôle et sa page prêtes" ($Visee.Pages -ge 1) | Out-Null
                # Le premier plan pendant la visée est noté (lignes [focus]) : le terminal de la
                # machine peut le prendre sans faute de l'app ; le contrat demande l'absence de
                # « annulee focus_perdu », vérifiée après le clic.
                Noter-Mesure -Nom '(A1) focus pendant la création de la fenêtre de contrôle (noté)' -Valeur $(if (-not $Visee.Focus.Eu) { 'non observable : la sélection n''a pas eu le premier plan' } elseif ($Visee.Focus.Perdu) { "PERDU $(if ($Visee.Focus.PerduApres) { 'après' } else { 'avant' }) la création" } else { 'gardé' })
                Exiger-Pages $Visee '(A1)'

                $Choix = Choisir $da $Visee '(A2)' -Clic @($t.Page.CX, $t.Page.CY)
                Verifier '(A1) aucune « selection fin … annulee focus_perdu »' ((Compter-Motif "selection fin $($Visee.Id) annulee focus_perdu" $da) -eq 0) | Out-Null
                $z = $Choix.Zone
                $egale = ([math]::Abs($z.X - $t.Page.X) -le 2) -and ([math]::Abs($z.Y - $t.Page.Y) -le 2) -and ([math]::Abs($z.L - $t.Page.L) -le 2) -and ([math]::Abs($z.H - $t.Page.H) -le 2)
                Verifier '(A2) clic sur la page : zone = page du témoin (2 px près), page, agrandie non' ($egale -and $Choix.Sorte -eq 'page' -and $Choix.Agrandie -eq 'non') "$($Choix.Ligne) ; page du témoin $(Texte-Rect $t.Page)" | Out-Null
                Verifier '(A2) écran figé caché ≤ 450 ms après le relâcher' (($Choix.CacheMs -ge 0) -and ($Choix.CacheMs -le 450)) "$($Choix.CacheMs) ms (-1 : déjà caché au relâcher)" | Out-Null
                $null = Attendre-Cle $da 'Choisi' $Cpt '(A2) Selection -> Defilement' 5
                $premiere = Attendre-Cle $da 'Premiere' $Cpt '(A2) première image' 5
                $debutPrise = (Nombre (Message $premiere) '\(relache (\d+)\)$') - (Nombre (Message $premiere) ' en (\d+) ms ')
                Noter-Mesure -Nom '(A2) écran figé caché (horloge de l''essai, depuis son relâcher) / début de la première prise (horloge de l''app : relache - prise)' -Valeur "$($Choix.CacheMs) ms / $debutPrise ms"
                $hc = $Visee.Controle
                Verifier '(A2) fenêtre de contrôle montrée' ($NATIF::IsWindowVisible($hc)) ($NATIF::Decrire($hc)) | Out-Null
                Verifier-Styles $hc '(A2)'
                $excl = @(Apres $da 'Exclue' $Cpt)
                Verifier '(A2) defilante exclue des captures: oui' (($excl.Count -ge 1) -and ($excl[-1] -match 'exclue des captures: oui$')) ($(if ($excl.Count) { Message $excl[-1] } else { '<aucune ligne>' })) | Out-Null
                # (A5) La sentinelle copiée pendant la course : la fenêtre de contrôle (voile
                # sur tout l'écran hors de la zone) est montrée, la copie ne la voit pas.
                $vis1 = $NATIF::IsWindowVisible($hc)
                $pendantS = Copie-Ecran $sentinelle.Fenetre.X $sentinelle.Fenetre.Y $sentinelle.Fenetre.L $sentinelle.Fenetre.H
                $vis2 = $NATIF::IsWindowVisible($hc)
                $part = $NATIF::PartDifferente($avantS.O, $pendantS.O, 6, $false)
                Verifier '(A5) pendant la course, la copie d''écran de la sentinelle ne voit pas la fenêtre de contrôle (< 2 %)' ($vis1 -and $vis2 -and $part -ge 0 -and $part -lt 0.02) ("{0:P1} des points différents, fenêtre visible avant {1} et après {2}" -f $part, $vis1, $vis2) | Out-Null
                $Issue = Attendre-Issue $da $Cpt '(A3)' 60 $hc -Memoire
                $moyens = Moyens $da $Cpt
                Write-Host "[note] (A3) moyens : $moyens ; premier plan pendant la course : $($Issue.Plans)"
                Verifier '(A3) fin en_bas sans aucune action' ($Issue.Message -match '^defilante fin en_bas ') $Issue.Message | Out-Null
                Verifier '(A3) ni « main (molette) » ni « repli: molette »' ((@(Apres $da 'Main' $Cpt).Count -eq 0) -and (@(Apres $da 'Repli' $Cpt | Where-Object { $_ -match 'repli: molette$' }).Count -eq 0)) $moyens | Out-Null
                Verifier '(A2) la fenêtre de contrôle n''a jamais le premier plan' ($Issue.ControleDevant -eq 0) "$($Issue.ControleDevant) relevé(s) sur elle" | Out-Null
                # B saute un pas tant qu'une touche ou un bouton est enfoncé (et le dit une
                # fois) : pendant cet auto, l'essai n'en tient aucun.
                $sautes = Compter-Motif 'defiler pas saute: touche ou bouton' $da
                Verifier '(A3) aucun pas sauté pour une touche ou un bouton tenu' ($sautes -eq 0) | Out-Null
                $Rendu = Attendre-Sortie $da $Cpt $avantPng '(A4)' $hc -Memoire
                $p = @(Positions $t)
                $pCourse = if ($p.Count -gt $nPos) { @($p[$nPos..($p.Count - 1)]) } else { @() }
                $finale = if ($pCourse.Count) { $pCourse[-1].P } else { 0 }
                Verifier '(A3) le témoin est au bas de sa page' ($finale -eq $t.Bas) "position $finale, bas $($t.Bas)" | Out-Null
                $img = $Rendu.Png.Img
                $null = Verifier-Page '(A4) PNG 600 × 6 000 : chaque rangée de la page une fois, dans l''ordre' $img 0 6000
                Verifier-Reference '(A4) PNG 600 × 6 000' $img 0 'page-120.bmp'
                Verifier '(A4) sortie copie faite' ($Rendu.Copie -match 'sortie copie faite') $Rendu.Copie | Out-Null
                $pp = Lire-PressePapiers
                Verifier '(A4) presse-papiers égal au PNG au bit près' (($pp.L -eq $img.L) -and ($pp.H -eq $img.H) -and ($NATIF::Differences($pp.O, $img.O, $img.L, $img.H) -eq 0)) "presse-papiers $($pp.L)x$($pp.H)" | Out-Null
                $null = Attendre-Cle $da 'Vignette' $Cpt '(A4) vignette entree' 10
                Verifier '(A4) vignette entree' $true | Out-Null
                # Budgets.
                $relache = Nombre (Message $premiere) '\(relache (\d+)\)$'
                $prise = Nombre (Message $premiere) ' en (\d+) ms '
                if ($Choix.PretAvant) { Verifier '(A9) départ : relâcher → première image ≤ 500 ms (page de contrôle prête avant le clic)' (($relache -ge 0) -and ($relache -le 500)) "$relache ms" | Out-Null }
                else { Write-Host "[note] (A9) départ $relache ms : page de contrôle pas prête au clic, budget non applicable (R1)" }
                Noter-Mesure -Nom '(A9) prise de la première image 600 × 450 (budget 40 ms en médiane : pas de médiane au journal)' -Valeur $prise -Unite 'ms'
                $mesure = Nombre $Rendu.Chrono 'mesure-mediane (\d+)'
                Verifier '(A9) mesure médiane ≤ 15 ms' (($mesure -ge 0) -and ($mesure -le 15)) $Rendu.Chrono | Out-Null
                # §16.3 : depuis la fin reçue (`fin-assemblee` de la ligne chrono) ; la durée de
                # la ligne `assemblee`, depuis la fin traitée, est notée.
                $finAssemblee = Nombre $Rendu.Chrono 'fin-assemblee (\d+)'
                Verifier '(A9) fin → assemblée ≤ 150 ms (600 × 6 000, fin-assemblee)' (($finAssemblee -ge 0) -and ($finAssemblee -le 150)) $Rendu.Chrono | Out-Null
                Noter-Mesure -Nom '(A9) assemblage seul (ligne « defilante assemblee »)' -Valeur (Nombre $Rendu.Assemblee ' en (\d+) ms$') -Unite 'ms'
                Noter-Mesure -Nom '(A9) pas médian (ligne chrono)' -Valeur (Nombre $Rendu.Chrono 'pas-median (\d+)') -Unite 'ms'
                $annonce = Nombre $Rendu.Decoupe 'relache-annonce (\d+)'
                Verifier '(A9) relâcher → annonce ≤ 1 500 ms (600 × 6 000)' (($annonce -ge 0) -and ($annonce -le 1500)) $Rendu.Decoupe | Out-Null
                if ($pCourse.Count -ge 2) {
                    $duree = ($pCourse[-1].Ms - $pCourse[0].Ms) / 1000.0
                    $vitesse = if ($duree -gt 0) { [math]::Round(($pCourse[-1].P - $pCourse[0].P) / $duree) } else { 0 }
                    # Décision du chef de chantier : la vitesse est mesurée et écrite ; l'essai
                    # échoue sous 800 px/s seulement, 1 500 px/s reste la cible (notée).
                    $detail = "$vitesse px/s, $($pCourse[-1].P - $pCourse[0].P) px en $duree s, $($pCourse.Count) positions"
                    Noter-Mesure -Nom '(A9) vitesse en Auto (cible 1 500 px/s ; du premier au dernier mouvement du témoin)' -Valeur $detail
                    Verifier '(A9) vitesse en Auto ≥ 800 px/s' ($vitesse -ge 800) $detail | Out-Null
                }
                else { Verifier '(A9) vitesse en Auto' $false "$($pCourse.Count) position(s) relevée(s)" | Out-Null }
                $rgba = 600 * 6000 * 4 / 1MB
                $budget = [math]::Round($reposA + 70 + 2.5 * $rgba, 1)
                $memMax = [math]::Max($Issue.MemMax, $Rendu.MemMax)
                Verifier '(A9) mémoire pendant la course et l''assemblage ≤ repos + 70 Mo + 2,5 × RGBA' (($Issue.Releves -ge 1) -and ($memMax -le $budget)) "au plus $memMax Mo sur $($Issue.Releves + $Rendu.Releves) relevé(s) ($($Issue.MemMax) Mo pendant la course, $($Rendu.MemMax) Mo jusqu'à l'assemblée), budget $budget Mo (repos $reposA)" | Out-Null
                Verifier-Avertissements $da $Cpt '(A)'
                Write-Host "[note] (A) $($Issue.Message)"
            }

            # ---------- (E) par le menu, appuis ignorés, Échap ---------------------------------
            Point '(E) menu, appuis ignorés, Échap' {
                Remonter $t '(E)'
                $avantPng = Recents $da
                $Cpt = Compteurs $da
                $Visee = Viser $da 'menu' '(E)' $Cpt
                Exiger-Pages $Visee '(E)'
                $Choix = Choisir $da $Visee '(E)' -Clic @($t.Page.CX, $t.Page.CY)
                $null = Attendre-Cle $da 'Premiere' $Cpt '(E) première image' 5
                # Les deux appuis durent 60 ms chacun (contrat §16.3, 14) : B saute au plus un
                # pas pendant ce temps ; rien n'est tenu ensuite.
                # Pendant la défilante (R2), un raccourci est dit pressé puis ignoré.
                $NATIF::AppuyerGlobal($MODS, $VK_2)
                $null = Attendre-Motif $MOTIFS['Ignore'] $da 3 '(E14) Ctrl+Maj+2 : raccourci ignore pendant la defilante' ($Cpt['Ignore'] + 1)
                $NATIF::AppuyerGlobal($MODS, $VK_5)
                $null = Attendre-Motif $MOTIFS['Ignore'] $da 3 '(E14) Ctrl+Maj+5 : raccourci ignore pendant la defilante' ($Cpt['Ignore'] + 2)
                $presses = @(Apres $da 'Presse' $Cpt | ForEach-Object { Message $_ })
                $suite = ($presses -join ' | ')
                $ordre = ($presses.Count -ge 2) -and ($presses[-2] -eq 'raccourci presse: plein_ecran') -and ($presses[-1] -eq 'raccourci presse: defilante')
                Verifier '(E14) Ctrl+Maj+2 et Ctrl+Maj+5 pendant le défilement : pressés (plein_ecran, defilante) puis « raccourci ignore pendant la defilante » (2), aucune capture' ($ordre -and (@(Apres $da 'Ignore' $Cpt).Count -eq 2) -and (@(Apres $da 'Issue' $Cpt).Count -eq 0) -and (@(Apres $da 'Gel' $Cpt).Count -eq 1)) "pressés : $suite" | Out-Null
                if (@(Apres $da 'Issue' $Cpt).Count -gt 0) { throw "(E13) la course a fini avant Échap : $(Message (@(Apres $da 'Issue' $Cpt))[0])" }
                Appuyer-Reservee $da 'Echap' '(E13)'
                $Issue = Attendre-Issue $da $Cpt '(E13)' 10 $Visee.Controle
                Verifier '(E13) Échap : defilante annulee echap' ($Issue.Message -eq 'defilante annulee echap') $Issue.Message | Out-Null
                $null = Attendre-Cle $da 'Annule' $Cpt '(E13) Defilement -> Repos' 5
                $null = Attendre-Cle $da 'EntreeL' $Cpt '(E13) entree globale liberee' 5
                $null = Attendre-Cle $da 'EchapL' $Cpt '(E13) echap global libere' 5
                Verifier '(E13) echap global presse, Defilement -> Repos, entree globale liberee, echap global libere' (@(Apres $da 'EchapP' $Cpt).Count -ge 1) | Out-Null
                $limite = [DateTime]::UtcNow.AddSeconds(3)
                while (@(Visibles-App).Count -gt 0 -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 50 }
                $vues = @(Visibles-App)
                Verifier '(E13) aucune fenêtre de l''app visible (contrôle, sélection, écran figé, vignette)' ($vues.Count -eq 0) ($vues -join ' ; ') | Out-Null
                Verifier '(E13) aucun PNG nouveau dans « Récents », aucune assemblée' ((@(Recents $da | Where-Object { $avantPng -notcontains $_ }).Count -eq 0) -and (@(Apres $da 'Assemblee' $Cpt).Count -eq 0)) | Out-Null
                Verifier '(E) lancée par le menu : menu: capturer defilante' ((Compter-Motif 'menu: capturer defilante$' $da) -ge 1) | Out-Null
                Verifier-Avertissements $da $Cpt '(E)'
            }

            # ---------- (C) la molette reprend la main --------------------------------------
            Point '(C) molette' {
                Remonter $t '(C)'
                Permettre @($t.Pid)
                $avantPng = Recents $da
                $Cpt = Compteurs $da
                $Visee = Viser $da 'raccourci' '(C)' $Cpt
                Exiger-Pages $Visee '(C)'
                $Choix = Choisir $da $Visee '(C)' -Clic @($t.Page.CX, $t.Page.CY)
                $null = Attendre-Cle $da 'Premiere' $Cpt '(C) première image' 5
                $cx = $t.Page.CX; $cy = $t.Page.CY
                # (C7) L'auto a fait au moins deux pas (450 px) : un cran de l'essai.
                $limite = [DateTime]::UtcNow.AddSeconds(8)
                while ((Position $t) -lt 450 -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 20 }
                if ((Position $t) -lt 450) { throw "(C7) l'auto n'a pas avancé de 450 px en 8 s (position $(Position $t)) ; $(Diagnostic-Journal $da)" }
                if (@(Apres $da 'Issue' $Cpt).Count -gt 0) { throw '(C7) la course a fini avant le premier cran' }
                $NATIF::Molette($cx, $cy, 1, -120, 0)
                $null = Attendre-Cle $da 'Main' $Cpt '(C7) defilante main (molette)' 3
                $p0 = Attendre-Calme $t '(C7)' 300 5
                $k = @(Positions $t).Count
                Start-Sleep -Milliseconds 1000   # fenêtre d'observation : rien ne doit bouger
                $k2 = @(Positions $t).Count
                Verifier '(C7) un cran de l''essai : « main (molette) », puis la page ne bouge plus seule (1 s observée)' ($k2 -eq $k) "$($k2 - $k) mouvement(s) en 1 s, position $p0" | Out-Null
                # (C8) Trois crans lents.
                $avant3 = Position $t
                for ($i = 0; $i -lt 3; $i++) {
                    $n = @(Positions $t).Count
                    $NATIF::Molette($cx, $cy, 1, -120, 0)
                    if ($null -eq (Attendre-Mouvement $t $n 1000)) { throw "(C8) cran lent $($i + 1) : la page du témoin ne bouge pas en 1 s" }
                    Start-Sleep -Milliseconds 300   # un cran « lent » : l'app prend ses images (toutes les 50 ms)
                }
                $apres3 = Attendre-Calme $t '(C8)' 300 5
                Verifier "(C8) trois crans lents : la page avance de trois crans ($(3 * $CRAN) px)" (($apres3 - $avant3) -eq 3 * $CRAN) "de $avant3 à $apres3" | Out-Null
                $pAvant = $apres3
                $NATIF::Molette($cx, $cy, 30, -120, 0)
                $null = Attendre-Cle $da 'TropVite' $Cpt '(C8) defilante trop vite' 3
                $pHaut = Attendre-Calme $t '(C8)' 300 5
                Verifier '(C8) 30 crans d''un coup : « trop vite »' $true "de $pAvant à $pHaut" | Out-Null
                $msg = Element $Visee.Controle $TEXTE_TROP_VITE $null 5
                Verifier "(C8) message « $TEXTE_TROP_VITE » montré (UI Automation, fenêtre de contrôle)" (Montre $msg) $(if (-not (Montre $msg)) { Elements-Vus $Visee.Controle } else { '' }) | Out-Null
                $repris = $false; $i = 0
                for ($i = 1; $i -le 80 -and -not $repris; $i++) {
                    $n = @(Positions $t).Count
                    if ((Position $t) -le 0) { break }
                    $NATIF::Molette($cx, $cy, 1, 120, 0)
                    $null = Attendre-Mouvement $t $n 1000
                    $limite = [DateTime]::UtcNow.AddMilliseconds(300)
                    do {
                        if (@(Apres $da 'Repris' $Cpt).Count -ge 1) { $repris = $true; break }
                        Start-Sleep -Milliseconds 20
                    } while ([DateTime]::UtcNow -lt $limite)
                }
                $pRepris = Attendre-Calme $t '(C8)' 200 5
                Verifier '(C8) crans vers le haut un par un : « repris »' $repris "après $($i - 1) cran(s), position $pRepris" | Out-Null
                $limite = [DateTime]::UtcNow.AddSeconds(3)
                while ((Montre (Element $Visee.Controle $TEXTE_TROP_VITE $null 0)) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
                Verifier '(C8) message caché après « repris »' (-not (Montre (Element $Visee.Controle $TEXTE_TROP_VITE $null 0))) | Out-Null
                # (C9) Entrée : en raccourci global, une fois réservée par l'app pour cette course.
                if (@(Apres $da 'EntreeR' $Cpt).Count -lt 1) { throw "(C9) Entrée non réservée par l'app pour cette course : la touche n'est pas envoyée ; $(Diagnostic-Journal $da)" }
                Permettre @($t.Pid, $sentinelle.Pid)
                Appuyer-Reservee $da 'Entree' '(C9)'
                $null = Attendre-Cle $da 'EntreeP' $Cpt '(C9) entree globale presse' 3
                $Issue = Attendre-Issue $da $Cpt '(C9)' 10 $Visee.Controle
                $perdues = Nombre $Issue.Message 'perdues (\d+)$'
                Verifier '(C9) Entrée : entree globale presse, fin entree, perdues ≥ 1' (($Issue.Message -match '^defilante fin entree ') -and ($perdues -ge 1)) $Issue.Message | Out-Null
                $Rendu = Attendre-Sortie $da $Cpt $avantPng '(C9)' $Visee.Controle
                $img = $Rendu.Png.Img
                $null = Verifier-Page '(C9) PNG exact du haut de la page à la dernière image gardée (aucun trou au « trop vite »)' $img 0
                Verifier-Reference '(C9) PNG' $img 0 'page-120.bmp'
                $haut = $img.H - $t.Page.H
                $connue = @(Positions $t | Where-Object { $_.P -eq $haut }).Count -ge 1
                Verifier '(C9) hauteur − 450 : une position du témoin, entre celle d''avant la rafale et le haut de la rafale' ($connue -and $haut -ge $pAvant -and $haut -lt $pHaut) "hauteur $($img.H), haut de la dernière image $haut ; avant la rafale $pAvant, haut de la rafale $pHaut, reprise $pRepris" | Out-Null
                Write-Host "[note] (C) moyens : $(Moyens $da $Cpt)"
                Verifier-Avertissements $da $Cpt '(C)'
            }

            # ---------- (S) suite d'images WPF pour le chantier C ------------------------------
            # Des données pour le chantier C, pas une preuve de l'app : notées seulement.
            try {
                Remonter $t '(S)'
                Permettre @($t.Pid)
                $images = Prendre-Suite $t.Page @($t.Page.CX, $t.Page.CY) 8 3 '(S) WPF'
                Ecrire-Suite 'wpf' $images -Decodable
            }
            catch { Write-Host "[note] (S) suite WPF : $_" }

            Point '(A5) sentinelle' {
                Verifier '(A5) la sentinelle n''a jamais défilé' (@(Positions $sentinelle).Count -eq 0) "$(@(Positions $sentinelle).Count) mouvement(s)" | Out-Null
            }
            Verifier-Interdites $da '(A)'
            Fermer-Cas $script:proc $da '(A)'
        }
        catch { Verifier '(A) cas sans erreur inattendue' $false "$_" | Out-Null }
        finally { Arreter-Tout; Arreter-Temoin $t; Arreter-Temoin $sentinelle }

        # ===================== Cas B : capturable ================================================
        $sentinelle = $null; $t = $null
        try {
            $db = Dossier-Cas 'b'
            Ecrire-Reglages -Dossier $db -Vignette $true
            Poser-Licence $db
            Ecrire-Reference 120
            Ecrire-Reference 400
            $sentinelle = Lancer-Temoin 'sentinelle-b' 760 80 @('-Largeur', '240', '-Hauteur', '300')
            $t = Lancer-Temoin 'immobile' 40 80 @('-Immobile')
            Permettre @($t.Pid, $sentinelle.Pid)
            $script:proc = Demarrer-Cas -Dossier $db -Quoi '(B)' -Capturable -AvecLicence
            $script:Cas = [pscustomobject]@{ Dossier = $db; Capturable = $true; AvecLicence = $true }
            $script:EcranFige = Hwnd-Journal 'ecran fige fenetre' $db
            $null = Attendre-Motif 'raccourci defilante: ' $db 10 '(B) raccourci defilante'

            # ---------- (B1) immobile, repli, Terminer ------------------------------------------
            Point '(B1) immobile' {
                $avantS = Copie-Ecran $sentinelle.Fenetre.X $sentinelle.Fenetre.Y $sentinelle.Fenetre.L $sentinelle.Fenetre.H
                $avantPng = Recents $db
                $Cpt = Compteurs $db
                $Visee = Viser $db 'raccourci' '(B1)' $Cpt
                $p = $t.Page
                # Le pointeur reste au centre de la page jusqu'au repli final : la molette de
                # l'app ne part que le pointeur sur la zone (sinon B rend « Saute » à chaque
                # pas, sans fin). Il ne gêne pas le témoin immobile, qui ne défile pas.
                $Choix = Choisir $db $Visee '(B1)' -Trace @($p.X, $p.Y, ($p.X + $p.L - 1), ($p.Y + $p.H - 1)) -PoserApres @($p.CX, $p.CY)
                Verifier '(B1) zone tracée exactement sur la page immobile' (($Choix.Zone.X -eq $p.X) -and ($Choix.Zone.Y -eq $p.Y) -and ($Choix.Zone.L -eq $p.L) -and ($Choix.Zone.H -eq $p.H) -and ($Choix.Sorte -eq 'trace') -and ($Choix.Agrandie -eq 'non')) $Choix.Ligne | Out-Null
                $null = Attendre-Cle $db 'Premiere' $Cpt '(B1) première image' 5
                $hc = $Visee.Controle
                $vis1 = $NATIF::IsWindowVisible($hc)
                $pendantS = Copie-Ecran $sentinelle.Fenetre.X $sentinelle.Fenetre.Y $sentinelle.Fenetre.L $sentinelle.Fenetre.H
                $part = $NATIF::PartDifferente($avantS.O, $pendantS.O, 6, $true)
                Verifier '(B1) capturable : le voile de la fenêtre de contrôle est vu sur la sentinelle (≥ 90 % des points non noirs changés ; contrôle de A5)' ($vis1 -and $part -ge 0.9) ("{0:P1} des points non noirs changés, fenêtre visible {1}" -f $part, $vis1) | Out-Null
                # `defilante repli: molette` seul : plus aucun moyen (la molette déclarée sans
                # effet à son tour) ; `defilante repli: molette (<ancien> …)` : la molette
                # retenue comme moyen. Un échec dit lequel des deux manque.
                $limite = [DateTime]::UtcNow.AddSeconds(20)
                $fini = $false
                do {
                    $replis = @(Apres $db 'Repli' $Cpt | ForEach-Object { Message $_ })
                    if (@($replis | Where-Object { $_ -eq 'defilante repli: molette' }).Count -ge 1) { $fini = $true; break }
                    if (@(Apres $db 'Issue' $Cpt).Count -gt 0) { break }
                    Start-Sleep -Milliseconds 50
                } while ([DateTime]::UtcNow -lt $limite)
                $moyens = Moyens $db $Cpt
                Write-Host "[note] (B1) moyens : $moyens"
                if (-not $fini) {
                    $retenue = @($replis | Where-Object { $_ -match '^defilante repli: molette \(' }).Count -ge 1
                    $fg = $NATIF::GetForegroundWindow()
                    $etat = "pointeur en $($p.CX),$($p.CY) sur $($NATIF::Decrire($NATIF::Sous($p.CX, $p.CY))), premier plan $($NATIF::Decrire($fg)), MouseWheelRouting $routage"
                    if ($retenue) { throw "(B1) la molette est retenue comme moyen mais n'est jamais déclarée sans effet en 20 s (la garde de B la retient ?) : $etat ; moyens $moyens ; $(Diagnostic-Journal $db)" }
                    throw "(B1) aucun repli vers la molette en 20 s (ni « repli: molette (… sans effet) », ni « repli: molette ») : $etat ; moyens $moyens ; $(Diagnostic-Journal $db)"
                }
                Verifier '(B1) les trois moyens sans effet : « defilante repli: molette » (pointeur au centre de la page)' $true $moyens | Out-Null
                if (@(Apres $db 'Issue' $Cpt).Count -gt 0) { throw "(B1) la course a fini avant « Terminer » : $(Message (@(Apres $db 'Issue' $Cpt))[0])" }
                # Le menu de l'icône pendant la course (témoin immobile : la course reste
                # ouverte) : « Quitter BreezyShot » seul ; refermé par un second clic sur
                # l'icône, sans rien choisir (aucune ligne qui commence par « menu: »).
                $motifChoix = '^\S+\s+\S+\s+\S+\s+menu: '
                $seuls = Compter-Motif 'menu pendant la defilante: quitter seulement$' $db
                $choisis = Compter-Motif $motifChoix $db
                $mq = Ouvrir-Menu '(B1) menu pendant la course' $db
                $null = Attendre-Suivante 'menu pendant la defilante: quitter seulement$' $db $seuls '(B1) menu pendant la defilante: quitter seulement' 3
                $seul = ($mq.Textes.Count -eq 1) -and ($mq.Textes[0] -ceq 'Quitter BreezyShot')
                Fermer-Menu-Par-Icone $mq $db '(B1) menu pendant la course'
                $vivante = -not $script:proc.HasExited
                Verifier '(B1) clic sur l''icône pendant la course : « Quitter BreezyShot » seul, refermé sans quitter (ni « menu: », ni issue)' ($seul -and $vivante -and ((Compter-Motif $motifChoix $db) -eq $choisis) -and (@(Apres $db 'Issue' $Cpt).Count -eq 0)) "menu $($mq.Lus) ; app $(if ($vivante) { 'en marche' } else { 'sortie' })" | Out-Null
                $el = Element $hc 'Terminer' $CT::Button 5
                if ($null -eq $el) { throw "(B1) bouton « Terminer » introuvable ; $(Elements-Vus $hc)" }
                $pt = Centre-Stable $el
                if (-not $NATIF::PoserSurApp($pt[0], $pt[1], 2000)) { throw "(B1) souris sur « Terminer » : la fenêtre de contrôle ne prend pas le clic en 2 s ($($NATIF::Decrire($NATIF::Sous($pt[0], $pt[1]))))" }
                $NATIF::BoutonBas(); Start-Sleep -Milliseconds 60; $NATIF::BoutonHaut()
                $NATIF::Poser($REPOS_X, $REPOS_Y)
                $Issue = Attendre-Issue $db $Cpt '(B1)' 10 $hc
                Verifier '(B1) clic réel sur « Terminer » : fin bouton' ($Issue.Message -match '^defilante fin bouton ') $Issue.Message | Out-Null
                $Rendu = Attendre-Sortie $db $Cpt $avantPng '(B1)' $hc
                # §16.3 : de « Terminer » (la fin reçue) à l'assemblée : `fin-assemblee`.
                $finAssemblee = Nombre $Rendu.Chrono 'fin-assemblee (\d+)'
                Verifier '(B1) « Terminer » → assemblée ≤ 150 ms (fin-assemblee)' (($finAssemblee -ge 0) -and ($finAssemblee -le 150)) "$($Rendu.Chrono) ; $($Rendu.Assemblee)" | Out-Null
                $null = Verifier-Page '(B1) PNG 600 × 450 égal à la page' $Rendu.Png.Img 0 450
                Verifier-Reference '(B1) PNG 600 × 450' $Rendu.Png.Img 0 'page-120.bmp'
                Verifier-Avertissements $db $Cpt '(B1)'
                Verifier '(B1) la page immobile n''a pas bougé' (@(Positions $t).Count -eq 0) | Out-Null
            }
            Arreter-Temoin $t

            # ---------- (B2) en-tête collant, (B3) petite zone ----------------------------------
            $t = Lancer-Temoin 'entete' 40 80 @('-EnTete')
            Permettre @($t.Pid, $sentinelle.Pid)
            Point '(B2) en-tête' {
                Remonter $t '(B2)'
                $avantPng = Recents $db
                $Cpt = Compteurs $db
                $Visee = Viser $db 'raccourci' '(B2)' $Cpt
                $f = $t.Fenetre
                $Choix = Choisir $db $Visee '(B2)' -Trace @($f.X, $f.Y, ($f.X + $f.L - 1), ($f.Y + $f.H - 1))
                Verifier '(B2) zone tracée sur toute la fenêtre (en-tête compris)' (($Choix.Zone.X -eq $f.X) -and ($Choix.Zone.Y -eq $f.Y) -and ($Choix.Zone.L -eq $f.L) -and ($Choix.Zone.H -eq $f.H)) "$($Choix.Ligne) ; fenêtre $(Texte-Rect $f)" | Out-Null
                $Issue = Attendre-Issue $db $Cpt '(B2)' 60 $Visee.Controle
                $fixes = @(Apres $db 'Fixes' $Cpt)
                Verifier '(B2) defilante fixes haut 40 bas 0' (($fixes.Count -ge 1) -and ((Message $fixes[-1]) -eq 'defilante fixes haut 40 bas 0')) ($(if ($fixes.Count) { Message $fixes[-1] } else { '<aucune ligne>' })) | Out-Null
                Verifier '(B2) fin en_bas' ($Issue.Message -match '^defilante fin en_bas ') $Issue.Message | Out-Null
                $Rendu = Attendre-Sortie $db $Cpt $avantPng '(B2)' $Visee.Controle
                $lus = Verifier-Page '(B2) PNG 600 × 6 040 : l''en-tête sur les rangées 0 à 39 seulement, puis la page' $Rendu.Png.Img 40 6040
                Noter-Mesure -Nom '(B2) rangées d''en-tête dans le PNG' -Valeur ($NATIF::Compter($lus, -1))
                Verifier-Reference '(B2) rangées 0 à 39 du PNG' $Rendu.Png.Img 0 'entete.bmp'
                Verifier-Reference '(B2) rangées 40 à 6 039 du PNG' $Rendu.Png.Img 40 'page-120.bmp'
                Verifier-Avertissements $db $Cpt '(B2)'
            }
            Point '(B3) petite zone' {
                Remonter $t '(B3)'
                $Cpt = Compteurs $db
                $Visee = Viser $db 'raccourci' '(B3)' $Cpt
                $x0 = $t.Page.X + 260; $y0 = $t.Page.Y + 195
                $Choix = Choisir $db $Visee '(B3)' -Trace @($x0, $y0, ($x0 + 79), ($y0 + 59))
                $ax = [math]::Floor((2 * $x0 + 80 - 120) / 2); $ay = [math]::Floor((2 * $y0 + 60 - 120) / 2)
                Verifier '(B3) zone de 80 × 60 tracée : 120 × 120 autour de son centre, agrandie oui' (($Choix.Zone.X -eq $ax) -and ($Choix.Zone.Y -eq $ay) -and ($Choix.Zone.L -eq 120) -and ($Choix.Zone.H -eq 120) -and ($Choix.Agrandie -eq 'oui')) "$($Choix.Ligne) ; attendu $ax,$ay 120x120" | Out-Null
                $null = Attendre-Cle $db 'Premiere' $Cpt '(B3) première image' 5
                Appuyer-Reservee $db 'Echap' '(B3)'
                $Issue = Attendre-Issue $db $Cpt '(B3)' 10 $Visee.Controle
                Verifier '(B3) Échap : defilante annulee echap' ($Issue.Message -eq 'defilante annulee echap') $Issue.Message | Out-Null
                $null = Attendre-Cle $db 'EchapL' $Cpt '(B3) echap global libere' 5
                Verifier-Avertissements $db $Cpt '(B3)'
            }
            Arreter-Temoin $t

            # ---------- (B4) limite, (B5) --quitter -------------------------------------------
            $t = Lancer-Temoin 'longue' 40 80 @('-Lignes', '400')
            Permettre @($t.Pid, $sentinelle.Pid)
            Point '(B4) limite' {
                Remonter $t '(B4)'
                $avantPng = Recents $db
                $Cpt = Compteurs $db
                $Visee = Viser $db 'raccourci' '(B4)' $Cpt
                Exiger-Pages $Visee '(B4)'
                $Choix = Choisir $db $Visee '(B4)' -Clic @($t.Page.CX, $t.Page.CY)
                $Issue = Attendre-Issue $db $Cpt '(B4)' 120 $Visee.Controle
                Verifier '(B4) page de 20 000 px : fin limite' ($Issue.Message -match '^defilante fin limite ') $Issue.Message | Out-Null
                $Rendu = Attendre-Sortie $db $Cpt $avantPng '(B4)' $Visee.Controle
                $null = Verifier-Page '(B4) PNG 600 × 16 384 (limite de l''éditeur), exact' $Rendu.Png.Img 0 16384
                Verifier-Reference '(B4) PNG 600 × 16 384' $Rendu.Png.Img 0 'page-400.bmp'
                Verifier-Avertissements $db $Cpt '(B4)'
            }
            Point '(B5) --quitter' {
                Remonter $t '(B5)'
                $Cpt = Compteurs $db
                $Visee = Viser $db 'raccourci' '(B5)' $Cpt
                Exiger-Pages $Visee '(B5)'
                $Choix = Choisir $db $Visee '(B5)' -Clic @($t.Page.CX, $t.Page.CY)
                $null = Attendre-Cle $db 'Premiere' $Cpt '(B5) première image' 5
                $arrets = Compter-Motif 'arret propre$' $db
                $q = Quitter-App -DelaiS 15 -Dossier $db
                if (-not $q.Fini) { throw '(B5) --quitter n''a pas répondu en 15 s' }
                if (-not $script:proc.WaitForExit(15000)) { throw '(B5) l''app ne s''est pas fermée en 15 s' }
                $Issue = Attendre-Issue $db $Cpt '(B5)' 3 ([IntPtr]::Zero)
                $null = Attendre-Suivante 'arret propre$' $db $arrets '(B5) arret propre' 3
                Verifier '(B5) --quitter pendant la course : defilante annulee arret, arret propre' ($Issue.Message -eq 'defilante annulee arret') $Issue.Message | Out-Null
                Verifier-Avertissements $db $Cpt '(B5)'
            }
            Verifier-Interdites $db '(B)'
        }
        catch { Verifier '(B) cas sans erreur inattendue' $false "$_" | Out-Null }
        finally { Arreter-Tout; Arreter-Temoin $t; Arreter-Temoin $sentinelle }

        # ===================== Cas N : Edge et l'Explorateur, notés seulement =====================
        # Rien ici ne fait échouer l'essai (R10) : tout est écrit en [note] et en mesures.
        try {
            $dn = Dossier-Cas 'n'
            Ecrire-Reglages -Dossier $dn -Vignette $false
            Poser-Licence $dn
            $script:proc = Demarrer-Cas -Dossier $dn -Quoi '(N)' -AvecLicence
            $script:Cas = [pscustomobject]@{ Dossier = $dn; Capturable = $false; AvecLicence = $true }
            $script:EcranFige = Hwnd-Journal 'ecran fige fenetre' $dn
            $null = Attendre-Motif 'raccourci defilante: ' $dn 10 '(N) raccourci defilante'

            # Une défilante notée (zone tracée $Trace, ou clic $Clic) : fin, moyens, image.
            function Noter-Defilante([string]$Quoi, [int[]]$Clic, [int[]]$Trace, [int]$Entete) {
                $avantPng = Recents $dn
                $Cpt = Compteurs $dn
                $Visee = Viser $dn 'raccourci' $Quoi $Cpt
                $Choix = $null
                if ($null -ne $Clic) {
                    $null = Attendre-Pages $Visee $Quoi
                    $Choix = Choisir $dn $Visee $Quoi -Clic $Clic -SansReprise
                }
                if ($null -eq $Choix) {
                    # Sans page sous le clic, la sélection continue : on trace.
                    $Choix = Choisir $dn $Visee $Quoi -Trace $Trace
                }
                try { $Issue = Attendre-Issue $dn $Cpt $Quoi 120 $Visee.Controle }
                catch {
                    # Retour au repos (Échap en raccourci global, s'il est tenu), puis l'échec.
                    try { Appuyer-Reservee $dn 'Echap' $Quoi } catch { }
                    throw
                }
                $premiere = @(Apres $dn 'Premiere' $Cpt)
                $produit = $null
                if ($Issue.Message -match '^defilante fin ') {
                    $produit = Attendre-Sortie $dn $Cpt $avantPng $Quoi
                    $img = $produit.Png.Img
                    $lus = $NATIF::Decoder($img.O, $img.L, $img.H, 0, 0, $img.H)
                    $ecart = $NATIF::Ecarts($lus, $Entete)
                    Write-Host "[note] $Quoi : image $($img.L)x$($img.H) ; relue sur la page : $(if ($ecart -eq '') { 'exacte' } else { $ecart })"
                }
                Write-Host "[note] $Quoi : $($Visee.Pages) page(s) proposée(s) ; $($Choix.Ligne) ; $(if ($premiere.Count) { Message $premiere[0] }) ; moyens $(Moyens $dn $Cpt) ; $($Issue.Message) ; $(Message (Derniere-Ligne 'defiler pages: ' $dn))"
                Noter-Mesure -Nom "$Quoi issue" -Valeur $Issue.Message
                return $Choix
            }

            # ---------- Edge : la même page en BMP dans une boîte qui défile, en-tête collant ----
            Point '(N) Edge' {
                try {
                    $edge = @("${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe", "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe") | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
                    if (-not $edge) { Write-Host '[note] (N) Edge absent de la machine : sauté'; return }
                    $site = Join-Path $script:Essai.Dossier 'edge'
                    $png = Start-Process -FilePath $script:Pwsh -PassThru -NoNewWindow -ArgumentList @('-NoProfile', '-File', "`"$(Join-Path $PSScriptRoot 'temoin-defilant.ps1')`"", '-Png', "`"$site`"", '-Lignes', '120')
                    if (-not $png.WaitForExit(60000)) { throw 'page BMP non écrite en 60 s' }
                    $html = @'
<!doctype html><html><head><meta charset="utf-8"><title>Temoin Edge BreezyShot</title><style>
html,body{margin:0;background:#fff;overflow:hidden}
#p{position:absolute;left:0;top:0;width:600px;height:490px;overflow-y:scroll;scrollbar-width:none}
#p::-webkit-scrollbar{display:none}
#t{position:sticky;top:0;display:block}
img{display:block;image-rendering:pixelated}
</style></head><body><div id="p"><img id="t" src="entete.bmp" width="600" height="40"><img src="page-120.bmp" width="600" height="6000"></div></body></html>
'@
                    [System.IO.File]::WriteAllText((Join-Path $site 'edge.html'), $html, [System.Text.UTF8Encoding]::new($false))
                    $script:ProfilEdge = Join-Path $script:Essai.Dossier 'edge-profil'
                    $adresse = ([System.Uri]::new((Join-Path $site 'edge.html'))).AbsoluteUri
                    $null = Start-Process -FilePath $edge -PassThru -ArgumentList @("--user-data-dir=`"$($script:ProfilEdge)`"", '--no-first-run', '--no-default-browser-check', '--disable-sync', '--force-device-scale-factor=1', '--window-position=40,80', '--window-size=700,640', "--app=$adresse")
                    $limite = [DateTime]::UtcNow.AddSeconds(40)
                    $h = [IntPtr]::Zero
                    do {
                        $v = @($NATIF::VisiblesTitre('Temoin Edge BreezyShot'))
                        if ($v.Count -ge 1) { $h = $v[0]; break }
                        Start-Sleep -Milliseconds 200
                    } while ([DateTime]::UtcNow -lt $limite)
                    if ($h -eq [IntPtr]::Zero) { throw 'fenêtre d''Edge introuvable en 40 s' }
                    Permettre @($NATIF::Pid($h))
                    # Le coin de l'en-tête (premier point magenta) dans la fenêtre.
                    $r = [BreezyShotEssai.Defilante+RECT]::new()
                    $null = $NATIF::GetWindowRect($h, [ref]$r)
                    $o = $null
                    $limite = [DateTime]::UtcNow.AddSeconds(15)
                    do {
                        $c = Copie-Ecran $r.Left $r.Top ($r.Right - $r.Left) ($r.Bottom - $r.Top)
                        $k = $NATIF::Premier($c.O, 240, 15, 240)
                        if ($k -ge 0) { $o = @(($r.Left + $k % $c.L), ($r.Top + [math]::Floor($k / $c.L))) }
                        else { Start-Sleep -Milliseconds 300 }
                    } while ($null -eq $o -and [DateTime]::UtcNow -lt $limite)
                    if ($null -eq $o) { throw 'page d''Edge non peinte (aucun point magenta) en 15 s' }
                    $z = [pscustomobject]@{ X = [int]$o[0]; Y = [int]$o[1]; L = 600; H = 490 }
                    Write-Host "[note] (N) Edge : boîte qui défile en $(Texte-Rect $z)"
                    $centre = @(($z.X + 300), ($z.Y + 245))
                    Ecrire-Suite 'edge' (Prendre-Suite $z $centre 8 3 '(N) Edge') -Decodable
                    $NATIF::Molette($centre[0], $centre[1], 80, 120, 5)
                    $null = Copie-Stable $z.X $z.Y $z.L $z.H '(N) Edge en haut'
                    $null = Noter-Defilante '(N) Edge' $null @($z.X, $z.Y, ($z.X + 599), ($z.Y + 489)) 40
                }
                catch { Write-Host "[note] (N) Edge : $_"; Revenir-Au-Repos '(N) Edge' }
                finally {
                    foreach ($p in @(Get-CimInstance Win32_Process -Filter "Name = 'msedge.exe'" -ErrorAction SilentlyContinue | Where-Object { $script:ProfilEdge -and "$($_.CommandLine)".Contains($script:ProfilEdge) })) {
                        try { Stop-Process -Id $p.ProcessId -Force -ErrorAction Stop } catch { }
                    }
                }
            }

            # ---------- L'Explorateur : 300 fichiers ----------------------------------------
            # La fenêtre est retrouvée par Shell.Application (son dossier, quel que soit le
            # titre : chemin complet, « - Explorateur de fichiers », anglais, onglets), puis
            # fermée dans un finally, même en cas d'échec.
            function Fenetres-Dossier($Application, [string]$Nom) {
                $vues = @()
                foreach ($w in @($Application.Windows())) {
                    $dossierVu = $null
                    try { $dossierVu = [string]$w.Document.Folder.Self.Path } catch { }
                    if ($dossierVu -and [string]::Equals((Split-Path -Leaf $dossierVu.TrimEnd('\')), $Nom, [StringComparison]::OrdinalIgnoreCase)) { $vues += $w }
                }
                return $vues
            }
            Point '(N) Explorateur' {
                $nom300 = "explorateur-300-$([guid]::NewGuid().ToString('N').Substring(0, 6))"
                $rep300 = Join-Path $script:Essai.Dossier $nom300
                $shell = $null; $hExplorateur = [IntPtr]::Zero
                try {
                    New-Item -ItemType Directory -Force -Path $rep300 | Out-Null
                    for ($i = 1; $i -le 300; $i++) { [System.IO.File]::WriteAllText((Join-Path $rep300 ('fichier-{0:D3}.txt' -f $i)), "$i") }
                    $shell = New-Object -ComObject Shell.Application
                    Start-Process -FilePath 'explorer.exe' -ArgumentList "`"$rep300`""
                    $limite = [DateTime]::UtcNow.AddSeconds(30)
                    do {
                        $w = @(Fenetres-Dossier $shell $nom300)
                        if ($w.Count -ge 1) { $hExplorateur = [IntPtr][long]$w[0].HWND; break }
                        Start-Sleep -Milliseconds 200
                    } while ([DateTime]::UtcNow -lt $limite)
                    if ($hExplorateur -eq [IntPtr]::Zero) { throw 'fenêtre de l''Explorateur introuvable en 30 s (Shell.Application)' }
                    Write-Host "[note] (N) Explorateur : fenêtre $($NATIF::Classe($hExplorateur)) « $($NATIF::Titre($hExplorateur)) »"
                    $null = $NATIF::SetWindowPos($hExplorateur, [IntPtr]::Zero, 40, 80, 720, 600, 0x14)
                    Permettre @($NATIF::Pid($hExplorateur))
                    $liste = $null
                    $limite = [DateTime]::UtcNow.AddSeconds(10)
                    do {
                        try {
                            $liste = $AE::FromHandle($hExplorateur).FindFirst([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, $CT::List))
                        } catch { $liste = $null }
                        if ($null -eq $liste -or $liste.Current.BoundingRectangle.Height -lt 200) { $liste = $null; Start-Sleep -Milliseconds 300 }
                    } while ($null -eq $liste -and [DateTime]::UtcNow -lt $limite)
                    if ($null -eq $liste) { throw 'liste des fichiers introuvable (UI Automation)' }
                    $borne = $liste.Current.BoundingRectangle
                    $z = [pscustomobject]@{ X = [int]$borne.X + 2; Y = [int]$borne.Y + 2; L = [int]$borne.Width - 24; H = [int]$borne.Height - 4 }
                    Write-Host "[note] (N) Explorateur : liste en $(Texte-Rect $z)"
                    $centre = @(($z.X + [math]::Floor($z.L / 2)), ($z.Y + [math]::Floor($z.H / 2)))
                    $null = Noter-Defilante '(N) Explorateur' $centre @($z.X, $z.Y, ($z.X + $z.L - 1), ($z.Y + $z.H - 1)) 0
                    $NATIF::Molette($centre[0], $centre[1], 120, 120, 5)
                    $null = Copie-Stable $z.X $z.Y $z.L $z.H '(N) Explorateur en haut'
                    Ecrire-Suite 'explorateur' (Prendre-Suite $z $centre 8 3 '(N) Explorateur')
                }
                catch { Write-Host "[note] (N) Explorateur : $_"; Revenir-Au-Repos '(N) Explorateur' }
                finally {
                    $fermees = 0
                    if ($null -ne $shell) {
                        foreach ($w in @(Fenetres-Dossier $shell $nom300)) { try { $w.Quit(); $fermees++ } catch { } }
                    }
                    if ($fermees -eq 0 -and $hExplorateur -ne [IntPtr]::Zero -and $NATIF::IsWindow($hExplorateur)) { $null = $NATIF::PostMessage($hExplorateur, $WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero) }
                    if ($null -ne $shell) { try { [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($shell) } catch { } }
                }
            }
            try { Fermer-Cas $script:proc $dn '(N)' } catch { Write-Host "[note] (N) fermeture : $_" }
        }
        catch { Write-Host "[note] (N) Edge et l'Explorateur : $_" }
        finally { Arreter-Tout }
        try { $NATIF::BoutonHaut() } catch { }
    }
}
catch {
    Verifier 'essai sans erreur inattendue' $false "$_" | Out-Null
}
finally {
    foreach ($a in $script:Auxiliaires) { try { if (-not $a.HasExited) { $a.Kill() } } catch { } }
    if ($script:ProfilEdge) {
        foreach ($p in @(Get-CimInstance Win32_Process -Filter "Name = 'msedge.exe'" -ErrorAction SilentlyContinue | Where-Object { "$($_.CommandLine)".Contains($script:ProfilEdge) })) {
            try { Stop-Process -Id $p.ProcessId -Force -ErrorAction Stop } catch { }
        }
    }
    Terminer-Essai
}
