# Version 0.5 « Historique et Copier le texte » (contrat 0.5, §12.3, avenant 2 « Licence »
# §3.5) : l'historique des captures, Copier le texte et la licence qui les débloque, sur le
# vrai .exe, avec de vraies entrées clavier et souris.
# BREEZYSHOT_ESSAI_CAPTURABLE=1. Presse-papiers lu dans un fil STA. Chaque attente lit une
# ligne nouvelle du journal (ou guette une fenêtre, un élément UI Automation) avec une
# borne ; jamais un délai fixe à la place d'une preuve.
# Garde (essai 12) : hors un raccourci global de l'app, une touche ne part que si la
# fenêtre au premier plan est à l'app, un appui de souris que si le point visé est sur une
# fenêtre de l'app ; sinon le point échoue en nommant la fenêtre trouvée, rien n'est envoyé.
# Les raccourcis Ctrl+Maj+H et Ctrl+Maj+T, tant que l'app ne les tient pas, ne sont jamais
# tapés : l'essai prouve qu'ils sont libres en les tenant lui-même (tenir-raccourci.ps1).
#
# Témoin (temoin-texte.ps1) : texte anglais ASCII, trois lignes noires en Segoe UI 20 px
# (« BreezyShot reads this text », « Version 0.5 of 12 November 2026 », « 0123456789 » :
# 3 lignes, 11 mots), une ligne de 12 px (lue, résultat noté seulement), une partie blanche.
# La machine GitHub a la langue de lecture en-US (sonde) : sans elle, l'essai échoue.
#
# Licence, sans licence d'abord (cas A) :
# (L1) `licence absente` ; menu de l'icône (API des menus) sans « Historique » ni « Copier
#      le texte » ; Ctrl+Maj+H et Ctrl+Maj+T libres (tenus par l'essai), aucune ligne
#      `raccourci historique` ni `raccourci texte`, ni `texte langues` (sans licence, la
#      sonde des langues ne tourne pas) ;
# (L2) Réglages ouverts par le vrai menu, rubrique Général : licence d'essai à signature
#      changée collée (Ctrl+V) dans le champ « Licence », clic réel sur « Activer » :
#      `licence refusee signature`, « Licence non valide », le champ garde le texte ;
# (L3) la licence de essais/licence-essai.txt collée, clic réel sur « Activer » : `licence
#      active ESSAI-0001`, `raccourci historique: Ctrl+Maj+H`, puis la sonde (`texte
#      langues`), puis `raccourci texte: Ctrl+Maj+T`, sans redémarrage, « Pro activé » et
#      « Retirer », licence.txt rangé, Ctrl+Maj+H tenu par l'app.
# Cas A, texte (copie, vignette éteinte, animations réduites) :
# (1) `texte langues` (une ligne, venue avec l'activation) contient en-US ; mémoire au
#     repos sans licence notée ;
# (2) menu (lu par position) : « Capture défilante<tab>Ctrl + Maj + 5 » après la dernière
#     zone (0.6, avec la licence), puis « Copier le texte<tab>Ctrl + Maj + T », séparateur,
#     « Historique<tab>Ctrl + Maj + H » ; Échap le ferme ;
# (3) presse-papiers témoin ; Ctrl+Maj+T : `DemandeCapture texte`, `selection montree …
#     mode texte` ; glisser réel autour des trois lignes : `zone texte` exacte, `Selection
#     -> Lecture`, écran figé encore visible 100 ms après le relâcher (guetté toutes les
#     2 ms), caché ensuite (≤ 1,5 s + marge) ; `texte lu 3 lignes 11 mots` ; presse-papiers
#     = les trois lignes séparées par CR LF, exactement ; `bandeau affiche … reussite` ;
#     temps notés ; (3b) la ligne de 12 px : lue, notée ;
# (4) zone blanche : `texte vide`, presse-papiers inchangé, `bandeau affiche … erreur`,
#     `bandeau termine` entre 3,8 et 5 s (heures du journal) sans clic ;
# (5) encore, clic réel sur « Réessayer » du bandeau : `bandeau action <id> reessayer`,
#     `DemandeCapture texte`, puis Échap : annulée, rien, presse-papiers inchangé ;
# (6) redémarrage (licence.txt en place : `licence active ESSAI-0001`) avec retardateur
#     3 s et pointeur allumés dans le fichier : Ctrl+Maj+T : ni `decompte`, ni `pointeur` ;
# (7) Ctrl+Maj+2 juste après le relâcher d'une lecture de l'écran entier : `appui ignore`,
#     aucune capture ; écran entier lu, temps noté ;
# (8) dix lectures : mémoire de l'app notée.
# Cas B, historique (licence.txt posé avant le démarrage, vignette « toujours ») :
# (9) aucune fenêtre « Historique » ; trois captures : trois PNG dans Recents et Apercus ;
# (10) Ctrl+Maj+H : `historique fenetre`, visible, titre, premier plan, barre des tâches,
#      sans barre de titre Windows (comme l'essai 12) ; liste « Captures récentes », trois
#      cases « Capture, à l'instant » ; `historique affiche` noté (budget 700 ms ;
#      échec au-delà de 1 500 ms sur la machine GitHub) ;
# (11) Ctrl+Maj+H encore, puis menu « Historique » : `ramene devant`, une seule fenêtre ;
#      capture fenêtre ouverte : quatre cases (la nouvelle en tête : prouvé en (13)) ;
# (12) survol réel puis clic sur « Copier » de la deuxième case : presse-papiers = PNG de
#      Recents au pixel près ; « Enregistrer » : un fichier au nom de l'heure de la capture,
#      égal ; encore : `deja fait`, aucun second fichier ;
# (13) case de tête (montrée par la vignette), Suppr : `vignette fermee <id> historique` ;
#      Suppr sur une autre : `en attente 2` ; Ctrl+Z : `annulee 2`, fichiers intacts ; Suppr
#      sur la case enregistrée : `suppression faite 1` 5 s après, PNG et aperçu partis,
#      fichier enregistré resté ;
# (14) Entrée sur une case : `editeur ouvert` à la taille de la case ; zone du témoin
#      capturée, Annoter, « Texte » : `editeur texte … lu 3 lignes 11 mots` ; Espace,
#      Maj+Droite × 2, Entrée : presse-papiers = « BreezyShot reads this » ; Bloc noir sur la
#      deuxième ligne, « Texte », « Tout copier » : les lignes 1 et 3 seulement (R7) ;
# (15) glisser réel d'une case vers cible-glisser.ps1 : un fichier, mêmes octets que Recents ;
# (16) « Plus d'options », « Tout effacer », « Effacer » : Recents, Apercus, Glisser vides de
#      leurs formats, « Aucune capture », « Plus d'options » grisé ;
# (17) bouton Fermer : `ferme (page)` ; WM_CLOSE : `ferme (cadre)` ; --quitter fenêtre
#      ouverte avec une suppression en attente : faite, `ferme (arret)`, `arret propre` ;
# (18) ouverte ≤ repos + 70 Mo ; 20 ouvertures et fermetures ; contrôles de l'essai 12 ;
# (L4) Réglages, clic réel sur « Retirer » : `licence retiree`, raccourcis rendus, entrées
#      parties du menu, licence.txt effacé ou vidé ; redémarrage : `licence absente`,
#      aucune sonde des langues.
# Cas C, migration (licence.txt en place au démarrage : `licence active ESSAI-0001`) :
#      reglages.json v3 où zone vaut Ctrl+Maj+T, trois PNG de la 0.4 sans aperçu : fichier
#      passé en v5, `raccourci texte: aucun` ; historique : trois cases, `historique apercus
#      faits 3`. (L'annonce « Aucun raccourci pour le texte » n'existe plus : rien attendu.)
# Cas D, sans langue (BREEZYSHOT_ESSAI_SANS_TEXTE=1, licence en place) : `texte langues
#      aucune (essai)` ; menu sans « Copier le texte » (avec « Historique ») ; aucune ligne
#      `raccourci texte` ; Ctrl+Maj+T libre ; Réglages › Raccourcis sans la ligne ; éditeur
#      sans « Texte ».
#
# Non vérifiable sur GitHub (un écran 1024 × 768 à 100 %) : les langues de l'utilisateur, le
# français et les accents, le japonais et le chinois, 125 à 200 %, plusieurs écrans, le
# balayage vu, l'allure des pilules. Hors Windows : démarrage seulement.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '13-historique-texte' -Exe $Exe -Sortie $Sortie | Out-Null

$script:Auxiliaires = [System.Collections.Generic.List[object]]::new()
$script:Pwsh = (Get-Process -Id $PID).Path
# La licence d'essai, telle que rangée par le chantier L (une ligne, blancs retirés).
$LICENCE = (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'licence-essai.txt') -Raw).Trim()
# Licence refusée à signature fausse : un caractère de la signature (pas de la charge) changé.
$parties = $LICENCE.Split('.')
$c5 = $parties[2][5]
$autre = if ($c5 -ceq 'A') { 'B' } else { 'A' }
$LICENCE_FAUSSE = "$($parties[0]).$($parties[1]).$($parties[2].Substring(0, 5))$autre$($parties[2].Substring(6))"
$FORMAT_RECENT = '^\d{8}-\d{6}-\d{3}(_\d+)?\.png$'
$LIGNES_TEMOIN = @('BreezyShot reads this text', 'Version 0.5 of 12 November 2026', '0123456789')

# --- Aides (même forme que les essais 07, 08, 10 et 12) ------------------------------------

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

# Attend une ligne de plus que maintenant (le compte est pris avant le geste).
function Attendre-Suivante([string]$Motif, [string]$Dossier, [int]$Avant, [string]$Quoi, [double]$DelaiS = 10) {
    return Attendre-Motif -Motif $Motif -Dossier $Dossier -DelaiS $DelaiS -Quoi $Quoi -Fois ($Avant + 1)
}

# Les 40 dernières lignes utiles, sans horodatage (ni chemin ni contenu : contrat §8).
function Diagnostic-Journal([string]$Dossier) {
    $l = @(Lignes-Journal $Dossier | Where-Object { $_ -match 'licence|texte|historique|editeur|vignette|selection|bandeau|raccourci|menu|etat |WARN|ERROR' } | Select-Object -Last 40 | ForEach-Object { $_ -replace '^\S+\s+', '' })
    if ($l.Count -eq 0) { return '(journal : aucune ligne utile)' }
    return 'journal : ' + ($l -join ' | ')
}

# L'heure d'une ligne du journal (premier mot, `2026-11-11T18:40:12.345+01:00`).
function Heure-Ligne([string]$Ligne) {
    $mot = ($Ligne -split '\s+')[0]
    return [DateTimeOffset]::Parse($mot, [Globalization.CultureInfo]::InvariantCulture)
}

function Fichiers-Png([string]$Dossier) {
    if (-not (Test-Path -LiteralPath $Dossier)) { return @() }
    return @(Get-ChildItem -LiteralPath $Dossier -File -Filter '*.png' | Sort-Object LastWriteTimeUtc, Name)
}

# Les fichiers au format de « Récents » (ou de leurs aperçus), le plus récent d'abord : l'ordre
# des cases de l'historique (le nom porte l'heure UTC).
function Fichiers-Format([string]$Dossier) {
    if (-not (Test-Path -LiteralPath $Dossier)) { return @() }
    return @(Get-ChildItem -LiteralPath $Dossier -File | Where-Object { $_.Name -match $FORMAT_RECENT } | Sort-Object Name -Descending)
}

# Le nom qu'Enregistrer donne à une capture de « Récents » (modèle des réglages, heure
# locale de la capture).
function Nom-Enregistre([string]$NomRecent) {
    $utc = [DateTime]::ParseExact($NomRecent.Substring(0, 19), 'yyyyMMdd-HHmmss-fff', [Globalization.CultureInfo]::InvariantCulture,
        [Globalization.DateTimeStyles]::AssumeUniversal -bor [Globalization.DateTimeStyles]::AdjustToUniversal)
    $l = $utc.ToLocalTime()
    return "BreezyShot $($l.ToString('yyyy-MM-dd', [Globalization.CultureInfo]::InvariantCulture)) à $($l.ToString('HH.mm.ss', [Globalization.CultureInfo]::InvariantCulture)).png"
}

function Ecrire-Reglages([string]$Dossier, [string]$Images, [bool]$Vignette = $false, [int]$Retardateur = 0, [bool]$Pointeur = $false, $Raccourcis = $null) {
    if ($null -eq $Raccourcis) {
        $Raccourcis = [ordered]@{ zone = @('Ctrl+Maj+1'); plein_ecran = @('Ctrl+Maj+2'); fenetre = @(); derniere_zone = @() }
    }
    $r = [ordered]@{
        version                   = 3
        raccourcis                = $Raccourcis
        copier_apres_capture      = $true
        enregistrer_apres_capture = $false
        dossier_enregistrement    = $Images
        vignette                  = [ordered]@{ afficher = $Vignette; duree_s = 0; coin = 'bas_gauche' }
        animations                = 'reduites'
        selection                 = [ordered]@{ loupe = $false }
        capture                   = [ordered]@{ retardateur_s = $Retardateur; pointeur = $Pointeur }
        demarrer_avec_systeme     = $false
        modele_nom                = 'BreezyShot {date} à {heure}'
        theme                     = 'systeme'
    }
    $d = Join-Path $Dossier 'reglages'
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $d 'reglages.json'), ($r | ConvertTo-Json -Depth 5), [System.Text.UTF8Encoding]::new($false))
}

function Lire-Reglages([string]$Dossier) {
    return (Get-Content -LiteralPath (Join-Path $Dossier 'reglages/reglages.json') -Raw -Encoding utf8 | ConvertFrom-Json)
}

function Chemin-Licence([string]$Dossier) { return (Join-Path $Dossier 'reglages/licence.txt') }

# Pose licence.txt avant le démarrage, comme le range le chantier L (une ligne nettoyée,
# UTF-8 sans BOM : un BOM donnerait `licence refusee forme`).
function Poser-Licence([string]$Dossier) {
    New-Item -ItemType Directory -Force -Path (Join-Path $Dossier 'reglages') | Out-Null
    [System.IO.File]::WriteAllText((Chemin-Licence $Dossier), $LICENCE, [System.Text.UTF8Encoding]::new($false))
}

function Demarrer-Cas([string]$Dossier, [string]$Quoi, [switch]$Vignette, [switch]$SansTexte) {
    $env:BREEZYSHOT_ESSAI_CAPTURABLE = '1'
    if ($SansTexte) { $env:BREEZYSHOT_ESSAI_SANS_TEXTE = '1' }
    $prets = Compter-Motif -Motif 'application +pret$' -Dossier $Dossier
    $bandeaux = Compter-Motif -Motif 'bandeau pret$' -Dossier $Dossier
    $vignettes = Compter-Motif -Motif 'vignette prete$' -Dossier $Dossier
    $langues = Compter-Motif -Motif 'texte (langues|non pris en charge)' -Dossier $Dossier
    $script:dossierCas = $Dossier
    try { $proc = Demarrer-App -Dossier $Dossier }
    finally {
        Remove-Item Env:BREEZYSHOT_ESSAI_CAPTURABLE -ErrorAction SilentlyContinue
        Remove-Item Env:BREEZYSHOT_ESSAI_SANS_TEXTE -ErrorAction SilentlyContinue
    }
    if ($IsWindows) { [BreezyShotEssai.Historique]::PidApp = $proc.Id }
    $null = Attendre-Motif -Motif 'application +pret$' -Dossier $Dossier -DelaiS 30 -Quoi "$Quoi : démarrage" -Fois ($prets + 1)
    $null = Attendre-Motif -Motif 'bandeau pret$' -Dossier $Dossier -DelaiS 30 -Quoi "$Quoi : page du bandeau" -Fois ($bandeaux + 1)
    if ($Vignette) { $null = Attendre-Motif -Motif 'vignette prete$' -Dossier $Dossier -DelaiS 30 -Quoi "$Quoi : page de la vignette" -Fois ($vignettes + 1) }
    # La sonde des langues ne tourne qu'avec une licence en place (une fois par lancement) :
    # sans licence.txt, ou vide, aucune ligne `texte langues` n'est attendue.
    $cl = Chemin-Licence $Dossier
    $avecLicence = (Test-Path -LiteralPath $cl) -and ([System.IO.File]::ReadAllText($cl).Trim() -ne '')
    if ($avecLicence) {
        $null = Attendre-Motif -Motif 'texte (langues|non pris en charge)' -Dossier $Dossier -DelaiS 30 -Quoi "$Quoi : langues de lecture" -Fois ($langues + 1)
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

# La mémoire privée par sorte de processus (essai 12) : { Texte, App, Navigateur, Gpu, Rendus }.
function Detail-Memoire($Processus) {
    $enfants = @{}
    foreach ($c in @(Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, CommandLine -ErrorAction SilentlyContinue)) {
        $parent = [int]$c.ParentProcessId
        if (-not $enfants.ContainsKey($parent)) { $enfants[$parent] = [System.Collections.Generic.List[object]]::new() }
        $enfants[$parent].Add($c)
    }
    $sommes = [ordered]@{}
    $nombres = @{}
    $vus = @{ [int]$Processus.Id = $true }
    $file = [System.Collections.Generic.Queue[object]]::new()
    $file.Enqueue([pscustomobject]@{ Id = [int]$Processus.Id; Sorte = 'app' })
    while ($file.Count -gt 0) {
        $n = $file.Dequeue()
        try { $mo = (Get-Process -Id $n.Id -ErrorAction Stop).PrivateMemorySize64 / 1MB } catch { $mo = 0 }
        if (-not $sommes.Contains($n.Sorte)) { $sommes[$n.Sorte] = 0.0; $nombres[$n.Sorte] = 0 }
        $sommes[$n.Sorte] += $mo
        $nombres[$n.Sorte] += 1
        if (-not $enfants.ContainsKey($n.Id)) { continue }
        foreach ($el in $enfants[$n.Id]) {
            $id = [int]$el.ProcessId
            if ($vus.ContainsKey($id)) { continue }
            $vus[$id] = $true
            $sorte = if ("$($el.CommandLine)" -match '--type=([\w-]+)') { $Matches[1] } else { 'navigateur' }
            $file.Enqueue([pscustomobject]@{ Id = $id; Sorte = $sorte })
        }
    }
    $texte = (@($sommes.Keys) | ForEach-Object { "$_ x$($nombres[$_]) $([math]::Round($sommes[$_], 1))" }) -join ' ; '
    return [pscustomobject]@{
        Texte      = $texte
        App        = [math]::Round([double]$sommes['app'], 1)
        Navigateur = [math]::Round([double]$sommes['navigateur'], 1)
        Gpu        = [math]::Round([double]$sommes['gpu-process'], 1)
        Rendus     = $(if ($nombres.ContainsKey('renderer')) { [int]$nombres['renderer'] } else { 0 })
    }
}

# Mémoire privée du seul processus de l'app (Mo).
function Memoire-App($Processus) {
    $Processus.Refresh()
    return [math]::Round($Processus.PrivateMemorySize64 / 1MB, 1)
}

# Un point de l'essai dans son propre try : un échec est noté et l'essai continue.
function Point([string]$Nom, [scriptblock]$Corps) {
    try { & $Corps }
    catch { Verifier $Nom $false "$_" | Out-Null }
}

try {
    if (-not $IsWindows) {
        $d = Dossier-Cas 'commun'
        Ecrire-Reglages -Dossier $d -Images (Join-Path $d 'images')
        $proc = Demarrer-App -Dossier $d
        $null = Attendre-Motif -Motif 'application +pret$' -Dossier $d -DelaiS 30 -Quoi 'démarrage'
        $null = Attendre-Motif -Motif 'licence absente$' -Dossier $d -DelaiS 5 -Quoi 'licence au démarrage'
        Verifier '(L1) licence absente au démarrage' $true | Out-Null
        Fermer-Cas $proc $d 'commun'
        Verifier '--quitter → arret propre' $true | Out-Null
        Write-Host '[saute] le reste : sauté hors Windows'
    }
    else {
        Add-Type -AssemblyName UIAutomationClient
        Add-Type -AssemblyName UIAutomationTypes
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms
        Add-Type -AssemblyName System.IO.Compression
        Add-Type -Namespace BreezyShotEssai -Name Historique -MemberDefinition @'
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
[DllImport("user32.dll")]
public static extern bool GetClientRect(IntPtr h, out RECT r);
[DllImport("user32.dll")]
public static extern uint GetDpiForWindow(IntPtr h);
[StructLayout(LayoutKind.Sequential)]
public struct POINT { public int X; public int Y; }
[DllImport("user32.dll")]
public static extern bool ClientToScreen(IntPtr h, ref POINT p);
[DllImport("user32.dll")]
public static extern IntPtr GetWindow(IntPtr h, uint quoi);
[DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
public static extern IntPtr GetWindowLongPtr(IntPtr h, int index);
[DllImport("user32.dll")]
public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
[DllImport("user32.dll")]
public static extern IntPtr SendMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
[StructLayout(LayoutKind.Sequential)]
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
[DllImport("user32.dll")]
public static extern bool GetWindowRect(IntPtr h, out RECT r);
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
[DllImport("user32.dll")]
public static extern uint GetClipboardSequenceNumber();
[DllImport("user32.dll")]
public static extern int GetMenuItemCount(IntPtr menu);
[DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "GetMenuStringW")]
static extern int GetMenuString(IntPtr menu, uint item, System.Text.StringBuilder texte, int taille, uint drapeaux);
[DllImport("user32.dll")]
public static extern bool GetMenuItemRect(IntPtr h, IntPtr menu, uint item, out RECT r);

// Garde : aucune touche ni aucun appui de souris ne part vers une fenêtre qui n'est pas à
// l'app. Un Ctrl+C ou un Alt+F4 reçu par une console de la machine d'essai l'arrêterait.
public static int PidApp;
static int PidDe(IntPtr h) { uint q; GetWindowThreadProcessId(h, out q); return (int)q; }
public static string Decrire(IntPtr h) {
    if (h == IntPtr.Zero) return "aucune fenêtre";
    string nom = "?";
    try { nom = System.Diagnostics.Process.GetProcessById(PidDe(h)).ProcessName; } catch { }
    string titre = Titre(h);
    if (titre.Length > 60) titre = titre.Substring(0, 60);
    return Classe(h) + " « " + titre + " », processus " + nom;
}
static void Garder(IntPtr h, string quoi) {
    if (PidApp == 0 || h == IntPtr.Zero || PidDe(h) != PidApp)
        throw new InvalidOperationException(quoi + " (" + Decrire(h) + ")");
}

const uint RELACHEE = 0x2;
const uint ETENDUE = 0x1;
// Flèches, Début, Fin, Inser, Suppr : touches étendues (sinon le pavé numérique).
static uint Drapeaux(byte vk) { return (vk >= 0x21 && vk <= 0x28) || vk == 0x2D || vk == 0x2E ? ETENDUE : 0u; }
static void Bas(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), Drapeaux(vk), UIntPtr.Zero); }
static void Haut(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), Drapeaux(vk) | RELACHEE, UIntPtr.Zero); }
public static void Appuyer(byte[] mods, byte touche) {
    Garder(GetForegroundWindow(), "touche 0x" + touche.ToString("X2") + " non envoyée : le premier plan n'est pas à l'app");
    AppuyerGlobal(mods, touche);
}
// Un raccourci global de l'app (tenu par elle) : il ne vise aucune fenêtre, il part sans garde.
public static void AppuyerGlobal(byte[] mods, byte touche) {
    foreach (var m in mods) Bas(m);
    Bas(touche);
    System.Threading.Thread.Sleep(30);
    Haut(touche);
    for (int i = mods.Length - 1; i >= 0; i--) Haut(mods[i]);
    System.Threading.Thread.Sleep(30);
}
public static void Touche(byte vk) { Appuyer(new byte[0], vk); }
public static void Poser(int x, int y) { SetCursorPos(x, y); mouse_event(0x1, 0, 0, 0, UIntPtr.Zero); }
public static void BoutonBas() {
    POINT p; GetCursorPos(out p);
    Garder(GetAncestor(WindowFromPoint(p), 2), "appui de souris en " + p.X + "," + p.Y + " non envoyé : ce point n'est pas sur une fenêtre de l'app");
    mouse_event(0x2, 0, 0, 0, UIntPtr.Zero);
}
public static void BoutonHaut() { mouse_event(0x4, 0, 0, 0, UIntPtr.Zero); }
// Pose la souris, puis attend que ce point soit sur une fenêtre de l'app. Le bandeau laisse
// passer les clics tant que la souris n'est pas vue sur sa pilule (surveillance toutes les
// 30 à 60 ms, puis un tour du fil principal) : un clic 40 ms après l'arrivée peut le précéder.
public static bool PoserSurApp(int x, int y, int ms) {
    Poser(x, y);
    POINT p; p.X = x; p.Y = y;
    DateTime fin = DateTime.UtcNow.AddMilliseconds(ms);
    do {
        IntPtr h = GetAncestor(WindowFromPoint(p), 2);
        if (PidApp != 0 && h != IntPtr.Zero && PidDe(h) == PidApp) return true;
        System.Threading.Thread.Sleep(20);
    } while (DateTime.UtcNow < fin);
    return false;
}
public static void Cliquer(int x, int y) {
    Poser(x, y); System.Threading.Thread.Sleep(40);
    BoutonBas(); System.Threading.Thread.Sleep(60); BoutonHaut();
}
public static void Deplacer(int x0, int y0, int x1, int y1, int pas, int ms) {
    for (int i = 1; i <= pas; i++) {
        Poser(x0 + (x1 - x0) * i / pas, y0 + (y1 - y0) * i / pas);
        System.Threading.Thread.Sleep(ms);
    }
}
// Un glisser réel jusqu'au relâcher : appui en (x0, y0), 20 pas, relâché en (x1, y1).
static void TracerSansFin(int x0, int y0, int x1, int y1) {
    Poser(x0, y0); System.Threading.Thread.Sleep(80);
    BoutonBas(); System.Threading.Thread.Sleep(80);
    Deplacer(x0, y0, x1, y1, 20, 15);
    System.Threading.Thread.Sleep(80);
    BoutonHaut();
}
public static void Tracer(int x0, int y0, int x1, int y1) {
    TracerSansFin(x0, y0, x1, y1); System.Threading.Thread.Sleep(80);
}
// Glisser réel, puis la fenêtre `h` guettée toutes les 2 ms depuis le relâcher : rend le
// délai (ms) au bout duquel elle est cachée ; -1 si elle ne l'était déjà plus au relâcher,
// maxMs + 1 si elle l'est encore à maxMs.
public static int TracerPuisGuetter(int x0, int y0, int x1, int y1, IntPtr h, int maxMs) {
    TracerSansFin(x0, y0, x1, y1);
    var sw = System.Diagnostics.Stopwatch.StartNew();
    if (!IsWindowVisible(h)) return -1;
    long prochain = 2;
    while (sw.ElapsedMilliseconds <= maxMs) {
        while (sw.ElapsedMilliseconds < prochain) System.Threading.Thread.Yield();
        if (!IsWindowVisible(h)) return (int)sw.ElapsedMilliseconds;
        prochain += 2;
    }
    return maxMs + 1;
}
// Glisser réel, puis aussitôt un raccourci global (pendant la lecture qui suit).
public static void TracerPuisAppuyer(int x0, int y0, int x1, int y1, byte[] mods, byte touche) {
    TracerSansFin(x0, y0, x1, y1);
    AppuyerGlobal(mods, touche);
}
public static string Titre(IntPtr h) {
    var t = new System.Text.StringBuilder(512); GetWindowText(h, t, t.Capacity); return t.ToString();
}
public static string Classe(IntPtr h) {
    var t = new System.Text.StringBuilder(256); GetClassName(h, t, t.Capacity); return t.ToString();
}
// Fenêtres de premier niveau du processus `pid` dont le titre est `titre`.
public static IntPtr[] FenetresTitre(int pid, string titre) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, p) => {
        uint q; GetWindowThreadProcessId(h, out q);
        if ((int)q == pid && Titre(h) == titre) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
// Fenêtres de premier niveau d'un processus, d'une classe donnée.
public static IntPtr[] Fenetres(int pid, string classe) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, p) => {
        uint q; GetWindowThreadProcessId(h, out q);
        if ((int)q == pid && Classe(h) == classe) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
// Fenêtres visibles de cette classe, tous processus confondus.
public static IntPtr[] Visibles(string classe) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, p) => {
        if (IsWindowVisible(h) && Classe(h) == classe) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
// Menus Win32 (essai 08) : le HMENU d'une fenêtre de menu (#32768), le texte de chaque
// position (tabulation et raccourci compris ; "" pour un séparateur).
public static IntPtr MenuDe(IntPtr fenetre) { return SendMessage(fenetre, 0x01E1, IntPtr.Zero, IntPtr.Zero); }
public static string[] Textes(IntPtr menu) {
    int n = GetMenuItemCount(menu);
    if (n < 0) return null;
    var textes = new string[n];
    for (int i = 0; i < n; i++) {
        var t = new System.Text.StringBuilder(256);
        GetMenuString(menu, (uint)i, t, t.Capacity, 0x400);
        textes[i] = t.ToString();
    }
    return textes;
}
// Pixels BGRA (largeur l) : nombre de pixels différents (toute l'image).
public static int Differences(byte[] a, byte[] b, int l, int h) {
    if (a.Length != b.Length || a.Length != l * h * 4) return -1;
    int n = 0;
    for (int i = 0; i < a.Length; i += 4)
        if (a[i] != b[i] || a[i + 1] != b[i + 1] || a[i + 2] != b[i + 2] || a[i + 3] != b[i + 3]) n++;
    return n;
}
// CRC des morceaux d'un PNG.
public static uint Crc32(byte[] o) {
    uint c = 0xFFFFFFFF;
    foreach (byte b in o) {
        c ^= b;
        for (int k = 0; k < 8; k++) c = (c & 1) != 0 ? 0xEDB88320 ^ (c >> 1) : c >> 1;
    }
    return c ^ 0xFFFFFFFF;
}
'@
        try { [void][BreezyShotEssai.Historique]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }
        $E = [BreezyShotEssai.Historique]
        $AE = [System.Windows.Automation.AutomationElement]
        $CT = [System.Windows.Automation.ControlType]

        $MODS = [byte[]]@(0x11, 0x10)          # Ctrl, Maj
        $CTRL = [byte[]]@(0x11)
        $MAJ = [byte[]]@(0x10)
        $AUCUN = [byte[]]@()
        $VK_1 = [byte]0x31; $VK_2 = [byte]0x32; $VK_H = [byte]0x48; $VK_T = [byte]0x54
        $VK_A = [byte]0x41; $VK_V = [byte]0x56; $VK_Z = [byte]0x5A
        $ECHAP = [byte]0x1B; $ENTREE = [byte]0x0D; $ESPACE = [byte]0x20; $DROITE = [byte]0x27; $SUPPR = [byte]0x2E
        $MOD_CTRL_MAJ = 6                       # RegisterHotKey : MOD_CONTROL | MOD_SHIFT
        $WM_CLOSE = [uint32]0x10
        $TITRE_HISTORIQUE = 'Historique'
        $TITRE_EDITEUR = 'Annoter la capture'
        $TITRE_REGLAGES = 'Réglages de BreezyShot'
        $MODELE = '^BreezyShot \d{4}-\d\d-\d\d à \d\d\.\d\d\.\d\d( \(\d+\))?\.png$'
        $utile = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        $ecran = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        # Souris au repos : sur le bureau, à droite, loin du bandeau (en haut au centre), de la
        # vignette (en bas à gauche), de l'historique (centré) et des témoins.
        $REPOS_X = [int]($utile.Right - 12)
        $REPOS_Y = [int]($utile.Top + $utile.Height / 2)
        Write-Host "écran $ecran ; zone utile $utile"

        # « … <x>,<y> <l>x<h> » dans une ligne → { X, Y, L, H, CX, CY }.
        function Lire-Rect([string]$Ligne) {
            if ($Ligne -notmatch '(-?\d+),(-?\d+) (\d+)x(\d+)') { throw "rectangle illisible : $Ligne" }
            $r = [pscustomobject]@{ X = [int]$Matches[1]; Y = [int]$Matches[2]; L = [int]$Matches[3]; H = [int]$Matches[4] }
            $r | Add-Member CX ([int]($r.X + $r.L / 2))
            $r | Add-Member CY ([int]($r.Y + $r.H / 2))
            return $r
        }

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
        function Octets-Png([byte[]]$Png) {
            $flux = [System.IO.MemoryStream]::new($Png)
            try {
                $img = [System.Drawing.Bitmap]::new($flux)
                try { return Octets-Image $img } finally { $img.Dispose() }
            }
            finally { $flux.Dispose() }
        }
        function Octets-Fichier([string]$Chemin) { return (Octets-Png ([System.IO.File]::ReadAllBytes($Chemin))) }

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

        # Le texte du presse-papiers, dans un fil STA, exactement (fins de ligne comprises) ;
        # $null s'il n'y a pas de texte.
        function Lire-Texte {
            $rs = [runspacefactory]::CreateRunspace()
            $rs.ApartmentState = [System.Threading.ApartmentState]::STA
            $rs.Open()
            try {
                $ps = [powershell]::Create()
                $ps.Runspace = $rs
                [void]$ps.AddScript({
                        Add-Type -AssemblyName System.Windows.Forms
                        for ($i = 0; $i -lt 10; $i++) {
                            try {
                                if ([System.Windows.Forms.Clipboard]::ContainsText()) { return [System.Windows.Forms.Clipboard]::GetText() }
                                return '<<aucun texte>>'
                            } catch { Start-Sleep -Milliseconds 100 }
                        }
                        return '<<presse-papiers illisible>>'
                    })
                try { $r = @($ps.Invoke()) } finally { $ps.Dispose() }
                if ($r.Count -lt 1) { return '<<presse-papiers illisible>>' }
                return [string]$r[0]
            }
            finally { $rs.Dispose() }
        }
        function Poser-Texte([string]$Texte) {
            $rs = [runspacefactory]::CreateRunspace()
            $rs.ApartmentState = [System.Threading.ApartmentState]::STA
            $rs.Open()
            try {
                $ps = [powershell]::Create()
                $ps.Runspace = $rs
                [void]$ps.AddScript({
                        param($t)
                        Add-Type -AssemblyName System.Windows.Forms
                        for ($i = 0; $i -lt 10; $i++) {
                            try { [System.Windows.Forms.Clipboard]::SetText($t); return 'ok' } catch { Start-Sleep -Milliseconds 100 }
                        }
                        return 'echec'
                    }).AddArgument($Texte)
                try { $r = @($ps.Invoke()) } finally { $ps.Dispose() }
                if ($r.Count -lt 1 -or $r[0] -ne 'ok') { throw 'presse-papiers : texte non posé' }
            }
            finally { $rs.Dispose() }
            if ((Lire-Texte) -cne $Texte) { throw 'presse-papiers : texte posé non relu' }
        }
        # Texte lisible pour un message (fins de ligne montrées).
        function Montrer([string]$T) { return ($T -replace "`r", '\r' -replace "`n", '\n') }

        function Lancer-Aide([string]$Script, [string[]]$Arguments, [string]$Fichier) {
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -ArgumentList (@('-NoProfile', '-File', (Join-Path $PSScriptRoot $Script)) + $Arguments)
            $script:Auxiliaires.Add($aide)
            $limite = [DateTime]::UtcNow.AddSeconds(30)
            while (-not (Test-Path -LiteralPath $Fichier)) {
                if ([DateTime]::UtcNow -ge $limite) { throw "$Script : rien écrit en 30 s" }
                Start-Sleep -Milliseconds 100
            }
            Start-Sleep -Milliseconds 400
            return $aide
        }
        function Arreter-Aide($Aide) {
            if ($null -eq $Aide) { return }
            try { if (-not $Aide.HasExited) { $Aide.Kill(); $null = $Aide.WaitForExit(5000) } } catch { }
        }

        # Le témoin de texte : rend { Aide, Lignes = @(r1, r2, r3), Petite, Blanc } (pixels réels).
        function Lancer-Temoin([string]$Nom, [int]$X, [int]$Y) {
            $f = Join-Path $script:Essai.Dossier "temoin-$Nom.txt"
            Remove-Item -LiteralPath $f -Force -ErrorAction SilentlyContinue
            $aide = Lancer-Aide 'temoin-texte.ps1' @('-X', "$X", '-Y', "$Y", '-Fichier', $f) $f
            $lu = @{}
            foreach ($l in (Get-Content -LiteralPath $f)) { if ($l -match '^(\w+) (.+)$') { $lu[$Matches[1]] = $Matches[2] } }
            foreach ($k in 'ligne1', 'ligne2', 'ligne3', 'petite', 'blanc') { if (-not $lu.ContainsKey($k)) { throw "témoin : $k absent de $f" } }
            $t = [pscustomobject]@{
                Aide   = $aide
                Lignes = @((Lire-Rect $lu['ligne1']), (Lire-Rect $lu['ligne2']), (Lire-Rect $lu['ligne3']))
                Petite = Lire-Rect $lu['petite']
                Blanc  = Lire-Rect $lu['blanc']
            }
            Write-Host "témoin : $(($t.Lignes | ForEach-Object { "$($_.X),$($_.Y) $($_.L)x$($_.H)" }) -join ' ; ') ; petite $($lu['petite']) ; blanc $($lu['blanc'])"
            return $t
        }
        # La zone autour des trois lignes de 20 px, marge $M : { X0, Y0, X1, Y1 } (bornes
        # comprises : la zone choisie fait X1 - X0 + 1 de large).
        function Zone-Lignes($T, [int]$M = 8) {
            $x0 = ($T.Lignes | ForEach-Object { $_.X } | Measure-Object -Minimum).Minimum - $M
            $x1 = ($T.Lignes | ForEach-Object { $_.X + $_.L } | Measure-Object -Maximum).Maximum + $M
            return [pscustomobject]@{ X0 = [int]$x0; Y0 = [int]($T.Lignes[0].Y - $M); X1 = [int]$x1; Y1 = [int]($T.Lignes[2].Y + $T.Lignes[2].H + $M) }
        }
        function Zone-De($R, [int]$M) {
            return [pscustomobject]@{ X0 = [int]($R.X - $M); Y0 = [int]($R.Y - $M); X1 = [int]($R.X + $R.L + $M); Y1 = [int]($R.Y + $R.H + $M) }
        }
        function Texte-Zone($Z) { return "$($Z.X0),$($Z.Y0) $($Z.X1 - $Z.X0 + 1)x$($Z.Y1 - $Z.Y0 + 1)" }

        # Un raccourci tenu un instant par l'essai (tenir-raccourci.ps1) : « tenu » si l'app
        # ne le tient pas, « refuse » sinon. L'aide est fermée ensuite (raccourci rendu).
        function Tenir-Ailleurs([int]$Vk, [string]$Nom) {
            $fichier = Join-Path $script:Essai.Dossier "tenu-$Nom-$([guid]::NewGuid().ToString('N').Substring(0, 6)).txt"
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -WindowStyle Hidden -ArgumentList @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'tenir-raccourci.ps1'), '-Vk', $Vk, '-Mods', $MOD_CTRL_MAJ, '-Fichier', $fichier)
            $script:Auxiliaires.Add($aide)
            $limite = [DateTime]::UtcNow.AddSeconds(30)
            while (-not (Test-Path -LiteralPath $fichier) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
            $lu = if (Test-Path -LiteralPath $fichier) { (Get-Content -LiteralPath $fichier -Raw).Trim() } else { 'pas de réponse en 30 s' }
            Arreter-Aide $aide
            return $lu
        }

        # --- UI Automation (essais 08 et 12) ----------------------------------------------
        function Elements-Vus([IntPtr]$Racine) {
            try {
                $tous = $AE::FromHandle($Racine).FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
                $noms = @()
                foreach ($el in $tous) {
                    $c = $el.Current
                    if ($c.Name -ne '') { $noms += "$($c.ControlType.ProgrammaticName -replace '^ControlType\.', '') « $($c.Name) »" }
                    if ($noms.Count -ge 60) { break }
                }
                return "$($tous.Count) éléments vus ; nommés : " + ($noms -join ', ')
            } catch { return "UI Automation en échec : $($_.Exception.Message)" }
        }
        function Condition([string]$Nom, $Type) {
            $cond = [System.Windows.Automation.PropertyCondition]::new($AE::NameProperty, $Nom)
            if ($null -ne $Type) {
                $cond = [System.Windows.Automation.AndCondition]::new($cond, [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, $Type))
            }
            return $cond
        }
        # Un élément de $Racine (HWND ou élément) par son nom (et son type), attendu ; $null.
        function Element($Racine, [string]$Nom, $Type = $null, [double]$DelaiS = 5) {
            $cond = Condition $Nom $Type
            $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
            do {
                try {
                    $base = if ($Racine -is [System.Windows.Automation.AutomationElement]) { $Racine } else { $AE::FromHandle([IntPtr]$Racine) }
                    $el = $base.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
                    if ($null -ne $el) { return $el }
                } catch { }
                Start-Sleep -Milliseconds 100
            } while ([DateTime]::UtcNow -lt $limite)
            return $null
        }
        function Element-Exige([IntPtr]$Fenetre, [string]$Nom, $Type, [string]$Quoi, [double]$DelaiS = 5) {
            $el = Element $Fenetre $Nom $Type $DelaiS
            if ($null -eq $el) {
                $t = if ($null -ne $Type) { $Type.ProgrammaticName } else { 'tout type' }
                throw "$Quoi : « $Nom » ($t) introuvable ; $(Elements-Vus $Fenetre)"
            }
            return $el
        }
        # Tous les éléments de ce nom et de ce type.
        function Elements([IntPtr]$Fenetre, [string]$Nom, $Type) {
            try { return @($AE::FromHandle($Fenetre).FindAll([System.Windows.Automation.TreeScope]::Descendants, (Condition $Nom $Type))) }
            catch { return @() }
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
        # Clic réel au centre de l'élément (garde : le point doit être sur l'app).
        function Cliquer-Element($el) {
            $p = Centre-Stable $el
            $E::Cliquer($p[0], $p[1])
            Start-Sleep -Milliseconds 150
        }
        function Cliquer-Nom([IntPtr]$Fenetre, [string]$Nom, $Type, [string]$Quoi) {
            Cliquer-Element (Element-Exige $Fenetre $Nom $Type $Quoi)
        }
        function Valeur($el) {
            try { return [string]$el.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value }
            catch { return '<<sans valeur>>' }
        }

        # La zone cliente à l'écran : { X, Y, L, H } en pixels réels, et l'échelle.
        function Zone-Cliente([IntPtr]$H) {
            $c = [BreezyShotEssai.Historique+RECT]::new()
            $null = $E::GetClientRect($H, [ref]$c)
            $o = [BreezyShotEssai.Historique+POINT]::new()
            $null = $E::ClientToScreen($H, [ref]$o)
            $dpi = $E::GetDpiForWindow($H)
            return [pscustomobject]@{ X = $o.X; Y = $o.Y; L = $c.Right; H = $c.Bottom; S = $(if ($dpi -gt 0) { $dpi / 96.0 } else { 1.0 }) }
        }

        # La fenêtre $H au premier plan : SetForegroundWindow, sinon un clic réel dans la
        # barre du haut de sa page (zone de glisser, $BarreCss px CSS sous le haut) ; vérifié.
        function Ramener-Devant([IntPtr]$H, [string]$Quoi, [int]$BarreCss = 16) {
            $null = $E::SetForegroundWindow($H)
            $limite = [DateTime]::UtcNow.AddSeconds(2)
            while ($E::GetForegroundWindow() -ne $H -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 50 }
            if ($E::GetForegroundWindow() -ne $H) {
                $z = Zone-Cliente $H
                $E::Cliquer([int]($z.X + $z.L / 2), [int]($z.Y + [math]::Round($BarreCss * $z.S)))
                $limite = [DateTime]::UtcNow.AddSeconds(3)
                while ($E::GetForegroundWindow() -ne $H -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 50 }
            }
            $E::Poser($REPOS_X, $REPOS_Y)
            if ($E::GetForegroundWindow() -ne $H) { throw "$Quoi : la fenêtre n'est pas revenue au premier plan ($($E::Decrire($E::GetForegroundWindow())))" }
        }

        # --- Menu de l'icône ------------------------------------------------------------------
        # Le vrai menu (clic droit sur l'icône, menu web lu par UI Automation) : commun.ps1.
        function Ouvrir-Menu([string]$Quoi, [string]$Dossier = $script:dossierCas) {
            return (Ouvrir-Menu-Icone $script:proc.Id $Dossier $Quoi)
        }
        function Fermer-Menu($M, [string]$Quoi) { Fermer-Menu-Icone $M $Quoi }
        function Cliquer-Entree($M, [string]$Texte, [string]$Quoi) {
            Cliquer-Ligne-Menu $M $Texte $Quoi
            $E::Poser($REPOS_X, $REPOS_Y)
        }

        # --- Réglages ------------------------------------------------------------------------
        # Ouvre la fenêtre des Réglages par le vrai menu : rend son HWND, page prête.
        function Ouvrir-Reglages([string]$Dossier, [string]$Quoi) {
            $vus = Compter-Motif -Motif 'menu: reglages$' -Dossier $Dossier
            $fen = Compter-Motif -Motif 'reglages fenetre \d+$' -Dossier $Dossier
            $pretes = Compter-Motif -Motif 'reglages page prete$' -Dossier $Dossier
            $m = Ouvrir-Menu $Quoi $Dossier
            Cliquer-Entree $m 'Réglages' $Quoi
            $null = Attendre-Suivante 'menu: reglages$' $Dossier $vus "$Quoi : menu: reglages" 5
            $l = Attendre-Suivante 'reglages fenetre (\d+)$' $Dossier $fen "$Quoi : fenêtre des Réglages" 10
            $null = $l -match 'reglages fenetre (\d+)$'
            $h = [IntPtr][long]$Matches[1]
            $null = Attendre-Suivante 'reglages page prete$' $Dossier $pretes "$Quoi : page des Réglages prête" 20
            Start-Sleep -Milliseconds 300
            return $h
        }
        function Fermer-Reglages([IntPtr]$H, [string]$Dossier, [string]$Quoi) {
            $fermes = Compter-Motif -Motif 'reglages fermes$' -Dossier $Dossier
            $null = $E::PostMessage($H, $WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
            $null = Attendre-Suivante 'reglages fermes$' $Dossier $fermes "$Quoi : Réglages fermés" 10
            $limite = [DateTime]::UtcNow.AddSeconds(5)
            while ($E::IsWindow($H) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
        }
        # Colle $Texte (presse-papiers, Ctrl+V réel) dans le champ « Licence », qui le garde.
        function Coller-Licence([IntPtr]$Fenetre, [string]$Texte, [string]$Quoi) {
            $champ = Element-Exige $Fenetre 'Licence' $CT::Edit $Quoi
            # Un clic réel dans le champ : la fenêtre des Réglages prend le premier plan (les
            # touches qui suivent sont gardées : elles ne partent que vers l'app).
            Cliquer-Element $champ
            $limite = [DateTime]::UtcNow.AddSeconds(2)
            while ($E::GetForegroundWindow() -ne $Fenetre -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 50 }
            if ($E::GetForegroundWindow() -ne $Fenetre) { throw "$Quoi : après le clic dans le champ, le premier plan est à $($E::Decrire($E::GetForegroundWindow()))" }
            Poser-Texte $Texte
            $E::Appuyer($CTRL, $VK_A)
            $E::Appuyer($CTRL, $VK_V)
            $limite = [DateTime]::UtcNow.AddSeconds(3)
            while ((Valeur $champ) -cne $Texte -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
            $v = Valeur $champ
            if ($v -cne $Texte) { throw "$Quoi : le champ « Licence » ne contient pas le texte collé (lu $($v.Length) caractères)" }
            return $champ
        }

        # --- Captures, vignette, éditeur (essai 12) -----------------------------------------
        # Ctrl + Maj + 2, puis la carte entrée : { Id, Carte, Recent }.
        function Capturer([string]$Dossier, [string]$Quoi) {
            $E::Poser($REPOS_X, $REPOS_Y)
            $entrees = Compter-Motif -Motif 'vignette entree \d+$' -Dossier $Dossier
            $avant = @(Fichiers-Png (Join-Path $Dossier 'local/Recents') | ForEach-Object { $_.FullName })
            $E::AppuyerGlobal($MODS, $VK_2)
            $l = Attendre-Suivante 'vignette entree (\d+)$' $Dossier $entrees "$Quoi : entrée de la vignette" 15
            $null = $l -match 'vignette entree (\d+)$'
            $id = [int]$Matches[1]
            $carte = Lire-Rect (Attendre-Motif -Motif "vignette zone $id " -Dossier $Dossier -DelaiS 3 -Quoi "$Quoi : zone de la carte")
            $nouveaux = @(Fichiers-Png (Join-Path $Dossier 'local/Recents') | Where-Object { $avant -notcontains $_.FullName })
            Start-Sleep -Milliseconds 350
            return [pscustomobject]@{ Id = $id; Carte = $carte; Recent = $(if ($nouveaux.Count -ge 1) { $nouveaux[-1].FullName } else { $null }) }
        }
        # Survole la carte, attend que le rectangle du bouton ne bouge plus, clique le bouton.
        function Cliquer-Bouton([string]$Dossier, $C, [string]$Nom) {
            $E::Poser($C.Carte.CX, $C.Carte.CY)
            $b = $null
            $limite = [DateTime]::UtcNow.AddSeconds(3)
            do {
                Start-Sleep -Milliseconds 150
                $avant = $b
                $b = Lire-Rect (Attendre-Motif -Motif "vignette bouton $($C.Id) $Nom " -Dossier $Dossier -DelaiS 3 -Quoi "bouton $Nom")
            } while (($null -eq $avant -or $avant.X -ne $b.X -or $avant.Y -ne $b.Y) -and [DateTime]::UtcNow -lt $limite)
            Start-Sleep -Milliseconds 300
            $E::Cliquer($b.CX, $b.CY)
        }
        # Annoter sur la carte $C, puis l'image affichée : rend le HWND de l'éditeur.
        function Ouvrir-Editeur([string]$Dossier, $C, [string]$Quoi) {
            $affichees = Compter-Motif -Motif "editeur image affichee $($C.Id) " -Dossier $Dossier
            Cliquer-Bouton $Dossier $C 'annoter'
            $null = Attendre-Motif -Motif "vignette fermee $($C.Id) annoter$" -Dossier $Dossier -DelaiS 5 -Quoi "$Quoi : vignette fermee annoter"
            $null = Attendre-Motif -Motif "editeur image affichee $($C.Id) en \d+ ms$" -Dossier $Dossier -DelaiS 40 -Quoi "$Quoi : image affichée" -Fois ($affichees + 1)
            $h = $E::FenetresTitre($script:proc.Id, $TITRE_EDITEUR)
            if ($h.Count -ne 1) { throw "$Quoi : $($h.Count) fenêtre(s) « $TITRE_EDITEUR »" }
            Start-Sleep -Milliseconds 300
            return $h[0]
        }
        # Ferme l'éditeur $Id par WM_CLOSE ; modifié, « Fermer sans copier » au menu « Fermer ».
        function Fermer-Editeur([IntPtr]$H, [string]$Dossier, [int]$Id, [string]$Quoi) {
            $avant = Compter-Motif -Motif "editeur (ferme $Id |fermeture $Id`: demander$)" -Dossier $Dossier
            $null = $E::PostMessage($H, $WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
            $l = Attendre-Suivante "editeur (ferme $Id |fermeture $Id`: demander$)" $Dossier $avant "$Quoi : fermeture de l'éditeur" 10
            if ($l -match "fermeture $Id`: demander$") {
                $sans = Element-Exige $H 'Fermer sans copier' $CT::MenuItem $Quoi
                Cliquer-Element $sans
                $null = Attendre-Motif -Motif "editeur ferme $Id \(sans garder\)$" -Dossier $Dossier -DelaiS 10 -Quoi "$Quoi : ferme (sans garder)"
            }
            $limite = [DateTime]::UtcNow.AddSeconds(5)
            while ($E::IsWindow($H) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
            if ($E::IsWindow($H)) { throw "$Quoi : fenêtre de l'éditeur non détruite" }
            $E::Poser($REPOS_X, $REPOS_Y)
        }
        # La vue de l'éditeur (Ajuster, Ctrl+0) : rectangle de l'image à l'écran et échelle.
        function Ajuster([string]$Dossier, [int]$Id, [int]$LargeurImage) {
            $vues = Compter-Motif -Motif "editeur vue $Id " -Dossier $Dossier
            $E::Appuyer($CTRL, [byte]0x30)
            $v = Lire-Rect (Attendre-Suivante "editeur vue $Id " $Dossier $vues 'vue après Ctrl+0' 5)
            $v | Add-Member S ($v.L / $LargeurImage)
            return $v
        }
        function Ecran-X($Vue, [double]$X) { return [int][math]::Round($Vue.X + $X * $Vue.S) }
        function Ecran-Y($Vue, [double]$Y) { return [int][math]::Round($Vue.Y + $Y * $Vue.S) }
        # Le bouton « Texte » de la barre du haut de l'éditeur : un bouton de ce nom à la
        # hauteur de « Enregistrer » (barre du haut). L'instrument « Texte » de la palette
        # (choisi par Ajouter › Texte) porte le même nom, mais pas à cette hauteur.
        function Bouton-Texte([IntPtr]$H, [double]$DelaiS = 5) {
            $enr = Element $H 'Enregistrer' $CT::Button 5
            if ($null -eq $enr) { throw "éditeur : bouton « Enregistrer » introuvable ; $(Elements-Vus $H)" }
            $re = $enr.Current.BoundingRectangle
            $yBarre = $re.Y + $re.Height / 2
            $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
            do {
                $b = @(Elements $H 'Texte' $CT::Button | Where-Object {
                        $rb = $_.Current.BoundingRectangle
                        (-not $rb.IsEmpty) -and ([math]::Abs($rb.Y + $rb.Height / 2 - $yBarre) -le 8)
                    })
                if ($b.Count -gt 0) { return $b[0] }
                Start-Sleep -Milliseconds 100
            } while ([DateTime]::UtcNow -lt $limite)
            return $null
        }

        # --- Copier le texte (raccourci) -----------------------------------------------------
        # Ctrl+Maj+T, puis la sélection en mode texte prête : rend son numéro.
        function Ouvrir-Texte([string]$Dossier, [string]$Quoi) {
            $gels = Compter-Motif -Motif 'etat Repos -> Gel \(DemandeCapture texte\)$' -Dossier $Dossier
            $montrees = Compter-Motif -Motif 'selection montree \d+ ' -Dossier $Dossier
            $pretes = Compter-Motif -Motif 'chrono selection prete \d+' -Dossier $Dossier
            $E::Poser($REPOS_X, $REPOS_Y)
            $E::AppuyerGlobal($MODS, $VK_T)
            $null = Attendre-Suivante 'etat Repos -> Gel \(DemandeCapture texte\)$' $Dossier $gels "$Quoi : DemandeCapture texte" 5
            $m = Attendre-Suivante 'selection montree (\d+) ' $Dossier $montrees "$Quoi : sélection montrée" 5
            $null = Attendre-Suivante 'chrono selection prete \d+' $Dossier $pretes "$Quoi : page de sélection prête" 5
            if ($m -notmatch 'selection montree (\d+) .* mode texte ') { throw "$Quoi : sélection hors du mode texte : $m" }
            Start-Sleep -Milliseconds 150
            return [int]([regex]::Match($m, 'selection montree (\d+) ').Groups[1].Value)
        }
        # Une lecture complète du raccourci sur la zone $Z : rend { Issue (ligne texte lu|vide|
        # impossible), Copie (ligne texte copie… ou $null), Chrono }.
        function Lire-Zone([string]$Dossier, $Z, [string]$Quoi) {
            $issues = Compter-Motif -Motif 'texte (lu \d+ lignes|vide en|impossible: )' -Dossier $Dossier
            $chronos = Compter-Motif -Motif 'chrono texte relache-gel-cache ' -Dossier $Dossier
            $null = Ouvrir-Texte $Dossier $Quoi
            $E::Tracer($Z.X0, $Z.Y0, $Z.X1, $Z.Y1)
            $E::Poser($REPOS_X, $REPOS_Y)
            $issue = Attendre-Suivante 'texte (lu \d+ lignes|vide en|impossible: )' $Dossier $issues "$Quoi : fin de la lecture" 15
            $chrono = Attendre-Suivante 'chrono texte relache-gel-cache ' $Dossier $chronos "$Quoi : chrono de la lecture" 5
            $copie = Derniere-Ligne -Motif 'texte copie' -Dossier $Dossier
            return [pscustomobject]@{ Issue = $issue; Copie = $copie; Chrono = $chrono }
        }

        # --- Historique ----------------------------------------------------------------------
        # Ctrl+Maj+H (raccourci global de l'app), puis la fenêtre créée et sa première rangée
        # peinte : rend { H, Ms }.
        function Ouvrir-Historique([string]$Dossier, [string]$Quoi) {
            $fen = Compter-Motif -Motif 'historique fenetre \d+$' -Dossier $Dossier
            $aff = Compter-Motif -Motif 'historique affiche \d+ en \d+ ms$' -Dossier $Dossier
            $E::Poser($REPOS_X, $REPOS_Y)
            $E::AppuyerGlobal($MODS, $VK_H)
            $l = Attendre-Suivante 'historique fenetre (\d+)$' $Dossier $fen "$Quoi : historique fenetre" 10
            $null = $l -match 'historique fenetre (\d+)$'
            $h = [IntPtr][long]$Matches[1]
            $a = Attendre-Suivante 'historique affiche \d+ en (\d+) ms$' $Dossier $aff "$Quoi : historique affiche" 20
            $null = $a -match 'en (\d+) ms$'
            return [pscustomobject]@{ H = $h; Ms = [int]$Matches[1] }
        }
        # Les cases montrées, dans l'ordre : { Item, Case (bouton « Capture, … »), Nom }.
        function Cases([IntPtr]$H) {
            $liste = Element $H 'Captures récentes' $CT::List 0.5
            if ($null -eq $liste -or $liste.Current.IsOffscreen) { return @() }
            $cases = [System.Collections.Generic.List[object]]::new()
            $items = $liste.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, $CT::ListItem))
            foreach ($it in $items) {
                $boutons = $it.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, $CT::Button))
                foreach ($b in $boutons) {
                    if ($b.Current.Name.StartsWith('Capture, ', [StringComparison]::Ordinal)) {
                        $cases.Add([pscustomobject]@{ Item = $it; Case = $b; Nom = $b.Current.Name }); break
                    }
                }
            }
            return @($cases)
        }
        function Attendre-Cases([IntPtr]$H, [int]$N, [string]$Quoi, [double]$DelaiS = 5) {
            $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
            do {
                $c = @(Cases $H)
                if ($c.Count -eq $N) { return $c }
                Start-Sleep -Milliseconds 100
            } while ([DateTime]::UtcNow -lt $limite)
            throw "$Quoi : $(@(Cases $H).Count) case(s) au lieu de $N en $DelaiS s ; $(Elements-Vus $H)"
        }
        # Le bouton d'action $Nom (Copier, Enregistrer, Annoter, Supprimer) de la case $C,
        # après un survol réel de la case.
        function Cliquer-Action($C, [string]$Nom, [string]$Quoi) {
            $p = Centre-Stable $C.Case
            $E::Poser($p[0], $p[1])
            Start-Sleep -Milliseconds 400
            $b = Element $C.Item $Nom $CT::Button 2
            if ($null -eq $b) { throw "$Quoi : bouton « $Nom » introuvable dans la case" }
            Cliquer-Element $b
        }
        # Clic réel sur la case (elle prend le focus), puis la touche.
        function Touche-Case($C, [byte[]]$Modifs, [byte]$Vk) {
            Cliquer-Element $C.Case
            Start-Sleep -Milliseconds 200
            $E::Appuyer($Modifs, $Vk)
        }
        function Fermer-Historique([IntPtr]$H, [string]$Dossier, [string]$Quoi) {
            $n = Compter-Motif -Motif 'historique ferme \(cadre\)$' -Dossier $Dossier
            $null = $E::PostMessage($H, $WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
            $null = Attendre-Suivante 'historique ferme \(cadre\)$' $Dossier $n "$Quoi : ferme (cadre)" 10
            $limite = [DateTime]::UtcNow.AddSeconds(5)
            while ($E::IsWindow($H) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
            if ($E::IsWindow($H)) { throw "$Quoi : fenêtre de l'historique non détruite" }
        }

        # Un PNG RGBA 8 bits uni de $L × $H (ce qu'écrivait la 0.4 dans « Récents »).
        function Ecrire-Png([string]$Chemin, [int]$L, [int]$H, [byte[]]$Rgba) {
            $ligne = [byte[]]::new(1 + 4 * $L)
            for ($x = 0; $x -lt $L; $x++) { [Array]::Copy($Rgba, 0, $ligne, 1 + 4 * $x, 4) }
            $brut = [byte[]]::new($ligne.Length * $H)
            for ($y = 0; $y -lt $H; $y++) { [Array]::Copy($ligne, 0, $brut, $y * $ligne.Length, $ligne.Length) }
            $m = [System.IO.MemoryStream]::new()
            $z = [System.IO.Compression.ZLibStream]::new($m, [System.IO.Compression.CompressionLevel]::Optimal, $true)
            $z.Write($brut, 0, $brut.Length); $z.Dispose()
            $sortie = [System.IO.MemoryStream]::new()
            $sortie.Write([byte[]]@(0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A), 0, 8)
            $grand = { param([uint32]$v) $o = [BitConverter]::GetBytes($v); [Array]::Reverse($o); , $o }
            $morceau = {
                param([string]$Type, [byte[]]$Donnees)
                $t = [System.Text.Encoding]::ASCII.GetBytes($Type)
                $sortie.Write((& $grand ([uint32]$Donnees.Length)), 0, 4)
                $td = [byte[]]::new(4 + $Donnees.Length)
                [Array]::Copy($t, 0, $td, 0, 4); [Array]::Copy($Donnees, 0, $td, 4, $Donnees.Length)
                $sortie.Write($td, 0, $td.Length)
                $sortie.Write((& $grand ($E::Crc32($td))), 0, 4)
            }
            $ihdr = [byte[]]::new(13)
            [Array]::Copy((& $grand ([uint32]$L)), 0, $ihdr, 0, 4)
            [Array]::Copy((& $grand ([uint32]$H)), 0, $ihdr, 4, 4)
            $ihdr[8] = 8; $ihdr[9] = 6
            & $morceau 'IHDR' $ihdr
            & $morceau 'IDAT' $m.ToArray()
            & $morceau 'IEND' ([byte[]]::new(0))
            [System.IO.File]::WriteAllBytes($Chemin, $sortie.ToArray())
        }

        $script:proc = $null
        try {
            # ===================== Cas A : licence, puis Copier le texte =====================
            $da = Dossier-Cas 'a'
            $imagesA = Join-Path $da 'images'
            New-Item -ItemType Directory -Force -Path $imagesA | Out-Null
            Ecrire-Reglages -Dossier $da -Images $imagesA
            $script:proc = Demarrer-Cas -Dossier $da -Quoi '(A)'
            Start-Sleep -Seconds 3
            $script:reposA = Memoire-Privee-Mo -Processus $script:proc
            $script:reposAppA = Memoire-App $script:proc
            Noter-Mesure -Nom 'memoire privee au repos sans licence (app et WebView2)' -Valeur $script:reposA -Unite 'Mo'

            # (L1) Sans licence.
            Point '(L1) sans licence' {
                Verifier '(L1) licence absente' ((Compter-Motif -Motif 'licence absente$' -Dossier $da) -eq 1) | Out-Null
                Verifier '(L1) aucune ligne raccourci historique ni raccourci texte' ((Compter-Motif -Motif 'raccourci (historique|texte)[: ]' -Dossier $da) -eq 0) | Out-Null
                Verifier '(L1) sans licence, la sonde des langues ne tourne pas (aucune ligne texte langues)' ((Compter-Motif -Motif 'texte (langues|non pris en charge)' -Dossier $da) -eq 0) (Derniere-Ligne -Motif 'texte (langues|non pris)' -Dossier $da) | Out-Null
                $m = Ouvrir-Menu '(L1)'
                $pro = @($m.Textes | Where-Object { $_.StartsWith('Historique', [StringComparison]::Ordinal) -or $_.StartsWith('Copier le texte', [StringComparison]::Ordinal) })
                Verifier '(L1) menu sans « Historique » ni « Copier le texte » (9 positions)' (($pro.Count -eq 0) -and ($m.Textes.Count -eq 9)) $m.Lus | Out-Null
                Fermer-Menu $m '(L1)'
                $h = Tenir-Ailleurs $VK_H 'h-sans'
                $t = Tenir-Ailleurs $VK_T 't-sans'
                Verifier '(L1) Ctrl+Maj+H et Ctrl+Maj+T libres (l''essai les tient)' (($h -eq 'tenu') -and ($t -eq 'tenu')) "H $h, T $t" | Out-Null
            }

            # (L2) et (L3) La licence dans les Réglages.
            $script:fenR = [IntPtr]::Zero
            Point '(L2) licence refusée' {
                $script:fenR = Ouvrir-Reglages $da '(L2)'
                Cliquer-Nom $script:fenR 'Général' $CT::TabItem '(L2) rubrique Général'
                $refus = Compter-Motif -Motif 'licence refusee ' -Dossier $da
                $champ = Coller-Licence $script:fenR $LICENCE_FAUSSE '(L2)'
                Cliquer-Nom $script:fenR 'Activer' $CT::Button '(L2) Activer'
                $l = Attendre-Suivante 'licence refusee (\w+)$' $da $refus '(L2) licence refusee' 10
                Verifier '(L2) licence refusee signature' ($l -match 'licence refusee signature$') $l | Out-Null
                $texte = Element $script:fenR 'Licence non valide' $null 3
                Verifier '(L2) la ligne dit « Licence non valide »' ($null -ne $texte) $(if ($null -eq $texte) { Elements-Vus $script:fenR } else { '' }) | Out-Null
                Verifier '(L2) le champ garde le texte collé' ((Valeur $champ) -ceq $LICENCE_FAUSSE) | Out-Null
                Verifier '(L2) aucune licence rangée, aucune entrée de plus' ((-not (Test-Path -LiteralPath (Chemin-Licence $da))) -and ((Compter-Motif -Motif 'licence active ' -Dossier $da) -eq 0)) | Out-Null
            }
            Point '(L3) licence activée' {
                if ($script:fenR -eq [IntPtr]::Zero) { throw '(L3) pas de fenêtre des Réglages' }
                $null = Coller-Licence $script:fenR $LICENCE '(L3)'
                Cliquer-Nom $script:fenR 'Activer' $CT::Button '(L3) Activer'
                $null = Attendre-Motif -Motif 'licence active ESSAI-0001$' -Dossier $da -DelaiS 10 -Quoi '(L3) licence active'
                $lh = Attendre-Motif -Motif 'raccourci historique: ' -Dossier $da -DelaiS 10 -Quoi '(L3) raccourci historique'
                # La sonde des langues part à la première activation ; « Copier le texte » (son
                # raccourci, son entrée du menu) n'arrive qu'après elle.
                $null = Attendre-Motif -Motif 'texte (langues|non pris en charge)' -Dossier $da -DelaiS 30 -Quoi '(L3) sonde des langues après l''activation'
                $lt = Attendre-Motif -Motif 'raccourci texte: ' -Dossier $da -DelaiS 10 -Quoi '(L3) raccourci texte'
                Verifier '(L3) sans redémarrage : raccourci historique: Ctrl+Maj+H, raccourci texte: Ctrl+Maj+T' (($lh -match 'raccourci historique: Ctrl\+Maj\+H$') -and ($lt -match 'raccourci texte: Ctrl\+Maj\+T$')) "$lh ; $lt" | Out-Null
                $pro = Element $script:fenR 'Pro activé' $null 5
                $retirer = Element $script:fenR 'Retirer' $CT::Button 2
                Verifier '(L3) la ligne dit « Pro activé », bouton « Retirer »' (($null -ne $pro) -and ($null -ne $retirer)) $(if ($null -eq $pro -or $null -eq $retirer) { Elements-Vus $script:fenR } else { '' }) | Out-Null
                $range = if (Test-Path -LiteralPath (Chemin-Licence $da)) { [System.IO.File]::ReadAllText((Chemin-Licence $da)) } else { '<<absent>>' }
                Verifier '(L3) licence.txt rangé (texte nettoyé)' ($range.Trim() -ceq $LICENCE) "$($range.Length) caractères" | Out-Null
                Verifier '(L3) Ctrl+Maj+H tenu par l''app' ((Tenir-Ailleurs $VK_H 'h-pro') -eq 'refuse') | Out-Null
                Fermer-Reglages $script:fenR $da '(L3)'
            }

            # (1) La langue de lecture de la machine (sonde lancée par l'activation) : sans
            # elle, rien ne peut être prouvé.
            $langues = $null
            try { $langues = Attendre-Motif -Motif 'texte (langues|non pris en charge)' -Dossier $da -DelaiS 30 -Quoi '(1) sonde des langues' } catch { $langues = "$_" }
            $enUS = ($langues -match 'texte langues (\S+)$') -and (@($Matches[1] -split ',') -contains 'en-US')
            if (-not (Verifier '(1) texte langues contient en-US (une ligne, après l''activation)' ($enUS -and ((Compter-Motif -Motif 'texte (langues|non pris en charge)' -Dossier $da) -eq 1)) "$langues")) {
                throw "langue de lecture absente sur la machine d'essai ($langues)"
            }

            # (2) Le menu avec la licence.
            Point '(2) menu avec la licence' {
                $m = Ouvrir-Menu '(2)'
                $t = $m.Textes
                # Piste A « Liste » : … séparateur, dernière zone, Capture défilante (0.6, avec
                # la licence ; les réglages v3 du cas reçoivent son défaut Ctrl+Maj+5 à la mise
                # à niveau), Copier le texte, séparateur, Historique, Réglages.
                $ok = ($t.Count -ge 10) -and ($t[5] -ceq "Capture défilante`tCtrl + Maj + 5") -and ($t[6] -ceq "Copier le texte`tCtrl + Maj + T") -and ($t[7] -eq '') -and ($t[8] -ceq "Historique`tCtrl + Maj + H") -and ($t[9] -ceq 'Réglages')
                Verifier '(2) « Capture défilante<tab>Ctrl + Maj + 5 » puis « Copier le texte<tab>Ctrl + Maj + T » après la dernière zone, séparateur, « Historique<tab>Ctrl + Maj + H » puis « Réglages »' $ok $m.Lus | Out-Null
                $menus = Compter-Motif -Motif 'menu: ' -Dossier $da
                Fermer-Menu $m '(2)'
                Verifier '(2) Échap ferme le menu, aucune entrée choisie' ($script:FermeParEchap -and ((Compter-Motif -Motif 'menu: ' -Dossier $da) -eq $menus)) | Out-Null
            }

            # Le témoin de texte, à gauche, sous la place du bandeau.
            $script:temoin = Lancer-Temoin 'a' ([int]($utile.X + 40)) ([int]($utile.Y + 130))
            $zone3 = Zone-Lignes $script:temoin
            $blanc = Zone-De $script:temoin.Blanc 0
            $fige = [IntPtr][long]([regex]::Match((Attendre-Motif -Motif 'ecran fige fenetre \d+$' -Dossier $da -DelaiS 3 -Quoi 'HWND de l''écran figé'), 'fenetre (\d+)$').Groups[1].Value)
            $bandeau = [IntPtr][long]([regex]::Match((Attendre-Motif -Motif 'bandeau fenetre \d+$' -Dossier $da -DelaiS 3 -Quoi 'HWND du bandeau'), 'fenetre (\d+)$').Groups[1].Value)

            # (3) Ctrl+Maj+T sur les trois lignes.
            Point '(3) Copier le texte' {
                Poser-Texte 'temoin-13'
                $issues = Compter-Motif -Motif 'texte (lu \d+ lignes|vide en|impossible: )' -Dossier $da
                $lectures = Compter-Motif -Motif 'etat Selection -> Lecture \(TexteChoisi\)$' -Dossier $da
                $reussites = Compter-Motif -Motif 'bandeau affiche \d+ reussite ' -Dossier $da
                $null = Ouvrir-Texte $da '(3)'
                Verifier '(3) DemandeCapture texte, selection montree … mode texte' $true | Out-Null
                $cache = $E::TracerPuisGuetter($zone3.X0, $zone3.Y0, $zone3.X1, $zone3.Y1, $fige, 3000)
                $E::Poser($REPOS_X, $REPOS_Y)
                Noter-Mesure -Nom 'relache -> ecran fige cache (guette toutes les 2 ms)' -Valeur $cache -Unite 'ms'
                Verifier '(3) écran figé encore visible 100 ms après le relâcher, caché ensuite (≤ 1 500 ms + 200)' (($cache -ge 100) -and ($cache -le 1700)) "caché à $cache ms (-1 : déjà caché au relâcher)" | Out-Null
                $z = Attendre-Motif -Motif 'zone texte ' -Dossier $da -DelaiS 5 -Quoi '(3) zone texte'
                Verifier '(3) zone texte = la zone tracée' ($z -match "zone texte $(Texte-Zone $zone3)$") "$z ; tracée $(Texte-Zone $zone3)" | Out-Null
                $null = Attendre-Suivante 'etat Selection -> Lecture \(TexteChoisi\)$' $da $lectures '(3) Selection -> Lecture' 5
                $lu = Attendre-Suivante 'texte (lu \d+ lignes|vide en|impossible: )' $da $issues '(3) fin de la lecture' 15
                Verifier '(3) texte lu 3 lignes 11 mots' ($lu -match 'texte lu 3 lignes 11 mots en \d+ ms langue en-US ') $lu | Out-Null
                if ($lu -match 'en (\d+) ms') { Noter-Mesure -Nom 'lecture des trois lignes (budget 400)' -Valeur ([int]$Matches[1]) -Unite 'ms' }
                $copie = Attendre-Motif -Motif 'texte copie' -Dossier $da -DelaiS 5 -Quoi '(3) texte copie'
                Verifier '(3) texte copie' ($copie -match 'texte copie$') $copie | Out-Null
                $attendu = $LIGNES_TEMOIN -join "`r`n"
                $pp = Lire-Texte
                Verifier '(3) presse-papiers = les trois lignes, séparées par CR LF, exactement' ($pp -ceq $attendu) "lu « $(Montrer $pp) »" | Out-Null
                $null = Attendre-Suivante 'bandeau affiche \d+ reussite ' $da $reussites '(3) bandeau de réussite' 5
                Verifier '(3) bandeau affiche … reussite' $true | Out-Null
                $chrono = Attendre-Motif -Motif 'chrono texte relache-gel-cache (\d+) relache-message (\d+)$' -Dossier $da -DelaiS 5 -Quoi '(3) chrono'
                $null = $chrono -match 'relache-gel-cache (\d+) relache-message (\d+)$'
                Noter-Mesure -Nom 'relache -> gel cache (journal)' -Valeur ([int]$Matches[1]) -Unite 'ms'
                Noter-Mesure -Nom 'relache -> Texte copie (budget max(400, lecture) + 150)' -Valeur ([int]$Matches[2]) -Unite 'ms'
                $moteur = Derniere-Ligne -Motif 'texte moteur cree en \d+ ms$' -Dossier $da
                if ($null -ne $moteur -and $moteur -match 'en (\d+) ms$') { Noter-Mesure -Nom 'texte moteur cree (R15 : 150)' -Valeur ([int]$Matches[1]) -Unite 'ms' }
            }

            # (3b) La ligne de 12 px : lue, résultat seulement noté (R11).
            Point '(3b) ligne de 12 px (notée)' {
                $r = Lire-Zone $da (Zone-De $script:temoin.Petite 6) '(3b)'
                $texte = if ($r.Issue -match 'texte lu ') { Montrer (Lire-Texte) } else { '' }
                Noter-Mesure -Nom 'ligne de 12 px : journal' -Valeur ($r.Issue -replace '^\S+\s+\S+\s+\S+\s+', '') -Unite ''
                Noter-Mesure -Nom 'ligne de 12 px : texte lu (attendu « Hello BreezyShot 2026, the quick brown fox: 42 »)' -Valeur $texte -Unite ''
                $null = Attendre-Motif -Motif 'etat Lecture -> Repos' -Dossier $da -DelaiS 5 -Quoi '(3b) retour au repos'
            }

            # (4) La partie blanche : rien de trouvé, l'erreur part seule.
            Point '(4) zone blanche' {
                Poser-Texte 'temoin-13'
                $seq = $E::GetClipboardSequenceNumber()
                $erreurs = Compter-Motif -Motif 'bandeau affiche \d+ erreur ' -Dossier $da
                $actions = Compter-Motif -Motif 'bandeau action ' -Dossier $da
                $r = Lire-Zone $da $blanc '(4)'
                # L'accès UI Automation au bandeau préparé ici (le premier appel peut prendre
                # 1 à 2 s ; en (5), « Réessayer » doit être cliqué avant les 4 s de l'erreur).
                $null = Element-Exige $bandeau 'Réessayer' $CT::Button '(4) action du bandeau' 3
                Verifier '(4) « Réessayer » sur l''erreur du bandeau' $true | Out-Null
                Verifier '(4) texte vide' ($r.Issue -match 'texte vide en \d+ ms$') $r.Issue | Out-Null
                $aff = Attendre-Suivante 'bandeau affiche (\d+) erreur ' $da $erreurs '(4) bandeau d''erreur' 5
                $id = [regex]::Match($aff, 'bandeau affiche (\d+) erreur ').Groups[1].Value
                $fin = Attendre-Motif -Motif "bandeau termine $id$" -Dossier $da -DelaiS 8 -Quoi '(4) fin de l''erreur'
                $s = ((Heure-Ligne $fin) - (Heure-Ligne $aff)).TotalSeconds
                Noter-Mesure -Nom '« Aucun texte trouvé » : affiche -> termine' -Valeur ([math]::Round($s, 2)) -Unite 's'
                Verifier '(4) bandeau termine entre 3,8 et 5 s, sans clic' (($s -ge 3.8) -and ($s -le 5.0) -and ((Compter-Motif -Motif 'bandeau action ' -Dossier $da) -eq $actions)) "$([math]::Round($s, 2)) s" | Out-Null
                Verifier '(4) presse-papiers inchangé' (($E::GetClipboardSequenceNumber() -eq $seq) -and ((Lire-Texte) -ceq 'temoin-13')) | Out-Null
            }

            # (5) Encore ; « Réessayer » relance Copier le texte ; Échap : rien.
            Point '(5) Réessayer puis Échap' {
                $seq = $E::GetClipboardSequenceNumber()
                $erreurs = Compter-Motif -Motif 'bandeau affiche \d+ erreur ' -Dossier $da
                $r = Lire-Zone $da $blanc '(5)'
                $aff = Attendre-Suivante 'bandeau affiche (\d+) erreur ' $da $erreurs '(5) bandeau d''erreur' 5
                $id = [regex]::Match($aff, 'bandeau affiche (\d+) erreur ').Groups[1].Value
                $null = Attendre-Motif -Motif "bandeau zone $id " -Dossier $da -DelaiS 5 -Quoi '(5) zone de la pilule'
                $b = Element-Exige $bandeau 'Réessayer' $CT::Button '(5) action du bandeau' 3
                $gels = Compter-Motif -Motif 'etat Repos -> Gel \(DemandeCapture texte\)$' -Dossier $da
                $montrees = Compter-Motif -Motif 'selection montree \d+ ' -Dossier $da
                $pretes = Compter-Motif -Motif 'chrono selection prete \d+' -Dossier $da
                $annulees = Compter-Motif -Motif 'etat Selection -> Repos \(SelectionAnnulee\)$' -Dossier $da
                $affiches = Compter-Motif -Motif 'bandeau affiche ' -Dossier $da
                $p = Centre-Stable $b
                if (-not $E::PoserSurApp($p[0], $p[1], 1500)) {
                    throw "(5) la pilule ne reçoit pas les clics 1,5 s après l'arrivée de la souris en $($p[0]),$($p[1]) ; $(Diagnostic-Journal $da)"
                }
                $E::Cliquer($p[0], $p[1])
                Start-Sleep -Milliseconds 150
                $E::Poser($REPOS_X, $REPOS_Y)
                $null = Attendre-Motif -Motif "bandeau action $id reessayer$" -Dossier $da -DelaiS 5 -Quoi '(5) bandeau action reessayer'
                $null = Attendre-Suivante 'etat Repos -> Gel \(DemandeCapture texte\)$' $da $gels '(5) DemandeCapture texte' 5
                $m = Attendre-Suivante 'selection montree \d+ ' $da $montrees '(5) sélection montrée' 5
                $null = Attendre-Suivante 'chrono selection prete \d+' $da $pretes '(5) page de sélection prête' 5
                Verifier '(5) bandeau action <id> reessayer, DemandeCapture texte, mode texte' ($m -match ' mode texte ') $m | Out-Null
                Start-Sleep -Milliseconds 200
                # Échap est un raccourci global de l'app pendant la sélection (`echap global reserve`).
                $E::AppuyerGlobal($AUCUN, $ECHAP)
                $null = Attendre-Suivante 'etat Selection -> Repos \(SelectionAnnulee\)$' $da $annulees '(5) Échap : annulée' 5
                Start-Sleep -Milliseconds 800
                Verifier '(5) Échap : rien, aucun message, presse-papiers inchangé' (((Compter-Motif -Motif 'bandeau affiche ' -Dossier $da) -eq $affiches) -and ($E::GetClipboardSequenceNumber() -eq $seq) -and ((Lire-Texte) -ceq 'temoin-13')) | Out-Null
            }
            Arreter-Aide $script:temoin.Aide
            Fermer-Cas $script:proc $da '(A)'

            # (6) Redémarrage : retardateur 3 s et pointeur allumés dans le fichier.
            $script:temoin = Lancer-Temoin 'a6' ([int]($utile.X + 40)) ([int]($utile.Y + 130))
            $zone3 = Zone-Lignes $script:temoin
            Ecrire-Reglages -Dossier $da -Images $imagesA -Retardateur 3 -Pointeur $true
            $script:proc = Demarrer-Cas -Dossier $da -Quoi '(6)'
            Point '(6) sans retardateur ni pointeur' {
                Verifier '(6) démarrage avec licence.txt en place : licence active ESSAI-0001' ((Compter-Motif -Motif 'licence active ESSAI-0001$' -Dossier $da) -eq 2) | Out-Null
                $null = Attendre-Motif -Motif 'raccourci texte: Ctrl\+Maj\+T$' -Dossier $da -DelaiS 10 -Quoi '(6) raccourci texte' -Fois 2
                $decomptes = Compter-Motif -Motif 'decompte \d+ ' -Dossier $da
                $pointeurs = Compter-Motif -Motif 'pointeur (pris|absent|pose|hors zone)' -Dossier $da
                $r = Lire-Zone $da $zone3 '(6)'
                Verifier '(6) Ctrl+Maj+T lit tout de suite' ($r.Issue -match 'texte lu 3 lignes 11 mots ') $r.Issue | Out-Null
                Verifier '(6) ni decompte ni pointeur' (((Compter-Motif -Motif 'decompte \d+ ' -Dossier $da) -eq $decomptes) -and ((Compter-Motif -Motif 'pointeur (pris|absent|pose|hors zone)' -Dossier $da) -eq $pointeurs)) | Out-Null
            }

            # (7) Ctrl+Maj+2 pendant la lecture de l'écran entier.
            Point '(7) capture pendant une lecture' {
                $ignores = Compter-Motif -Motif 'capture deja en cours: appui ignore$' -Dossier $da
                $presses = Compter-Motif -Motif 'raccourci presse: plein_ecran$' -Dossier $da
                $gels = Compter-Motif -Motif 'Demande(Capture|Decompte) plein_ecran' -Dossier $da
                $issues = Compter-Motif -Motif 'texte (lu \d+ lignes|vide en|impossible: )' -Dossier $da
                $null = Ouvrir-Texte $da '(7)'
                $E::TracerPuisAppuyer($ecran.X + 2, $ecran.Y + 2, $ecran.Right - 3, $ecran.Bottom - 3, $MODS, $VK_2)
                $E::Poser($REPOS_X, $REPOS_Y)
                $null = Attendre-Suivante 'raccourci presse: plein_ecran$' $da $presses '(7) Ctrl+Maj+2 reçu' 5
                $null = Attendre-Suivante 'capture deja en cours: appui ignore$' $da $ignores '(7) appui ignore' 5
                $lu = Attendre-Suivante 'texte (lu \d+ lignes|vide en|impossible: )' $da $issues '(7) fin de la lecture' 15
                Start-Sleep -Milliseconds 1500
                Verifier '(7) Ctrl+Maj+2 juste après le relâcher (en Sélection ou en Lecture) : appui ignore, aucune capture ni décompte' ((Compter-Motif -Motif 'Demande(Capture|Decompte) plein_ecran' -Dossier $da) -eq $gels) | Out-Null
                Verifier '(7) écran entier lu' ($lu -match 'texte (lu \d+ lignes|vide en)') $lu | Out-Null
                if ($lu -match 'en (\d+) ms') { Noter-Mesure -Nom 'lecture de l''écran entier (budget 1000)' -Valeur ([int]$Matches[1]) -Unite 'ms' }
                $null = Attendre-Motif -Motif 'etat Lecture -> Repos' -Dossier $da -DelaiS 5 -Quoi '(7) retour au repos'
            }

            # (8) Dix lectures : la mémoire de l'app.
            Point '(8) dix lectures' {
                Start-Sleep -Seconds 2
                $avant = Memoire-App $script:proc
                for ($i = 1; $i -le 10; $i++) {
                    $r = Lire-Zone $da $zone3 "(8) $i"
                    if ($r.Issue -notmatch 'texte lu 3 lignes 11 mots ') { throw "(8) lecture $i : $($r.Issue)" }
                }
                Start-Sleep -Seconds 3
                $apres = Memoire-App $script:proc
                Noter-Mesure -Nom 'memoire de l''app avant dix lectures' -Valeur $avant -Unite 'Mo'
                Noter-Mesure -Nom 'memoire de l''app apres dix lectures (budget repos + 30, 1 Mo par lecture au plus)' -Valeur $apres -Unite 'Mo'
                Noter-Mesure -Nom 'memoire de l''app au repos sans licence' -Valeur $script:reposAppA -Unite 'Mo'
                Verifier '(8) dix lectures exactes' $true | Out-Null
            }
            Arreter-Aide $script:temoin.Aide
            Point '(8) arrêt' { Fermer-Cas $script:proc $da '(8)' }
            Arreter-Tout

            # ===================== Cas B : historique ========================================
            $db = Dossier-Cas 'b'
            $imagesB = Join-Path $db 'images'
            New-Item -ItemType Directory -Force -Path $imagesB | Out-Null
            $recents = Join-Path $db 'local/Recents'
            $apercus = Join-Path $db 'local/Apercus'
            $glisser = Join-Path $db 'local/Glisser'
            Ecrire-Reglages -Dossier $db -Images $imagesB -Vignette $true
            Poser-Licence $db
            $script:proc = Demarrer-Cas -Dossier $db -Quoi '(B)' -Vignette
            $null = Attendre-Motif -Motif 'raccourci historique: Ctrl\+Maj\+H$' -Dossier $db -DelaiS 10 -Quoi '(B) raccourci historique'
            $null = Attendre-Motif -Motif 'raccourci texte: Ctrl\+Maj\+T$' -Dossier $db -DelaiS 10 -Quoi '(B) raccourci texte'
            Verifier '(B) licence.txt en place au démarrage : licence active ESSAI-0001' ((Compter-Motif -Motif 'licence active ESSAI-0001$' -Dossier $db) -eq 1) | Out-Null
            $script:hist = $null
            $script:c4 = $null
            $script:enregistree = $null

            # (9) Repos, trois captures.
            Point '(9) trois captures' {
                Verifier '(9) aucune fenêtre « Historique » au repos' ((($E::FenetresTitre($script:proc.Id, $TITRE_HISTORIQUE)).Count -eq 0) -and ((Compter-Motif -Motif 'historique fenetre' -Dossier $db) -eq 0)) | Out-Null
                for ($i = 1; $i -le 3; $i++) { $null = Capturer $db "(9) $i" }
                $limite = [DateTime]::UtcNow.AddSeconds(5)
                while (@(Fichiers-Format $apercus).Count -lt 3 -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
                $nr = @(Fichiers-Format $recents).Count; $na = @(Fichiers-Format $apercus).Count
                Verifier '(9) trois PNG dans Recents, trois dans Apercus' (($nr -eq 3) -and ($na -eq 3)) "Recents $nr, Apercus $na" | Out-Null
            }

            # (10) Ctrl+Maj+H.
            Point '(10) ouverture' {
                $o = Ouvrir-Historique $db '(10)'
                $script:hist = $o.H
                $h = $o.H
                Start-Sleep -Milliseconds 300
                $r = [BreezyShotEssai.Historique+RECT]::new()
                $null = $E::GetWindowRect($h, [ref]$r)
                $exStyle = $E::GetWindowLongPtr($h, -20).ToInt64()
                $style = $E::GetWindowLongPtr($h, -16).ToInt64()
                $z = Zone-Cliente $h
                $haut = $z.Y - $r.Top
                Verifier '(10) visible, titre « Historique »' ($E::IsWindowVisible($h) -and $E::Titre($h) -ceq $TITRE_HISTORIQUE) | Out-Null
                Verifier '(10) au premier plan' ($E::GetForegroundWindow() -eq $h) "premier plan $($E::Decrire($E::GetForegroundWindow()))" | Out-Null
                Verifier '(10) dans la barre des tâches (ni outil ni propriétaire)' ((($exStyle -band 0x80) -eq 0) -and $E::GetWindow($h, 4) -eq [IntPtr]::Zero) | Out-Null
                Verifier '(10) sans barre de titre Windows (zone cliente à 2 px au plus du haut), redimensionnable' (($haut -ge 0) -and ($haut -le 2) -and (($style -band 0x40000) -ne 0)) "haut $haut px, style 0x$('{0:X8}' -f $style)" | Out-Null
                $c = Attendre-Cases $h 3 '(10)'
                $noms = @($c | ForEach-Object { $_.Nom })
                Verifier '(10) liste « Captures récentes », trois cases « Capture, à l''instant »' (@($noms | Where-Object { $_ -ceq "Capture, à l'instant" }).Count -eq 3) ($noms -join ' | ') | Out-Null
                Noter-Mesure -Nom 'Ctrl+Maj+H -> historique affiche (budget 700)' -Valeur $o.Ms -Unite 'ms'
                # Budget du contrat : 700 ms (mesure notée) ; sur la machine GitHub, la première
                # fenêtre WebView2 compte dedans : échec seulement au-delà de 1 500 ms.
                Verifier "(10) historique affiche en $($o.Ms) ms (budget du contrat 700 ms ; échec au-delà de 1 500 ms sur cette machine)" ($o.Ms -le 1500) "$($o.Ms) ms" | Out-Null
                $p = Derniere-Ligne -Motif 'historique pret en \d+ ms$' -Dossier $db
                if ($p -match 'en (\d+) ms$') { Noter-Mesure -Nom 'Ctrl+Maj+H -> historique pret' -Valeur ([int]$Matches[1]) -Unite 'ms' }
            }
            if ($null -eq $script:hist) { throw "(10) pas d'historique : la suite du cas B n'est pas jouée ; $(Diagnostic-Journal $db)" }
            $hist = $script:hist

            # (11) Encore : ramenée devant ; une capture fenêtre ouverte.
            Point '(11) ramenée devant, capture fenêtre ouverte' {
                $devant = Compter-Motif -Motif 'historique ramene devant$' -Dossier $db
                $fen = Compter-Motif -Motif 'historique fenetre \d+$' -Dossier $db
                $E::AppuyerGlobal($MODS, $VK_H)
                $null = Attendre-Suivante 'historique ramene devant$' $db $devant '(11) Ctrl+Maj+H : ramene devant' 5
                $m = Ouvrir-Menu '(11)'
                Cliquer-Entree $m 'Historique' '(11)'
                $null = Attendre-Motif -Motif 'menu: historique$' -Dossier $db -DelaiS 5 -Quoi '(11) menu: historique'
                $null = Attendre-Suivante 'historique ramene devant$' $db ($devant + 1) '(11) menu Historique : ramene devant' 5
                Start-Sleep -Milliseconds 300
                Verifier '(11) ramene devant (raccourci et menu), une seule fenêtre' ((($E::FenetresTitre($script:proc.Id, $TITRE_HISTORIQUE)).Count -eq 1) -and ((Compter-Motif -Motif 'historique fenetre \d+$' -Dossier $db) -eq $fen)) | Out-Null
                $listes = Compter-Motif -Motif 'historique liste 4 entrees ' -Dossier $db
                $script:c4 = Capturer $db '(11) capture'
                $null = Attendre-Suivante 'historique liste 4 entrees ' $db $listes '(11) liste de quatre' 10
                $c = Attendre-Cases $hist 4 '(11)'
                Verifier '(11) quatre cases' ($c.Count -eq 4) | Out-Null
            }

            # (12) Copier et Enregistrer la deuxième case.
            Point '(12) Copier, Enregistrer' {
                Ramener-Devant $hist '(12)'
                $c = Attendre-Cases $hist 4 '(12)'
                $nom1 = (Fichiers-Format $recents)[1].Name
                $reference = Octets-Fichier (Join-Path $recents $nom1)
                $copies = Compter-Motif -Motif 'historique copie 1 \d+x\d+ ' -Dossier $db
                Cliquer-Action $c[1] 'Copier' '(12)'
                $l = Attendre-Suivante 'historique copie 1 \d+x\d+ ' $db $copies '(12) copie' 10
                Verifier '(12) historique copie 1 … faite' ($l -match 'historique copie 1 \d+x\d+ faite$') $l | Out-Null
                $p = Lire-PressePapiers
                $d = $E::Differences($p.O, $reference.O, $p.L, $p.H)
                Verifier '(12) presse-papiers = PNG de Recents au pixel près' ($d -eq 0) "$d pixel(s) différent(s), image $($p.L)x$($p.H), référence $($reference.L)x$($reference.H)" | Out-Null
                $avant = @(Fichiers-Png $imagesB).Count
                $enr = Compter-Motif -Motif 'historique enregistrement 1 \d+x\d+ ' -Dossier $db
                $c = Attendre-Cases $hist 4 '(12) enregistrer'
                Cliquer-Action $c[1] 'Enregistrer' '(12)'
                $l = Attendre-Suivante 'historique enregistrement 1 \d+x\d+ ' $db $enr '(12) enregistrement' 10
                $f = @(Fichiers-Png $imagesB)
                $nomAttendu = Nom-Enregistre $nom1
                $ok = ($l -match 'historique enregistrement 1 \d+x\d+ fait$') -and ($f.Count -eq $avant + 1) -and ($f[-1].Name -ceq $nomAttendu)
                Verifier '(12) un fichier, au nom de l''heure de la capture' $ok "$l ; $($f.Count) fichier(s) ; attendu « $nomAttendu »" | Out-Null
                if ($ok) {
                    $png = Octets-Fichier $f[-1].FullName
                    $d = $E::Differences($png.O, $reference.O, $png.L, $png.H)
                    Verifier '(12) fichier enregistré égal à la capture' ($d -eq 0) "$d pixel(s)" | Out-Null
                    $script:enregistree = [pscustomobject]@{ Fichier = $f[-1].FullName; Recent = $nom1 }
                }
                $c = Attendre-Cases $hist 4 '(12) encore'
                Cliquer-Action $c[1] 'Enregistrer' '(12)'
                $l = Attendre-Suivante 'historique enregistrement 1 \d+x\d+ ' $db ($enr + 1) '(12) second enregistrement' 10
                Start-Sleep -Milliseconds 500
                Verifier '(12) encore : deja fait, aucun second fichier' (($l -match 'historique enregistrement 1 \d+x\d+ deja fait$') -and (@(Fichiers-Png $imagesB).Count -eq $avant + 1)) $l | Out-Null
            }

            # (13) Supprimer, Annuler, Supprimer pour de bon.
            Point '(13) supprimer' {
                if ($null -eq $script:c4) { throw '(13) pas de capture de (11) : vignette inconnue' }
                Ramener-Devant $hist '(13)'
                $attentes = Compter-Motif -Motif 'historique suppression en attente ' -Dossier $db
                $c = Attendre-Cases $hist 4 '(13)'
                Touche-Case $c[0] $AUCUN $SUPPR
                $l = Attendre-Suivante 'historique suppression en attente (\d+)$' $db $attentes '(13) première suppression' 5
                $null = Attendre-Motif -Motif "vignette fermee $($script:c4.Id) historique$" -Dossier $db -DelaiS 5 -Quoi '(13) vignette fermee historique'
                Verifier '(13) case de tête (celle de la vignette), Suppr : vignette fermee <id> historique' ($l -match 'en attente 1$') $l | Out-Null
                $c = Attendre-Cases $hist 3 '(13) après la première'
                Touche-Case $c[0] $AUCUN $SUPPR
                $l = Attendre-Suivante 'historique suppression en attente (\d+)$' $db ($attentes + 1) '(13) seconde suppression' 5
                Verifier '(13) Suppr sur une autre : en attente 2' ($l -match 'en attente 2$') $l | Out-Null
                $null = Attendre-Cases $hist 2 '(13) après la seconde'
                $annulees = Compter-Motif -Motif 'historique suppression annulee ' -Dossier $db
                $E::Appuyer($CTRL, $VK_Z)
                $l = Attendre-Suivante 'historique suppression annulee (\d+)$' $db $annulees '(13) Ctrl+Z' 5
                $c = Attendre-Cases $hist 4 '(13) après Ctrl+Z'
                Start-Sleep -Milliseconds 300
                $nr = @(Fichiers-Format $recents).Count; $na = @(Fichiers-Format $apercus).Count
                Verifier '(13) Ctrl+Z : annulee 2, fichiers intacts' (($l -match 'annulee 2$') -and ($nr -eq 4) -and ($na -eq 4)) "$l ; Recents $nr, Apercus $na" | Out-Null
                # La case enregistrée en (12) (rang 1) part pour de bon, 5 s après.
                $nom1 = (Fichiers-Format $recents)[1].Name
                $faites = Compter-Motif -Motif 'historique suppression faite ' -Dossier $db
                Touche-Case $c[1] $AUCUN $SUPPR
                $la = Attendre-Suivante 'historique suppression en attente (\d+)$' $db ($attentes + 2) '(13) suppression' 5
                $lf = Attendre-Suivante 'historique suppression faite (\d+) echecs (\d+)$' $db $faites '(13) suppression faite' 10
                $s = ((Heure-Ligne $lf) - (Heure-Ligne $la)).TotalSeconds
                Verifier '(13) suppression faite 1 echecs 0, 5 s après' (($lf -match 'suppression faite 1 echecs 0$') -and ($s -ge 4.9) -and ($s -le 6.5)) "$lf ; $([math]::Round($s, 2)) s" | Out-Null
                $parti = (-not (Test-Path -LiteralPath (Join-Path $recents $nom1))) -and (-not (Test-Path -LiteralPath (Join-Path $apercus $nom1)))
                $reste = ($null -ne $script:enregistree) -and ($script:enregistree.Recent -ceq $nom1) -and (Test-Path -LiteralPath $script:enregistree.Fichier)
                Verifier '(13) PNG et aperçu partis, fichier enregistré resté' ($parti -and $reste) "parti $parti, enregistré resté $reste" | Out-Null
            }

            # (14) Annoter ; Copier le texte dans l'éditeur.
            Point '(14) Annoter depuis une case' {
                Ramener-Devant $hist '(14)'
                $c = Attendre-Cases $hist 3 '(14)'
                $actions = Compter-Motif -Motif 'historique action 0 \d+x\d+ annoter$' -Dossier $db
                $ouverts = Compter-Motif -Motif 'editeur ouvert \d+ ' -Dossier $db
                Touche-Case $c[0] $AUCUN $ENTREE
                $la = Attendre-Suivante 'historique action 0 (\d+x\d+) annoter$' $db $actions '(14) action annoter' 5
                $lo = Attendre-Suivante 'editeur ouvert (\d+) (\d+x\d+) ' $db $ouverts '(14) editeur ouvert' 20
                $taille = [regex]::Match($la, 'action 0 (\d+x\d+) annoter$').Groups[1].Value
                $m = [regex]::Match($lo, 'editeur ouvert (\d+) (\d+x\d+) ')
                Verifier '(14) Entrée : editeur ouvert à la taille de la case' ($m.Groups[2].Value -eq $taille) "$la ; $lo" | Out-Null
                $id = [int]$m.Groups[1].Value
                $null = Attendre-Motif -Motif "editeur image affichee $id en \d+ ms$" -Dossier $db -DelaiS 20 -Quoi '(14) image affichée'
                $h = @($E::FenetresTitre($script:proc.Id, $TITRE_EDITEUR))
                if ($h.Count -ne 1) { throw "(14) $($h.Count) fenêtre(s) d'éditeur" }
                Fermer-Editeur $h[0] $db $id '(14)'
            }
            Point '(14) Texte dans l''éditeur' {
                $t = Lancer-Temoin 'b' ([int]($utile.X + 20)) ([int]($utile.Y + 20))
                try {
                    $zt = Zone-Lignes $t
                    $entrees = Compter-Motif -Motif 'vignette entree \d+$' -Dossier $db
                    $gels = Compter-Motif -Motif '-> Gel \(DemandeCapture zone\)$' -Dossier $db
                    $pretes = Compter-Motif -Motif 'chrono selection prete \d+' -Dossier $db
                    $E::Poser($REPOS_X, $REPOS_Y)
                    $E::AppuyerGlobal($MODS, $VK_1)
                    $null = Attendre-Suivante '-> Gel \(DemandeCapture zone\)$' $db $gels '(14) capture de zone' 5
                    $null = Attendre-Suivante 'chrono selection prete \d+' $db $pretes '(14) sélection prête' 5
                    Start-Sleep -Milliseconds 150
                    $E::Tracer($zt.X0, $zt.Y0, $zt.X1, $zt.Y1)
                    $E::Poser($REPOS_X, $REPOS_Y)
                    $l = Attendre-Suivante 'vignette entree (\d+)$' $db $entrees '(14) vignette de la zone' 15
                    $vid = [int]([regex]::Match($l, 'vignette entree (\d+)$').Groups[1].Value)
                    $carte = Lire-Rect (Attendre-Motif -Motif "vignette zone $vid " -Dossier $db -DelaiS 3 -Quoi '(14) zone de la carte')
                }
                finally { Arreter-Aide $t.Aide }   # toujours au-dessus : il couvrirait l'éditeur
                Start-Sleep -Milliseconds 350
                $h = Ouvrir-Editeur $db ([pscustomobject]@{ Id = $vid; Carte = $carte }) '(14)'
                $lo = Derniere-Ligne -Motif "editeur ouvert $vid (\d+)x(\d+) " -Dossier $db
                $m = [regex]::Match($lo, "editeur ouvert $vid (\d+)x(\d+) ")
                $largeur = [int]$m.Groups[1].Value
                Verifier '(14) éditeur de la zone du témoin' ($largeur -eq ($zt.X1 - $zt.X0 + 1)) "$lo ; zone $(Texte-Zone $zt)" | Out-Null
                Ramener-Devant $h '(14)' 26
                # « Texte », puis Espace, Maj+Droite × 2, Entrée : les trois premiers mots.
                $b = Bouton-Texte $h
                if ($null -eq $b) { throw "(14) bouton « Texte » introuvable dans l'éditeur ; $(Elements-Vus $h)" }
                $lus = Compter-Motif -Motif "editeur texte $vid \d+ (lu|vide|echec)" -Dossier $db
                Cliquer-Element $b
                $E::Poser($REPOS_X, $REPOS_Y)
                $l = Attendre-Suivante "editeur texte $vid \d+ (lu|vide|echec)" $db $lus '(14) lecture de l''éditeur' 15
                Verifier '(14) « Texte » : editeur texte … lu 3 lignes 11 mots' ($l -match "editeur texte $vid \d+ lu 3 lignes 11 mots en \d+ ms$") $l | Out-Null
                $ex = Derniere-Ligne -Motif "editeur export $vid \d+ texte version " -Dossier $db
                Verifier '(14) export de but texte' ($null -ne $ex) | Out-Null
                if ($l -match 'en (\d+) ms$') { Noter-Mesure -Nom 'éditeur : lecture de la zone du témoin' -Valeur ([int]$Matches[1]) -Unite 'ms' }
                $null = Element-Exige $h 'Choisir le texte' $null '(14) calque de choix' 5
                # La barre flottante (« Tout copier ») n'existe qu'une fois le balayage fini :
                # avant, le calque avale les touches.
                $null = Element-Exige $h 'Tout copier' $CT::Button '(14) barre du choix' 5
                Start-Sleep -Milliseconds 300
                $copies = Compter-Motif -Motif "editeur texte copie $vid \d+ " -Dossier $db
                $E::Touche($ESPACE)
                $E::Appuyer($MAJ, $DROITE)
                $E::Appuyer($MAJ, $DROITE)
                $E::Touche($ENTREE)
                $l = Attendre-Suivante "editeur texte copie $vid \d+ " $db $copies '(14) copie du choix' 10
                $pp = Lire-Texte
                Verifier '(14) Espace, Maj+Droite × 2, Entrée : presse-papiers = « BreezyShot reads this »' (($l -match ' 3 mots$') -and ($pp -ceq 'BreezyShot reads this')) "$l ; lu « $(Montrer $pp) »" | Out-Null
                # Bloc noir sur la deuxième ligne (outil Flou, variante Bloc noir), puis tout.
                $v = Ajuster $db $vid $largeur
                Cliquer-Nom $h 'Ajouter' $CT::Button '(14) Ajouter'
                Cliquer-Nom $h 'Flou' $CT::MenuItem '(14) Ajouter › Flou'
                Cliquer-Nom $h 'Flou' $CT::Button '(14) outil Flou'
                Cliquer-Nom $h 'Bloc noir' $CT::Button '(14) variante Bloc noir'
                $E::Touche($ECHAP)
                Start-Sleep -Milliseconds 300
                $r2 = $t.Lignes[1]
                $x0 = $r2.X - $zt.X0 - 4; $y0 = $r2.Y - $zt.Y0 - 3
                $x1 = $r2.X + $r2.L - $zt.X0 + 4; $y1 = $r2.Y + $r2.H - $zt.Y0 + 3
                $E::Tracer((Ecran-X $v $x0), (Ecran-Y $v $y0), (Ecran-X $v $x1), (Ecran-Y $v $y1))
                $E::Poser($REPOS_X, $REPOS_Y)
                Start-Sleep -Milliseconds 500
                $b = Bouton-Texte $h
                $lus = Compter-Motif -Motif "editeur texte $vid \d+ (lu|vide|echec)" -Dossier $db
                Cliquer-Element $b
                $E::Poser($REPOS_X, $REPOS_Y)
                $l = Attendre-Suivante "editeur texte $vid \d+ (lu|vide|echec)" $db $lus '(14) lecture après le Bloc noir' 15
                Verifier '(14) Bloc noir sur la deuxième ligne : lu 2 lignes 5 mots' ($l -match ' lu 2 lignes 5 mots en \d+ ms$') $l | Out-Null
                $copies = Compter-Motif -Motif "editeur texte copie $vid \d+ " -Dossier $db
                Cliquer-Nom $h 'Tout copier' $CT::Button '(14) Tout copier'
                $l = Attendre-Suivante "editeur texte copie $vid \d+ " $db $copies '(14) Tout copier' 10
                $pp = Lire-Texte
                $attendu = "$($LIGNES_TEMOIN[0])`r`n$($LIGNES_TEMOIN[2])"
                Verifier '(14) « Tout copier » : deux lignes, sans la deuxième (R7)' (($l -match ' 5 mots$') -and ($pp -ceq $attendu)) "$l ; lu « $(Montrer $pp) »" | Out-Null
                Fermer-Editeur $h $db $vid '(14) fermeture'
            }

            # (15) Glisser une case vers une autre application.
            Point '(15) glisser' {
                $fh = Join-Path $script:Essai.Dossier 'cible-13.txt'
                $fd = Join-Path $script:Essai.Dossier 'depots-13.txt'
                $cible = Lancer-Aide 'cible-glisser.ps1' @('-X', "$([int]($utile.Right - 124))", '-Y', "$([int]($utile.Y + 300))", '-Cote', '120', '-FichierHwnd', $fh, '-FichierDepots', $fd) $fh
                try {
                    Ramener-Devant $hist '(15)'
                    $c = @(Cases $hist)
                    if ($c.Count -lt 1) { throw '(15) aucune case' }
                    $source = Join-Path $recents (Fichiers-Format $recents)[0].Name
                    $p = Centre-Stable $c[0].Case
                    $debuts = Compter-Motif -Motif 'historique glisser 0 \d+x\d+ debut$' -Dossier $db
                    $E::Poser($p[0], $p[1]); Start-Sleep -Milliseconds 100
                    $E::BoutonBas()
                    $E::Deplacer($p[0], $p[1], $p[0] + 12, $p[1] + 6, 6, 20)
                    try {
                        $null = Attendre-Suivante 'historique glisser 0 \d+x\d+ debut$' $db $debuts '(15) glisser debut' 10
                        $E::Deplacer($p[0] + 12, $p[1] + 6, [int]($utile.Right - 64), [int]($utile.Y + 360), 25, 20)
                        Start-Sleep -Milliseconds 300
                    }
                    finally { $E::BoutonHaut() }
                    $l = Attendre-Motif -Motif 'historique glisser 0 \d+x\d+ (depose|annule|refuse|indisponible)' -Dossier $db -DelaiS 10 -Quoi '(15) fin du glisser'
                    Verifier '(15) historique glisser … depose' ($l -match 'depose$') $l | Out-Null
                    $limite = [DateTime]::UtcNow.AddSeconds(5)
                    while (-not (Test-Path -LiteralPath $fd) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
                    $recus = @(if (Test-Path -LiteralPath $fd) { Get-Content -LiteralPath $fd | Where-Object { $_ -ne '' } })
                    $pareil = ($recus.Count -eq 1) -and [System.Linq.Enumerable]::SequenceEqual([System.IO.File]::ReadAllBytes($recus[0]), [System.IO.File]::ReadAllBytes($source))
                    Verifier '(15) un fichier reçu, mêmes octets que Recents' $pareil "$($recus.Count) reçu(s)" | Out-Null
                }
                finally { Arreter-Aide $cible; $E::Poser($REPOS_X, $REPOS_Y) }
            }

            # (16) Tout effacer.
            Point '(16) tout effacer' {
                Ramener-Devant $hist '(16)'
                Cliquer-Nom $hist "Plus d'options" $CT::Button '(16) Plus d''options'
                Cliquer-Nom $hist 'Tout effacer' $CT::MenuItem '(16) Tout effacer'
                $null = Element-Exige $hist 'Tout effacer ?' $null '(16) confirmation' 3
                Cliquer-Nom $hist 'Effacer' $CT::Button '(16) Effacer'
                $l = Attendre-Motif -Motif 'historique tout efface \d+ echecs \d+$' -Dossier $db -DelaiS 10 -Quoi '(16) tout efface'
                $nr = @(Fichiers-Format $recents).Count; $na = @(Fichiers-Format $apercus).Count; $ng = @(Fichiers-Png $glisser).Count
                Verifier '(16) Recents, Apercus et Glisser vides de leurs formats' (($l -match 'echecs 0$') -and ($nr -eq 0) -and ($na -eq 0) -and ($ng -eq 0)) "$l ; Recents $nr, Apercus $na, Glisser $ng" | Out-Null
                $vide = Element $hist 'Aucune capture' $CT::Image 3
                $plus = Element $hist "Plus d'options" $CT::Button 1
                Start-Sleep -Milliseconds 300
                Verifier '(16) liste vide, « Aucune capture », « Plus d''options » grisé' (($null -ne $vide) -and (@(Cases $hist).Count -eq 0) -and ($null -ne $plus) -and (-not $plus.Current.IsEnabled)) | Out-Null
                Verifier '(16) fichier enregistré resté' (($null -ne $script:enregistree) -and (Test-Path -LiteralPath $script:enregistree.Fichier)) | Out-Null
            }

            # (17) Fermer : bouton, WM_CLOSE, arrêt avec une suppression en attente.
            Point '(17) fermetures' {
                Ramener-Devant $hist '(17)'
                Cliquer-Nom $hist 'Fermer' $CT::Button '(17) Fermer'
                $null = Attendre-Motif -Motif 'historique ferme \(page\)$' -Dossier $db -DelaiS 10 -Quoi '(17) ferme (page)'
                $limite = [DateTime]::UtcNow.AddSeconds(5)
                while ($E::IsWindow($hist) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
                Verifier '(17) bouton Fermer : ferme (page), fenêtre détruite' (-not $E::IsWindow($hist)) | Out-Null
                $o = Ouvrir-Historique $db '(17) cadre'
                Fermer-Historique $o.H $db '(17)'
                Verifier '(17) WM_CLOSE : ferme (cadre), fenêtre détruite' $true | Out-Null
                $null = Capturer $db '(17) capture'
                $o = Ouvrir-Historique $db '(17) arrêt'
                $c = Attendre-Cases $o.H 1 '(17)'
                Ramener-Devant $o.H '(17)'
                $nom = (Fichiers-Format $recents)[0].Name
                # Compté avec le motif attendu : (13) a déjà écrit des « en attente 2 ».
                $attentes = Compter-Motif -Motif 'historique suppression en attente 1$' -Dossier $db
                Touche-Case $c[0] $AUCUN $SUPPR
                $null = Attendre-Suivante 'historique suppression en attente 1$' $db $attentes '(17) suppression en attente' 5
                Fermer-Cas $script:proc $db '(17)'
                $lf = Derniere-Ligne -Motif 'historique suppression faite ' -Dossier $db
                $ok = ((Compter-Motif -Motif 'historique ferme \(arret\)$' -Dossier $db) -eq 1) -and ($lf -match 'suppression faite 1 echecs 0$') -and (-not (Test-Path -LiteralPath (Join-Path $recents $nom)))
                Verifier '(17) --quitter : suppression faite, ferme (arret), arret propre' $ok "$lf ; $(Diagnostic-Journal $db)" | Out-Null
            }
            Arreter-Tout

            # (18) Mémoire : 20 ouvertures et fermetures.
            $script:proc = Demarrer-Cas -Dossier $db -Quoi '(18)' -Vignette
            $null = Attendre-Motif -Motif 'raccourci historique: Ctrl\+Maj\+H$' -Dossier $db -DelaiS 10 -Quoi '(18) raccourci historique' -Fois 2
            Point '(18) mémoire et stabilité' {
                for ($i = 1; $i -le 3; $i++) { $null = Capturer $db "(18) capture $i" }
                Start-Sleep -Seconds 3
                $froid = Memoire-Privee-Mo -Processus $script:proc
                $d0 = Detail-Memoire $script:proc
                Noter-Mesure -Nom 'memoire privee au repos (cas B)' -Valeur $froid -Unite 'Mo'
                Noter-Mesure -Nom 'detail au repos (cas B)' -Valeur $d0.Texte -Unite 'Mo'
                $chaud = $null; $fils = $null; $poigneesAvant = $null; $ouvert = $null; $dOuvert = $null; $d1 = $null
                $temps = [System.Collections.Generic.List[int]]::new()
                for ($i = 1; $i -le 20; $i++) {
                    $o = Ouvrir-Historique $db "(18) $i"
                    $temps.Add($o.Ms)
                    if ($i -eq 1) {
                        Start-Sleep -Seconds 2
                        $ouvert = Memoire-Privee-Mo -Processus $script:proc
                        $dOuvert = Detail-Memoire $script:proc
                        Noter-Mesure -Nom 'memoire privee historique ouvert' -Valeur $ouvert -Unite 'Mo'
                        Noter-Mesure -Nom 'detail historique ouvert' -Valeur $dOuvert.Texte -Unite 'Mo'
                    }
                    Fermer-Historique $o.H $db "(18) $i"
                    if ($i -eq 1) {
                        Start-Sleep -Seconds 10
                        $chaud = Memoire-Privee-Mo -Processus $script:proc
                        $d1 = Detail-Memoire $script:proc
                        $script:proc.Refresh()
                        $fils = $script:proc.Threads.Count
                        $poigneesAvant = (Get-Process -Id $script:proc.Id).HandleCount
                        Noter-Mesure -Nom 'memoire privee apres une fermeture (repos chaud)' -Valeur $chaud -Unite 'Mo'
                        Noter-Mesure -Nom 'detail apres une fermeture' -Valeur $d1.Texte -Unite 'Mo'
                    }
                }
                Start-Sleep -Seconds 10
                $apres = Memoire-Privee-Mo -Processus $script:proc
                $d20 = Detail-Memoire $script:proc
                Noter-Mesure -Nom 'Ctrl+Maj+H -> historique affiche, 20 ouvertures (min / max)' -Valeur "$(($temps | Measure-Object -Minimum).Minimum) / $(($temps | Measure-Object -Maximum).Maximum)" -Unite 'ms'
                Noter-Mesure -Nom 'memoire privee 10 s apres 20 fermetures' -Valeur $apres -Unite 'Mo'
                Noter-Mesure -Nom 'detail 10 s apres 20 fermetures' -Valeur $d20.Texte -Unite 'Mo'
                Verifier '(18) historique ouvert ≤ repos + 70 Mo' ($ouvert -le $froid + 70) "$ouvert Mo, repos $froid Mo" | Out-Null
                $rendus = "repos $($d0.Rendus), ouvert $($dOuvert.Rendus), après 1 fermeture $($d1.Rendus), après 20 $($d20.Rendus)"
                Verifier '(18) le processus de rendu de l''historique part à la fermeture' (($d0.Rendus -ge 1) -and ($dOuvert.Rendus -gt $d0.Rendus) -and ($d1.Rendus -eq $d0.Rendus) -and ($d20.Rendus -eq $d0.Rendus)) $rendus | Out-Null
                Verifier '(18) processus de l''app : après 20 fermetures ≤ après la première + 8 Mo' (($d1.App -gt 0) -and ($d20.App -le $d1.App + 8)) "$($d20.App) Mo, après la première $($d1.App) Mo" | Out-Null
                Verifier '(18) navigateur WebView2 : après 20 fermetures ≤ après la première + 6 Mo' (($d1.Navigateur -gt 0) -and ($d20.Navigateur -le $d1.Navigateur + 6)) "$($d20.Navigateur) Mo, après la première $($d1.Navigateur) Mo" | Out-Null
                Verifier '(18) GPU WebView2 : après 20 fermetures ≤ après la première + 6 Mo' (($d1.Gpu -gt 0) -and ($d20.Gpu -le $d1.Gpu + 6)) "$($d20.Gpu) Mo, après la première $($d1.Gpu) Mo" | Out-Null
                $script:proc.Refresh()
                $filsApres = $script:proc.Threads.Count
                $poignees = (Get-Process -Id $script:proc.Id).HandleCount
                Noter-Mesure -Nom 'fils apres 20 fermetures' -Valeur "$fils -> $filsApres" -Unite ''
                Noter-Mesure -Nom 'poignees apres 20 fermetures (contrat : + 1 par ouverture au plus)' -Valeur "$poigneesAvant -> $poignees" -Unite ''
                Verifier '(18) fils et poignées stables' (($filsApres -le $fils + 3) -and ($poignees -le $poigneesAvant + 60)) "fils $fils -> $filsApres, poignées $poigneesAvant -> $poignees" | Out-Null
                Verifier '(18) aucune fenêtre « Historique » restante' (($E::FenetresTitre($script:proc.Id, $TITRE_HISTORIQUE)).Count -eq 0) | Out-Null
            }

            # (L4) Retirer la licence, pour de vrai.
            Point '(L4) licence retirée' {
                $fen = Ouvrir-Reglages $db '(L4)'
                Cliquer-Nom $fen 'Général' $CT::TabItem '(L4) rubrique Général'
                $rendus = Compter-Motif -Motif 'raccourci (historique|texte) rendu$' -Dossier $db
                Cliquer-Nom $fen 'Retirer' $CT::Button '(L4) Retirer'
                $null = Attendre-Motif -Motif 'licence retiree$' -Dossier $db -DelaiS 10 -Quoi '(L4) licence retiree'
                $null = Attendre-Suivante 'raccourci (historique|texte) rendu$' $db ($rendus + 1) '(L4) raccourcis rendus' 10
                Verifier '(L4) raccourci historique rendu, raccourci texte rendu' (((Compter-Motif -Motif 'raccourci historique rendu$' -Dossier $db) -eq 1) -and ((Compter-Motif -Motif 'raccourci texte rendu$' -Dossier $db) -eq 1)) | Out-Null
                $champ = Element $fen 'Licence' $CT::Edit 5
                Verifier '(L4) la ligne redevient un champ « Licence »' ($null -ne $champ) | Out-Null
                # Chantier L : effacé, sinon remplacé par un fichier vide (qui vaut licence absente).
                $cl = Chemin-Licence $db
                $reste = if (Test-Path -LiteralPath $cl) { [System.IO.File]::ReadAllText($cl) } else { '' }
                Verifier '(L4) licence.txt effacé ou vidé' ($reste.Trim() -eq '') "$(if (Test-Path -LiteralPath $cl) { "présent, $($reste.Length) caractères" } else { 'absent' })" | Out-Null
                Fermer-Reglages $fen $db '(L4)'
                $m = Ouvrir-Menu '(L4)'
                $pro = @($m.Textes | Where-Object { $_.StartsWith('Historique', [StringComparison]::Ordinal) -or $_.StartsWith('Copier le texte', [StringComparison]::Ordinal) })
                Verifier '(L4) entrées « Historique » et « Copier le texte » parties du menu' (($pro.Count -eq 0) -and ($m.Textes.Count -eq 9)) $m.Lus | Out-Null
                Fermer-Menu $m '(L4)'
                Verifier '(L4) Ctrl+Maj+H rendu à Windows' ((Tenir-Ailleurs $VK_H 'h-retiree') -eq 'tenu') | Out-Null
                Fermer-Cas $script:proc $db '(L4)'
                $languesAvant = Compter-Motif -Motif 'texte (langues|non pris en charge)' -Dossier $db
                $script:proc = Demarrer-Cas -Dossier $db -Quoi '(L4) redémarrage' -Vignette
                Verifier '(L4) redémarrage : licence absente' ((Compter-Motif -Motif 'licence absente$' -Dossier $db) -eq 1) | Out-Null
                Start-Sleep -Seconds 2
                Verifier '(L4) redémarrage sans licence : aucune sonde des langues' ((Compter-Motif -Motif 'texte (langues|non pris en charge)' -Dossier $db) -eq $languesAvant) "$languesAvant avant, $(Compter-Motif -Motif 'texte (langues|non pris en charge)' -Dossier $db) après" | Out-Null
                Fermer-Cas $script:proc $db '(L4) arrêt'
            }
            Arreter-Tout

            # ===================== Cas C : migration depuis la 0.4 ============================
            Point '(C) migration' {
                $dc = Dossier-Cas 'c'
                $rc = Join-Path $dc 'local/Recents'
                New-Item -ItemType Directory -Force -Path $rc | Out-Null
                $racc = [ordered]@{ zone = @('Ctrl+Maj+T'); plein_ecran = @('Ctrl+Maj+2'); fenetre = @(); derniere_zone = @() }
                Ecrire-Reglages -Dossier $dc -Images (Join-Path $dc 'images') -Raccourcis $racc
                $teintes = @(@(220, 40, 40), @(40, 160, 60), @(40, 80, 200))
                for ($k = 0; $k -lt 3; $k++) {
                    $nom = [DateTime]::UtcNow.AddMinutes(-10 * ($k + 1)).ToString('yyyyMMdd-HHmmss-fff', [Globalization.CultureInfo]::InvariantCulture) + '.png'
                    Ecrire-Png (Join-Path $rc $nom) 640 400 ([byte[]]@($teintes[$k][0], $teintes[$k][1], $teintes[$k][2], 255))
                }
                Poser-Licence $dc
                $script:proc = Demarrer-Cas -Dossier $dc -Quoi '(C)'
                $null = Attendre-Motif -Motif 'licence active ESSAI-0001$' -Dossier $dc -DelaiS 5 -Quoi '(C) licence active'
                Verifier '(C) démarrage avec licence.txt en place : licence active ESSAI-0001' $true | Out-Null
                $lt = Attendre-Motif -Motif 'raccourci texte: ' -Dossier $dc -DelaiS 10 -Quoi '(C) raccourci texte'
                $f = Lire-Reglages $dc
                $texte = @($f.raccourcis.texte)
                $histo = @($f.raccourcis.historique)
                Verifier '(C) fichier passé en v5 : historique Ctrl+Maj+H, texte vide (Ctrl+Maj+T déjà donné)' (($f.version -eq 5) -and ($texte.Count -eq 0) -and ($histo.Count -eq 1) -and ($histo[0] -eq 'Ctrl+Maj+H') -and ((Compter-Motif -Motif 'reglages mis a niveau de la version 3 a la version 5$' -Dossier $dc) -eq 1)) "version $($f.version), texte [$($texte -join ',')], historique [$($histo -join ',')]" | Out-Null
                Verifier '(C) raccourci texte: aucun' ($lt -match 'raccourci texte: aucun$') $lt | Out-Null
                $o = Ouvrir-Historique $dc '(C)'
                $c = Attendre-Cases $o.H 3 '(C)'
                $l = Attendre-Motif -Motif 'historique apercus faits \d+ en \d+ ms$' -Dossier $dc -DelaiS 10 -Quoi '(C) aperçus faits'
                Verifier '(C) trois cases, historique apercus faits 3' (($c.Count -eq 3) -and ($l -match 'apercus faits 3 en (\d+) ms$')) $l | Out-Null
                if ($l -match 'en (\d+) ms$') { Noter-Mesure -Nom 'trois aperçus manquants faits' -Valeur ([int]$Matches[1]) -Unite 'ms' }
                Verifier '(C) trois aperçus rangés' (@(Fichiers-Format (Join-Path $dc 'local/Apercus')).Count -eq 3) | Out-Null
                Fermer-Historique $o.H $dc '(C)'
                Fermer-Cas $script:proc $dc '(C)'
            }
            Arreter-Tout

            # ===================== Cas D : sans langue de lecture ==============================
            Point '(D) sans langue' {
                $dd = Dossier-Cas 'd'
                Ecrire-Reglages -Dossier $dd -Images (Join-Path $dd 'images') -Vignette $true
                Poser-Licence $dd
                $script:proc = Demarrer-Cas -Dossier $dd -Quoi '(D)' -Vignette -SansTexte
                $null = Attendre-Motif -Motif 'raccourci historique: ' -Dossier $dd -DelaiS 10 -Quoi '(D) raccourci historique'
                Verifier '(D) texte langues aucune (essai)' ((Compter-Motif -Motif 'texte langues aucune \(essai\)$' -Dossier $dd) -eq 1) (Derniere-Ligne -Motif 'texte (langues|non pris)' -Dossier $dd) | Out-Null
                Start-Sleep -Milliseconds 500
                Verifier '(D) aucune ligne raccourci texte' ((Compter-Motif -Motif 'raccourci texte[: ]' -Dossier $dd) -eq 0) | Out-Null
                $m = Ouvrir-Menu '(D)'
                $sansTexte = @($m.Textes | Where-Object { $_.StartsWith('Copier le texte', [StringComparison]::Ordinal) }).Count -eq 0
                $avecHisto = @($m.Textes | Where-Object { $_ -ceq "Historique`tCtrl + Maj + H" }).Count -eq 1
                Verifier '(D) menu sans « Copier le texte », avec « Historique »' ($sansTexte -and $avecHisto) $m.Lus | Out-Null
                Fermer-Menu $m '(D)'
                Verifier '(D) Ctrl+Maj+T libre (l''essai le tient)' ((Tenir-Ailleurs $VK_T 't-sans-langue') -eq 'tenu') | Out-Null
                $fen = Ouvrir-Reglages $dd '(D)'
                Cliquer-Nom $fen 'Raccourcis' $CT::TabItem '(D) rubrique Raccourcis'
                $histo = Element $fen 'Historique' $CT::Button 5
                $texte = Element $fen 'Copier le texte' $CT::Button 1
                Verifier '(D) Réglages › Raccourcis : ligne « Historique », sans « Copier le texte »' (($null -ne $histo) -and ($null -eq $texte)) $(if ($null -eq $histo -or $null -ne $texte) { Elements-Vus $fen } else { '' }) | Out-Null
                Fermer-Reglages $fen $dd '(D)'
                $c = Capturer $dd '(D) capture'
                $h = Ouvrir-Editeur $dd $c '(D)'
                $enregistrer = Element $h 'Enregistrer' $CT::Button 5
                $b = Bouton-Texte $h 1
                Verifier '(D) éditeur sans « Texte » (barre lue : « Enregistrer » trouvé)' (($null -ne $enregistrer) -and ($null -eq $b)) | Out-Null
                Fermer-Editeur $h $dd $c.Id '(D)'
                Fermer-Cas $script:proc $dd '(D)'
            }
        }
        finally {
            try { $E::BoutonHaut() } catch { }
        }
    }
}
catch {
    Verifier 'essai sans erreur inattendue' $false "$_" | Out-Null
}
finally {
    foreach ($a in $script:Auxiliaires) { try { if (-not $a.HasExited) { $a.Kill() } } catch { } }
    Terminer-Essai
}
