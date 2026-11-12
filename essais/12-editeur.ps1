# Version 0.4 « Éditeur d'annotation », chantier Cœur : l'éditeur de bout en bout sur le
# vrai .exe, avec de vraies entrées clavier et souris (contrat 0.4 « Éditeur », §12.3), aux
# noms et aux gestes de l'avenant « Éditeur épuré » (§3, §4 : rôles et noms accessibles).
# Réglages : plein écran sur Ctrl+Maj+2, copie, pas d'enregistrement automatique, vignette
# « toujours » en bas à gauche, animations réduites, dossier d'enregistrement de l'essai.
# BREEZYSHOT_ESSAI_CAPTURABLE=1. Presse-papiers lu dans un fil STA. Référence : le PNG de
# la capture dans « Récents ». Chaque attente lit une ligne nouvelle du journal (ou guette
# une fenêtre) avec une borne ; jamais un délai fixe à la place d'une preuve.
# Garde : hors le raccourci global de capture, une touche ne part que si la fenêtre au
# premier plan est à l'app, un appui de souris que si le point visé est sur une fenêtre de
# l'app ; sinon le point échoue en nommant la fenêtre trouvée, et rien n'est envoyé (un
# Ctrl+C ou un Alt+F4 reçu par une console de la machine d'essai l'arrêterait). Sans
# éditeur après (2), la suite du cas A n'est pas jouée.
#
# Cas A :
# (1) mémoire au repos notée ; aucune fenêtre « Annoter la capture », aucun `editeur
#     fenetre` ;
# (2) Ctrl + Maj + 2 : `vignette preparee … actions copier,enregistrer,annoter,supprimer,
#     fermer,annuler` ; clic réel au centre de `vignette bouton <id> annoter` : `vignette
#     fermee <id> annoter`, `etat Vignette -> Repos (VignetteFermee)`, `editeur fenetre` ;
# (3) fenêtre visible, titre exact, au premier plan, dans la barre des tâches (ni outil ni
#     propriétaire), zone cliente dans la zone utile ; `editeur image affichee` noté (budget
#     800 ms) ; sans barre de titre mais redimensionnable : tao garde WS_CAPTION et
#     WS_SIZEBOX dans le style et rend toute la fenêtre à la zone cliente par WM_NCCALCSIZE,
#     donc on vérifie WS_SIZEBOX et une zone cliente qui commence à 2 px au plus du haut de
#     la fenêtre (`round(dpi / 96)` sous Windows 11 avec l'ombre, 1 px à 100 % ; au lieu
#     d'une barre de titre de ~30 px) ; zone cliente exactement au rectangle `fenetre x,y
#     lxh` de `editeur ouvert` (tao pose la taille et la position en pixels physiques
#     entiers, la taille cliente demandée plus les bords mesurés : aucun arrondi, 0 px) ;
# (4) Ctrl+1 : une ligne `editeur vue … zoom 1.00` attendue, 1024x768 (un pixel pour un
#     pixel) ; Ctrl+0 :
#     Ajuster, rectangle de l'image lu (la capture plein écran ne tient pas à la taille
#     réelle dans sa fenêtre : les gestes se font en Ajuster, convertis par ce rectangle) ;
# (5) R, glisser réel de (100, 100) à (300, 200) de l'image, Ctrl+C : `copie faite recent ok
#     fichier non demande` ; hors de la boîte (+ 12 px, + 3 px de conversion) égal à la
#     référence au bit près ; bord haut à #E5484D ± 6 ; touche → bilan noté (budget 400 ms) ;
# (6) B sur la frontière de deux carrés du témoin, Ctrl+C : blocs de 13 px unis, alignés
#     sur la grille, au moins deux couleurs ;
# (7) Ctrl+Z × 2, Ctrl+C : égal à la référence ; Ctrl+Y × 2, Ctrl+C : égal à l'export du 6 ;
# (8) Ctrl+S : un fichier au modèle, PNG égal au presse-papiers ; encore : `deja fait`, aucun
#     second fichier ; « Récents » : un fichier par `recent ok` ;
# (9) WM_CLOSE sans changement depuis l'export : `ferme <id> (propre)`, `pixels rendus`,
#     fenêtre détruite ;
# (10) capture, Annoter, un rectangle, Alt+F4 : `fermeture <id>: demander`, le `menu`
#     « Fermer » et le focus sur le `menuitem` « Copier et fermer » (UI Automation, bornés),
#     la fenêtre reste après le filet de 2 s ; Échap sur le menu (« Continuer ») : rien ;
#     Alt+F4, menu attendu, clic réel sur « Fermer sans copier » : `(sans garder)` ;
# (11) éditeur propre puis Annoter d'une autre capture : `remplace <a> par <b>`, une seule
#     fenêtre ; modifié puis Annoter : `fermeture demandee <b> (remplacer)`, menu « Fermer »
#     et focus attendus, Échap (« Continuer ») : `capture <c> abandonnee`, le document b
#     reste ;
# (12) Ctrl + Maj + 2 éditeur ouvert : capture faite, éditeur ouvert, même fenêtre, aucun
#     `editeur ferme` ;
# (13) Ctrl+Maj+S : dialogue #32770 possédé par l'éditeur (guetté, 30 s au plus), annulé
#     (IDCANCEL) : `dialogue annule`, rien d'écrit ; « Plus », `menuitem` « Glisser vers
#     une appli », puis glisser réel depuis la vignette de prise (`button` « Glisser vers
#     une appli » : appui, 4 px et plus) vers la fenêtre de cible-glisser.ps1 : `glisser
#     <id> debut`, `depose`, un fichier reçu au modèle, mêmes octets que le « Récents » de
#     cet export ;
# (14) éditeur ramené au premier plan (SetForegroundWindow, sinon clic réel au milieu de la
#     barre du haut de la page, zone de glisser, à 26 px CSS du haut ; vérifié), un
#     rectangle, --quitter : `ferme <id> (arret)`, `arret propre`.
# Cas B :
# (15) éditeur ouvert ≤ repos + 90 Mo ; 20 ouvertures et fermetures ; 10 s après : le
#     processus de rendu de l'éditeur est parti (même nombre qu'au repos), le processus de
#     l'app n'a pas pris plus de 8 Mo depuis la première fermeture (une image gardée par
#     ouverture en ferait 60), app et WebView2 ensemble ≤ repos chaud + 40 Mo (une page
#     gardée en ferait 50 ; le repos varie de quelques Mo et les pages permanentes
#     grossissent avec les captures, éditeur ou non) ; fenêtres et fils stables ; la
#     mémoire par sorte de processus est notée à chaque étape, puis après 40 captures sans
#     éditeur (sans seuil).
#
# Non vérifiable sur GitHub (un écran 1024 × 768 à 100 %) : plusieurs écrans, 125, 150 et
# 200 %, captures de 3 × 1080p et 2K, Enregistrer sous… jusqu'au bout (le chemin tapé dans
# le dialogue du système), le glisser vers l'Explorateur, Word ou Teams.
# Hors Windows : démarrage seulement (aucune fenêtre d'éditeur préparée).

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '12-editeur' -Exe $Exe -Sortie $Sortie | Out-Null

$script:Auxiliaires = [System.Collections.Generic.List[object]]::new()
$script:Pwsh = (Get-Process -Id $PID).Path

# --- Aides (même forme que les essais 07, 08 et 10) --------------------------------------

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

# Les 40 dernières lignes sur l'éditeur, la vignette, ou de niveau WARN / ERROR, sans
# horodatage (ni chemin ni contenu : contrat §8).
function Diagnostic-Journal([string]$Dossier) {
    $l = @(Lignes-Journal $Dossier | Where-Object { $_ -match 'editeur|vignette|WARN|ERROR' } | Select-Object -Last 40 | ForEach-Object { $_ -replace '^\S+\s+', '' })
    if ($l.Count -eq 0) { return '(journal : aucune ligne editeur, vignette, WARN ni ERROR)' }
    return 'journal : ' + ($l -join ' | ')
}

function Fichiers-Png([string]$Dossier) {
    if (-not (Test-Path -LiteralPath $Dossier)) { return @() }
    return @(Get-ChildItem -LiteralPath $Dossier -File -Filter '*.png' | Sort-Object LastWriteTimeUtc, Name)
}

function Ecrire-Reglages([string]$Dossier, [string]$Images) {
    $r = [ordered]@{
        version                   = 3
        raccourcis                = [ordered]@{ zone = @(); plein_ecran = @('Ctrl+Maj+2'); fenetre = @(); derniere_zone = @() }
        copier_apres_capture      = $true
        enregistrer_apres_capture = $false
        dossier_enregistrement    = $Images
        vignette                  = [ordered]@{ afficher = $true; duree_s = 0; coin = 'bas_gauche' }
        animations                = 'reduites'
        selection                 = [ordered]@{ loupe = $false }
        capture                   = [ordered]@{ retardateur_s = 0; pointeur = $false }
        demarrer_avec_systeme     = $false
        modele_nom                = 'BreezyShot {date} à {heure}'
        theme                     = 'systeme'
    }
    $d = Join-Path $Dossier 'reglages'
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $r | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $d 'reglages.json') -Encoding utf8
}

function Demarrer-Cas([string]$Dossier, [string]$Quoi) {
    $env:BREEZYSHOT_ESSAI_CAPTURABLE = '1'
    $prets = Compter-Motif -Motif 'application +pret$' -Dossier $Dossier
    $vignettes = Compter-Motif -Motif 'vignette prete$' -Dossier $Dossier
    try { $proc = Demarrer-App -Dossier $Dossier }
    finally { Remove-Item Env:BREEZYSHOT_ESSAI_CAPTURABLE -ErrorAction SilentlyContinue }
    if ($IsWindows) { [BreezyShotEssai.Editeur]::PidApp = $proc.Id }
    $null = Attendre-Motif -Motif 'application +pret$' -Dossier $Dossier -DelaiS 30 -Quoi "$Quoi : démarrage" -Fois ($prets + 1)
    $null = Attendre-Motif -Motif 'vignette prete$' -Dossier $Dossier -DelaiS 30 -Quoi "$Quoi : page de la vignette" -Fois ($vignettes + 1)
    return $proc
}

function Fermer-Cas($Processus, [string]$Dossier, [string]$Quoi) {
    $arrets = Compter-Motif -Motif 'arret propre$' -Dossier $Dossier
    $q = Quitter-App -DelaiS 15 -Dossier $Dossier
    if (-not $q.Fini) { throw "$Quoi : --quitter n'a pas répondu en 15 s" }
    if (-not $Processus.WaitForExit(15000)) { throw "$Quoi : l'app ne s'est pas fermée en 15 s" }
    $null = Attendre-Motif -Motif 'arret propre$' -Dossier $Dossier -DelaiS 3 -Quoi "$Quoi : arrêt" -Fois ($arrets + 1)
}

# La mémoire privée par sorte de processus : l'app, puis les processus WebView2 d'après
# leur `--type` (sans type : le navigateur). Rien d'autre de la ligne de commande n'est lu.
# Rend { Texte, App (Mo), Rendus (nombre de processus de rendu) }.
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
        foreach ($e in $enfants[$n.Id]) {
            $id = [int]$e.ProcessId
            if ($vus.ContainsKey($id)) { continue }
            $vus[$id] = $true
            $sorte = if ("$($e.CommandLine)" -match '--type=([\w-]+)') { $Matches[1] } else { 'navigateur' }
            $file.Enqueue([pscustomobject]@{ Id = $id; Sorte = $sorte })
        }
    }
    $texte = (@($sommes.Keys) | ForEach-Object { "$_ x$($nombres[$_]) $([math]::Round($sommes[$_], 1))" }) -join ' ; '
    return [pscustomobject]@{
        Texte  = $texte
        App    = [math]::Round([double]$sommes['app'], 1)
        Rendus = $(if ($nombres.ContainsKey('renderer')) { [int]$nombres['renderer'] } else { 0 })
    }
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
        $proc = Demarrer-Cas -Dossier $d -Quoi 'commun'
        Verifier '(1) aucune fenêtre d''éditeur au démarrage' ((Compter-Motif -Motif 'editeur fenetre' -Dossier $d) -eq 0) | Out-Null
        Fermer-Cas $proc $d 'commun'
        Verifier '--quitter → arret propre' $true | Out-Null
        Write-Host '[saute] (2) à (15) : sautés hors Windows'
    }
    else {
        Add-Type -AssemblyName UIAutomationClient
        Add-Type -AssemblyName UIAutomationTypes
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms
        Add-Type -Namespace BreezyShotEssai -Name Editeur -MemberDefinition @'
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
public static extern bool IsWindowEnabled(IntPtr h);
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
static void Bas(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), 0u, UIntPtr.Zero); }
static void Haut(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), RELACHEE, UIntPtr.Zero); }
public static void Appuyer(byte[] mods, byte touche) {
    Garder(GetForegroundWindow(), "touche 0x" + touche.ToString("X2") + " non envoyée : le premier plan n'est pas à l'app");
    AppuyerGlobal(mods, touche);
}
// Un raccourci global de l'app : il ne vise aucune fenêtre, il part sans garde.
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
// Un glisser réel : appui en (x0, y0), déplacement en 20 pas, relâché en (x1, y1).
public static void Tracer(int x0, int y0, int x1, int y1) {
    Poser(x0, y0); System.Threading.Thread.Sleep(80);
    BoutonBas(); System.Threading.Thread.Sleep(80);
    Deplacer(x0, y0, x1, y1, 20, 15);
    System.Threading.Thread.Sleep(80);
    BoutonHaut(); System.Threading.Thread.Sleep(80);
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
// Le dialogue (#32770) possédé par `proprietaire`, ou zéro.
public static IntPtr Dialogue(IntPtr proprietaire) {
    IntPtr trouve = IntPtr.Zero;
    EnumWindows((h, p) => {
        if (Classe(h) == "#32770" && GetWindow(h, 4) == proprietaire) { trouve = h; return false; }
        return true;
    }, IntPtr.Zero);
    return trouve;
}
// Pixels BGRA (largeur l) : nombre de pixels différents hors du rectangle exclu
// [x0, x1] × [y0, y1] (bornes comprises ; x0 > x1 : rien d'exclu).
public static int Differences(byte[] a, byte[] b, int l, int h, int x0, int y0, int x1, int y1) {
    if (a.Length != b.Length || a.Length != l * h * 4) return -1;
    int n = 0;
    for (int y = 0; y < h; y++) for (int x = 0; x < l; x++) {
        if (x >= x0 && x <= x1 && y >= y0 && y <= y1) continue;
        int i = (y * l + x) * 4;
        if (a[i] != b[i] || a[i + 1] != b[i + 1] || a[i + 2] != b[i + 2] || a[i + 3] != b[i + 3]) n++;
    }
    return n;
}
// Les blocs de `bloc` pixels alignés sur la grille de l'image et entièrement dans
// [x0, x1) × [y0, y1) : rend le nombre de blocs non unis ; `couleurs` : couleurs distinctes.
public static int BlocsNonUnis(byte[] a, int l, int x0, int y0, int x1, int y1, int bloc, out int couleurs, out int blocs) {
    var vues = new System.Collections.Generic.HashSet<int>();
    int mauvais = 0; blocs = 0;
    for (int by = (y0 + bloc - 1) / bloc * bloc; by + bloc <= y1; by += bloc)
    for (int bx = (x0 + bloc - 1) / bloc * bloc; bx + bloc <= x1; bx += bloc) {
        blocs++;
        int i0 = (by * l + bx) * 4; bool uni = true;
        for (int y = by; y < by + bloc && uni; y++) for (int x = bx; x < bx + bloc; x++) {
            int i = (y * l + x) * 4;
            if (a[i] != a[i0] || a[i + 1] != a[i0 + 1] || a[i + 2] != a[i0 + 2]) { uni = false; break; }
        }
        if (!uni) mauvais++;
        vues.Add(a[i0] | (a[i0 + 1] << 8) | (a[i0 + 2] << 16));
    }
    couleurs = vues.Count;
    return mauvais;
}
// La couleur RVB (0xRRVVBB) du pixel (x, y).
public static int Pixel(byte[] a, int l, int x, int y) {
    int i = (y * l + x) * 4;
    return (a[i + 2] << 16) | (a[i + 1] << 8) | a[i];
}
'@
        try { [void][BreezyShotEssai.Editeur]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }
        $E = [BreezyShotEssai.Editeur]
        $AE = [System.Windows.Automation.AutomationElement]

        $MODS = [byte[]]@(0x11, 0x10)          # Ctrl, Maj
        $CTRL = [byte[]]@(0x11)
        $ALT = [byte[]]@(0x12)
        $AUCUN = [byte[]]@()
        $TOUCHE_2 = [byte]0x32
        $ECHAP = [byte]0x1B
        $WM_CLOSE = [uint32]0x10
        $WM_COMMAND = [uint32]0x111
        $TITRE = 'Annoter la capture'
        $MODELE = '^BreezyShot \d{4}-\d\d-\d\d à \d\d\.\d\d\.\d\d( \(\d+\))?\.png$'
        $utile = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        $REPOS_X = [int]($utile.X + $utile.Width / 2)
        $REPOS_Y = [int]($utile.Y + 8)
        Write-Host "zone utile $utile"

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

        # Le presse-papiers, dans un fil STA : l'image en PNG (octets), ou $null.
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

        function Lancer-Aide([string]$Script, [string[]]$Arguments, [string]$Fichier) {
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -ArgumentList (@('-NoProfile', '-File', (Join-Path $PSScriptRoot $Script)) + $Arguments)
            $script:Auxiliaires.Add($aide)
            $limite = [DateTime]::UtcNow.AddSeconds(30)
            while (-not (Test-Path -LiteralPath $Fichier)) {
                if ([DateTime]::UtcNow -ge $limite) { throw "$Script : aucun HWND en 30 s" }
                Start-Sleep -Milliseconds 100
            }
            Start-Sleep -Milliseconds 400
            return $aide
        }

        # Ctrl + Maj + 2, puis la carte entrée : { Id, Carte, Recent (PNG de « Récents ») }.
        function Capturer([string]$Dossier, [string]$Quoi) {
            $E::Poser($REPOS_X, $REPOS_Y)
            $entrees = Compter-Motif -Motif 'vignette entree \d+$' -Dossier $Dossier
            $avant = @(Fichiers-Png (Join-Path $Dossier 'local/Recents') | ForEach-Object { $_.FullName })
            $E::AppuyerGlobal($MODS, $TOUCHE_2)
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
        function Cliquer-Annoter([string]$Dossier, $C) { Cliquer-Bouton $Dossier $C 'annoter' }

        # Annoter sur la carte $C, puis l'image affichée : rend le HWND de la fenêtre.
        function Ouvrir-Editeur([string]$Dossier, $C, [string]$Quoi) {
            $affichees = Compter-Motif -Motif "editeur image affichee $($C.Id) " -Dossier $Dossier
            Cliquer-Annoter $Dossier $C
            $null = Attendre-Motif -Motif "vignette fermee $($C.Id) annoter$" -Dossier $Dossier -DelaiS 5 -Quoi "$Quoi : vignette fermee annoter"
            $null = Attendre-Motif -Motif "editeur image affichee $($C.Id) en \d+ ms$" -Dossier $Dossier -DelaiS 40 -Quoi "$Quoi : image affichée" -Fois ($affichees + 1)
            $h = $E::FenetresTitre($script:proc.Id, $TITRE)
            if ($h.Count -ne 1) { throw "$Quoi : $($h.Count) fenêtre(s) « $TITRE »" }
            Start-Sleep -Milliseconds 300
            return $h[0]
        }

        # La vue (Ajuster) : rectangle de l'image à l'écran, et le passage image → écran.
        function Ajuster([string]$Dossier, [int]$Id, $Image) {
            $vues = Compter-Motif -Motif "editeur vue $Id " -Dossier $Dossier
            $E::Appuyer($CTRL, [byte]0x30)
            $v = Lire-Rect (Attendre-Suivante "editeur vue $Id " $Dossier $vues 'vue après Ctrl+0' 5)
            $v | Add-Member S ($v.L / $Image.L)
            return $v
        }
        function Ecran-X($Vue, [double]$X) { return [int][math]::Round($Vue.X + $X * $Vue.S) }
        function Ecran-Y($Vue, [double]$Y) { return [int][math]::Round($Vue.Y + $Y * $Vue.S) }

        # Un geste d'outil (touche, puis glisser réel de (x0, y0) à (x1, y1) de l'image), et
        # le temps laissé à la page pour dire son état (`editeur_etat`, 250 ms au plus).
        function Tracer-Image($Vue, [byte]$Outil, [double]$X0, [double]$Y0, [double]$X1, [double]$Y1) {
            if ($null -eq $Vue) { throw 'vue inconnue : aucun geste envoyé' }
            $E::Touche($Outil)
            $E::Tracer((Ecran-X $Vue $X0), (Ecran-Y $Vue $Y0), (Ecran-X $Vue $X1), (Ecran-Y $Vue $Y1))
            Start-Sleep -Milliseconds 600
            $E::Poser($REPOS_X, $REPOS_Y)
        }

        # Ctrl + C (ou autre), puis le bilan de l'export : rend { Ligne, Ms }.
        function Exporter([string]$Dossier, [int]$Id, [byte[]]$Mods, [byte]$Touche, [string]$Motif, [string]$Quoi) {
            $avant = Compter-Motif -Motif "editeur export $Id \d+ $Motif" -Dossier $Dossier
            $chrono = [System.Diagnostics.Stopwatch]::StartNew()
            $E::Appuyer($Mods, $Touche)
            $l = Attendre-Suivante "editeur export $Id \d+ $Motif" $Dossier $avant $Quoi 15
            return [pscustomobject]@{ Ligne = $l; Ms = $chrono.ElapsedMilliseconds }
        }

        function Proche([int]$Rvb, [int]$Attendu, [int]$Tolerance) {
            foreach ($decalage in @(16, 8, 0)) {
                $a = ($Rvb -shr $decalage) -band 0xFF
                $b = ($Attendu -shr $decalage) -band 0xFF
                if ([math]::Abs($a - $b) -gt $Tolerance) { return $false }
            }
            return $true
        }

        # Un élément de la page par son nom accessible, attendu ; clic quand son rectangle
        # ne bouge plus.
        function Element([IntPtr]$Racine, [string]$Nom, $Type, [double]$DelaiS = 5) {
            $cond = [System.Windows.Automation.AndCondition]::new(
                [System.Windows.Automation.PropertyCondition]::new($AE::NameProperty, $Nom),
                [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, $Type))
            $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
            do {
                try {
                    $e = $AE::FromHandle($Racine).FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
                    if ($null -ne $e) { return $e }
                } catch { }
                Start-Sleep -Milliseconds 100
            } while ([DateTime]::UtcNow -lt $limite)
            throw "« $Nom » ($($Type.ProgrammaticName)) introuvable dans l'éditeur"
        }

        # La zone cliente à l'écran : { X, Y, L, H } en pixels réels, et l'échelle.
        function Zone-Cliente([IntPtr]$H) {
            $c = [BreezyShotEssai.Editeur+RECT]::new()
            $null = $E::GetClientRect($H, [ref]$c)
            $o = [BreezyShotEssai.Editeur+POINT]::new()
            $null = $E::ClientToScreen($H, [ref]$o)
            $dpi = $E::GetDpiForWindow($H)
            return [pscustomobject]@{ X = $o.X; Y = $o.Y; L = $c.Right; H = $c.Bottom; S = $(if ($dpi -gt 0) { $dpi / 96.0 } else { 1.0 }) }
        }

        # Le menu « Fermer » de la page (`menu` « Fermer », avenant §4) est ouvert et le focus
        # est sur son `menuitem` `$Article` : attendus par UI Automation, bornés.
        function Attendre-Focus([IntPtr]$Racine, [string]$Article, [string]$Quoi, [double]$DelaiS = 5) {
            $menu = [System.Windows.Automation.AndCondition]::new(
                [System.Windows.Automation.PropertyCondition]::new($AE::NameProperty, 'Fermer'),
                [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, [System.Windows.Automation.ControlType]::Menu))
            $cond = [System.Windows.Automation.AndCondition]::new(
                [System.Windows.Automation.PropertyCondition]::new($AE::NameProperty, $Article),
                [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, [System.Windows.Automation.ControlType]::MenuItem))
            $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
            $vu = 'aucun'
            do {
                try {
                    $m = $AE::FromHandle($Racine).FindFirst([System.Windows.Automation.TreeScope]::Descendants, $menu)
                    $b = if ($null -ne $m) { $m.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond) } else { $null }
                    $f = $AE::FocusedElement
                    if ($null -ne $f) { $vu = "« $($f.Current.Name) » ($($f.Current.ControlType.ProgrammaticName))" }
                    if ($null -ne $b -and $b.Current.HasKeyboardFocus) { return $b }
                } catch { }
                Start-Sleep -Milliseconds 50
            } while ([DateTime]::UtcNow -lt $limite)
            throw "$Quoi : menu « Fermer » ou focus sur « $Article » absent en $DelaiS s (focus $vu)"
        }

        # L'éditeur au premier plan : SetForegroundWindow, sinon un clic réel au milieu de la
        # barre du haut de la page (zone de glisser, 26 px CSS sous le haut : il n'y a plus
        # de barre de titre Windows) ; vérifié, borné.
        function Ramener-Devant([IntPtr]$H, [string]$Quoi) {
            $null = $E::SetForegroundWindow($H)
            $limite = [DateTime]::UtcNow.AddSeconds(2)
            while ($E::GetForegroundWindow() -ne $H -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 50 }
            if ($E::GetForegroundWindow() -ne $H) {
                $z = Zone-Cliente $H
                $E::Cliquer([int]($z.X + $z.L / 2), [int]($z.Y + [math]::Round(26 * $z.S)))
                $limite = [DateTime]::UtcNow.AddSeconds(3)
                while ($E::GetForegroundWindow() -ne $H -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 50 }
            }
            $E::Poser($REPOS_X, $REPOS_Y)
            if ($E::GetForegroundWindow() -ne $H) { throw "$Quoi : l'éditeur n'est pas revenu au premier plan (premier plan $($E::GetForegroundWindow()))" }
        }

        function Centre-Stable($e) {
            $r = $e.Current.BoundingRectangle
            $limite = [DateTime]::UtcNow.AddSeconds(2)
            do {
                Start-Sleep -Milliseconds 120
                $avant = $r; $r = $e.Current.BoundingRectangle
            } while (($r.X -ne $avant.X -or $r.Y -ne $avant.Y) -and [DateTime]::UtcNow -lt $limite)
            if ($r.IsEmpty -or $r.Width -le 0) { throw "élément sans rectangle" }
            return @([int]($r.X + $r.Width / 2), [int]($r.Y + $r.Height / 2))
        }

        $script:proc = $null
        $temoin = $null
        $script:h1 = [IntPtr]::Zero; $script:h11 = [IntPtr]::Zero; $script:c11 = $null
        $script:vue = $null; $script:export6 = $null; $script:dernier = $null
        try {
            # ===================== Cas A =========================================================
            $da = Dossier-Cas 'a'
            $images = Join-Path $da 'images'
            New-Item -ItemType Directory -Force -Path $images | Out-Null
            $recents = Join-Path $da 'local/Recents'
            Ecrire-Reglages -Dossier $da -Images $images
            # Témoin : quatre carrés connus (frontière verticale en x = 630 de l'écran).
            $fichierTemoin = Join-Path $script:Essai.Dossier 'temoin-12.txt'
            $temoin = Lancer-Aide 'temoin.ps1' @('-X', '500', '-Y', '380', '-Cote', '260', '-Couleurs', '220,40,40;40,160,60;40,80,200;230,200,40', '-Fichier', $fichierTemoin, '-CacherConsole') $fichierTemoin
            $script:proc = Demarrer-Cas -Dossier $da -Quoi '(A)'

            # (1) Repos.
            Start-Sleep -Seconds 3
            $repos = Memoire-Privee-Mo -Processus $script:proc
            Noter-Mesure -Nom 'memoire privee au repos' -Valeur $repos -Unite 'Mo'
            Verifier '(1) aucune fenêtre « Annoter la capture » au repos' (($E::FenetresTitre($script:proc.Id, $TITRE)).Count -eq 0 -and (Compter-Motif -Motif 'editeur fenetre' -Dossier $da) -eq 0) | Out-Null

            # (2) Capture, Annoter.
            $c1 = Capturer $da '(2)'
            try { $temoin.Kill() } catch { }   # il est toujours au-dessus : il couvrirait l'éditeur
            $prep = Derniere-Ligne -Motif "vignette preparee $($c1.Id) " -Dossier $da
            Verifier '(2) vignette préparée avec Annoter avant Supprimer' ($prep -match 'actions copier,enregistrer,annoter,supprimer,fermer,annuler ') $prep | Out-Null
            Point '(2) Annoter ouvre l''éditeur' {
                $script:h1 = Ouvrir-Editeur $da $c1 '(2)'
                $lf = Derniere-Ligne -Motif 'editeur fenetre (\d+)$' -Dossier $da
                $null = $lf -match 'editeur fenetre (\d+)$'
                Verifier '(2) vignette fermee annoter, Vignette -> Repos (VignetteFermee), editeur fenetre' (((Compter-Motif -Motif 'etat Vignette -> Repos \(VignetteFermee\)' -Dossier $da) -ge 1) -and ([IntPtr][long]$Matches[1] -eq $script:h1)) "$lf" | Out-Null
            }
            $hwnd = $script:h1
            if ($hwnd -eq [IntPtr]::Zero) { throw "(2) pas d'éditeur : la suite n'est pas jouée ; $(Diagnostic-Journal $da)" }

            # (3) La fenêtre.
            Point '(3) fenêtre de l''éditeur' {
                $r = [BreezyShotEssai.Editeur+RECT]::new()
                $null = $E::GetWindowRect($hwnd, [ref]$r)
                $exStyle = $E::GetWindowLongPtr($hwnd, -20).ToInt64()
                $outil = ($exStyle -band 0x80) -ne 0
                $z = Zone-Cliente $hwnd
                $dans = ($z.X -ge $utile.Left) -and ($z.Y -ge $utile.Top) -and ($z.X + $z.L -le $utile.Right) -and ($z.Y + $z.H -le $utile.Bottom)
                Verifier '(3) visible, titre exact' ($E::IsWindowVisible($hwnd) -and $E::Titre($hwnd) -eq $TITRE) | Out-Null
                Verifier '(3) au premier plan' ($E::GetForegroundWindow() -eq $hwnd) "premier plan $($E::GetForegroundWindow())" | Out-Null
                Verifier '(3) dans la barre des tâches (ni outil ni propriétaire)' ((-not $outil) -and $E::GetWindow($hwnd, 4) -eq [IntPtr]::Zero) | Out-Null
                Verifier '(3) zone cliente dans la zone utile' $dans "$($z.X),$($z.Y) $($z.L)x$($z.H) ; zone $utile" | Out-Null
                $lo = Derniere-Ligne -Motif "editeur ouvert $($c1.Id) .* fenetre -?\d+,-?\d+ \d+x\d+$" -Dossier $da
                if ($lo -notmatch 'fenetre (-?\d+,-?\d+ \d+x\d+)$') { throw "(3) ligne editeur ouvert illisible : $lo" }
                $texte = $Matches[1]
                $prevue = Lire-Rect $texte
                Verifier '(3) zone cliente au rectangle fenetre de editeur ouvert (0 px)' ($z.X -eq $prevue.X -and $z.Y -eq $prevue.Y -and $z.L -eq $prevue.L -and $z.H -eq $prevue.H) "client $($z.X),$($z.Y) $($z.L)x$($z.H) ; ligne $texte" | Out-Null
                $style = $E::GetWindowLongPtr($hwnd, -16).ToInt64()
                $haut = $z.Y - $r.Top
                Verifier '(3) sans barre de titre (zone cliente à 2 px au plus du haut), redimensionnable (WS_SIZEBOX)' (($haut -ge 0) -and ($haut -le 2) -and (($style -band 0x40000) -ne 0)) "haut $haut px, style 0x$('{0:X8}' -f $style)" | Out-Null
                $la = Derniere-Ligne -Motif "editeur image affichee $($c1.Id) en (\d+) ms$" -Dossier $da
                $null = $la -match 'en (\d+) ms$'
                Noter-Mesure -Nom 'clic Annoter -> image affichee (budget 800)' -Valeur ([int]$Matches[1]) -Unite 'ms'
                $lp = Derniere-Ligne -Motif "editeur pret $($c1.Id) en (\d+) ms$" -Dossier $da
                if ($lp -match 'en (\d+) ms$') { Noter-Mesure -Nom 'clic Annoter -> page prete' -Valeur ([int]$Matches[1]) -Unite 'ms' }
            }

            # Si (3) a noté que l'éditeur n'est pas devant, la suite le ramène pour être jouée.
            Point '(3) éditeur devant pour la suite' {
                if ($E::GetForegroundWindow() -ne $hwnd) { Ramener-Devant $hwnd '(3)' }
            }

            if ($null -eq $c1.Recent) { throw "(2) aucun PNG de la capture dans « Récents » ; $(Diagnostic-Journal $da)" }
            $reference = Octets-Png ([System.IO.File]::ReadAllBytes($c1.Recent))
            # (4) Taille réelle, puis Ajuster.
            Point '(4) Ctrl+1 puis Ctrl+0' {
                $motif = "editeur vue $($c1.Id) .* zoom 1\.00$"
                $vues = Compter-Motif -Motif $motif -Dossier $da
                $E::Appuyer($CTRL, [byte]0x31)
                $l = Attendre-Suivante $motif $da $vues '(4) vue zoom 1.00 après Ctrl+1' 5
                $v1 = Lire-Rect $l
                Verifier '(4) Ctrl+1 : zoom 1.00, un pixel pour un pixel' (($l -match 'zoom 1\.00$') -and $v1.L -eq $reference.L -and $v1.H -eq $reference.H) $l | Out-Null
                $script:vue = Ajuster $da $c1.Id $reference
                Verifier '(4) Ctrl+0 : Ajuster, au plus 100 %' ($script:vue.S -gt 0.2 -and $script:vue.S -le 1.0) "échelle $($script:vue.S)" | Out-Null
            }
            $vue = $script:vue

            # (5) Rectangle, Ctrl+C.
            Point '(5) rectangle copié' {
                Tracer-Image $vue ([byte]0x52) 100 100 300 200
                $x = Exporter $da $c1.Id $CTRL ([byte]0x43) 'copie ' '(5) bilan de Ctrl+C'
                Noter-Mesure -Nom 'Ctrl+C -> bilan (budget 400)' -Valeur $x.Ms -Unite 'ms'
                Verifier '(5) copie faite recent ok fichier non demande' ($x.Ligne -match 'copie faite recent ok fichier non demande en \d+ ms$') $x.Ligne | Out-Null
                $p = Lire-PressePapiers
                $marge = 12 + 3 + [int][math]::Ceiling(2 / $vue.S)
                $d = $E::Differences($p.O, $reference.O, $p.L, $p.H, 100 - $marge, 100 - $marge, 300 + $marge, 200 + $marge)
                Verifier '(5) hors de la boîte : égal à la référence au bit près' ($d -eq 0) "$d pixel(s) différent(s), image $($p.L)x$($p.H)" | Out-Null
                $bord = $E::Pixel($p.O, $p.L, 200, 100)
                Verifier '(5) bord haut à #E5484D ± 6' (Proche $bord 0xE5484D 6) ('#{0:X6}' -f $bord) | Out-Null
            }

            # (6) Pixeliser sur la frontière de deux carrés du témoin.
            Point '(6) pixeliser' {
                Tracer-Image $vue ([byte]0x42) 591 400 669 470
                $x = Exporter $da $c1.Id $CTRL ([byte]0x43) 'copie faite' '(6) bilan de Ctrl+C'
                $p = Lire-PressePapiers
                $script:export6 = $p
                $couleurs = 0; $blocs = 0
                $m = $E::BlocsNonUnis($p.O, $p.L, 591 + 4, 400 + 4, 669 - 4, 470 - 4, 13, [ref]$couleurs, [ref]$blocs)
                Verifier '(6) blocs de 13 px unis, alignés sur la grille, deux couleurs au moins' (($m -eq 0) -and $blocs -ge 4 -and $couleurs -ge 2) "$m non unis sur $blocs, $couleurs couleurs" | Out-Null
            }
            $export6 = $script:export6

            # (7) Annuler, rétablir.
            Point '(7) annuler et rétablir' {
                $E::Appuyer($CTRL, [byte]0x5A); $E::Appuyer($CTRL, [byte]0x5A)
                Start-Sleep -Milliseconds 400
                $null = Exporter $da $c1.Id $CTRL ([byte]0x43) 'copie faite' '(7) Ctrl+C après annuler'
                $p = Lire-PressePapiers
                $d = $E::Differences($p.O, $reference.O, $p.L, $p.H, 1, 1, 0, 0)
                Verifier '(7) Ctrl+Z × 2 : égal à la référence' ($d -eq 0) "$d pixel(s)" | Out-Null
                $E::Appuyer($CTRL, [byte]0x59); $E::Appuyer($CTRL, [byte]0x59)
                Start-Sleep -Milliseconds 400
                $null = Exporter $da $c1.Id $CTRL ([byte]0x43) 'copie faite' '(7) Ctrl+C après rétablir'
                $p = Lire-PressePapiers
                $d = if ($null -ne $export6) { $E::Differences($p.O, $export6.O, $p.L, $p.H, 1, 1, 0, 0) } else { -1 }
                Verifier '(7) Ctrl+Y × 2 : égal à l''export du (6)' ($d -eq 0) "$d pixel(s)" | Out-Null
                $script:dernier = $p
            }

            # (8) Enregistrer, puis encore.
            Point '(8) enregistrer' {
                $avant = @(Fichiers-Png $images).Count
                $x = Exporter $da $c1.Id $CTRL ([byte]0x53) 'copie non demandee recent \S+( demande)? fichier' '(8) bilan de Ctrl+S'
                $f = @(Fichiers-Png $images)
                $ok = ($x.Ligne -match 'fichier fait en') -and $f.Count -eq ($avant + 1) -and $f[-1].Name -match $MODELE
                Verifier '(8) un fichier au modèle' $ok "$($x.Ligne) ; $($f.Count) fichier(s)" | Out-Null
                if ($ok) {
                    $png = Octets-Png ([System.IO.File]::ReadAllBytes($f[-1].FullName))
                    $d = $E::Differences($png.O, $script:dernier.O, $png.L, $png.H, 1, 1, 0, 0)
                    Verifier '(8) PNG égal au presse-papiers' ($d -eq 0) "$d pixel(s)" | Out-Null
                }
                $null = Exporter $da $c1.Id $CTRL ([byte]0x53) 'deja fait$' '(8) second Ctrl+S'
                Start-Sleep -Milliseconds 500
                Verifier '(8) deja fait : aucun second fichier' (@(Fichiers-Png $images).Count -eq ($avant + 1)) | Out-Null
                $ok = Compter-Motif -Motif "editeur export $($c1.Id) \d+ copie \S+( demandee)? recent ok " -Dossier $da
                $annotes = @(Fichiers-Png $recents | Where-Object { $_.FullName -ne $c1.Recent }).Count
                Verifier '(8) « Récents » : une version annotée par version exportée' ($ok -ge 1 -and $annotes -eq $ok) "$annotes fichier(s), $ok recent ok" | Out-Null
            }

            # (9) WM_CLOSE sans changement depuis l'export.
            Point '(9) fermeture propre' {
                $null = $E::PostMessage($hwnd, $WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
                $null = Attendre-Motif -Motif "editeur ferme $($c1.Id) \(propre\)$" -Dossier $da -DelaiS 10 -Quoi '(9) ferme (propre)'
                $null = Attendre-Motif -Motif "editeur pixels rendus $($c1.Id)$" -Dossier $da -DelaiS 3 -Quoi '(9) pixels rendus'
                $limite = [DateTime]::UtcNow.AddSeconds(5)
                while ($E::IsWindow($hwnd) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
                Verifier '(9) ferme (propre), pixels rendus, fenêtre détruite' (-not $E::IsWindow($hwnd)) | Out-Null
            }

            # (10) Modifié : Alt+F4 ouvre le menu « Fermer » ; Échap continue ; « Fermer sans copier ».
            Point '(10) fermer sans garder' {
                $c = Capturer $da '(10)'
                $h = Ouvrir-Editeur $da $c '(10)'
                $v = Ajuster $da $c.Id $reference
                Tracer-Image $v ([byte]0x52) 200 200 400 300
                $n = Compter-Motif -Motif "editeur fermeture $($c.Id): demander$" -Dossier $da
                $E::Appuyer($ALT, [byte]0x73)
                $null = Attendre-Suivante "editeur fermeture $($c.Id): demander$" $da $n '(10) Alt+F4 : demander' 10
                $null = Attendre-Focus $h 'Copier et fermer' '(10) menu Fermer après Alt+F4'
                Start-Sleep -Milliseconds 2600   # au-delà du filet de 2 s : la page a répondu
                Verifier '(10) Alt+F4 : demander, la fenêtre reste' ($E::IsWindow($h) -and (Compter-Motif -Motif "editeur ferme $($c.Id) " -Dossier $da) -eq 0) | Out-Null
                $E::Touche($ECHAP)
                Start-Sleep -Milliseconds 1000
                Verifier '(10) Échap (Continuer) : rien' ($E::IsWindow($h) -and (Compter-Motif -Motif "editeur ferme $($c.Id) " -Dossier $da) -eq 0) | Out-Null
                $E::Appuyer($ALT, [byte]0x73)
                $null = Attendre-Suivante "editeur fermeture $($c.Id): demander$" $da ($n + 1) '(10) second Alt+F4' 10
                $null = Attendre-Focus $h 'Copier et fermer' '(10) menu Fermer après le second Alt+F4'
                $sans = Element $h 'Fermer sans copier' ([System.Windows.Automation.ControlType]::MenuItem)
                $p = Centre-Stable $sans
                $E::Cliquer($p[0], $p[1])
                $null = Attendre-Motif -Motif "editeur ferme $($c.Id) \(sans garder\)$" -Dossier $da -DelaiS 10 -Quoi '(10) ferme (sans garder)'
                Verifier '(10) « Fermer sans copier » : ferme (sans garder)' $true | Out-Null
            }

            # (11) et (12) Remplacer ; capture éditeur ouvert ; demander, Continuer.
            Point '(11) remplacer, puis demander' {
                $ca = Capturer $da '(11) a'
                $h = Ouvrir-Editeur $da $ca '(11) a'
                $cb = Capturer $da '(11) b'
                $hb = Ouvrir-Editeur $da $cb '(11) b'
                $null = Attendre-Motif -Motif "editeur remplace $($ca.Id) par $($cb.Id)$" -Dossier $da -DelaiS 5 -Quoi '(11) remplace'
                Verifier '(11) remplace <a> par <b>, une seule fenêtre, la même' ($hb -eq $h) | Out-Null
                $v = Ajuster $da $cb.Id $reference
                Tracer-Image $v ([byte]0x52) 200 200 400 300
                $fermes = Compter-Motif -Motif 'editeur ferme ' -Dossier $da
                $exports = Compter-Motif -Motif 'editeur export ' -Dossier $da
                $cc = Capturer $da '(12)'
                Verifier '(12) capture faite éditeur ouvert : éditeur ouvert, même fenêtre, inchangé' ($E::IsWindowVisible($h) -and ($E::FenetresTitre($script:proc.Id, $TITRE)).Count -eq 1 -and (Compter-Motif -Motif 'editeur ferme ' -Dossier $da) -eq $fermes -and (Compter-Motif -Motif 'editeur export ' -Dossier $da) -eq $exports) | Out-Null
                Cliquer-Annoter $da $cc
                $null = Attendre-Motif -Motif "editeur fermeture demandee $($cb.Id) \(remplacer\)$" -Dossier $da -DelaiS 10 -Quoi '(11) fermeture demandee (remplacer)'
                $null = Attendre-Motif -Motif "editeur fermeture $($cb.Id): demander$" -Dossier $da -DelaiS 5 -Quoi '(11) demander'
                $null = Attendre-Focus $h 'Copier et fermer' '(11) menu Fermer (remplacer)'
                $E::Touche($ECHAP)
                $null = Attendre-Motif -Motif "editeur: capture $($cc.Id) abandonnee$" -Dossier $da -DelaiS 5 -Quoi '(11) capture abandonnee'
                Verifier '(11) Continuer : capture <c> abandonnee, le document b reste' ($E::IsWindow($h) -and (Compter-Motif -Motif "editeur remplace $($cb.Id) par" -Dossier $da) -eq 0) | Out-Null
                $script:h11 = $h; $script:c11 = $cb
            }

            # (13) Enregistrer sous… annulé ; Glisser vers une appli, vers une cible.
            Point '(13) enregistrer sous annulé' {
                $h = $script:h11; $c = $script:c11
                $avant = @(Fichiers-Png $images).Count
                $E::Appuyer($MODS, [byte]0x53)
                $dlg = [IntPtr]::Zero
                $limite = [DateTime]::UtcNow.AddSeconds(30)
                while ($dlg -eq [IntPtr]::Zero -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 200; $dlg = $E::Dialogue($h) }
                if ($dlg -eq [IntPtr]::Zero) { throw "aucun dialogue #32770 possédé par l'éditeur en 30 s ; $(Diagnostic-Journal $da)" }
                Verifier '(13) dialogue du système, modal à l''éditeur' (-not $E::IsWindowEnabled($h)) | Out-Null
                Start-Sleep -Milliseconds 500
                $null = $E::PostMessage($dlg, $WM_COMMAND, [IntPtr]2, [IntPtr]::Zero)
                $null = Attendre-Motif -Motif "editeur enregistrer sous $($c.Id): dialogue annule$" -Dossier $da -DelaiS 10 -Quoi '(13) dialogue annule'
                Start-Sleep -Milliseconds 500
                Verifier '(13) dialogue annule : rien d''écrit' (@(Fichiers-Png $images).Count -eq $avant -and @(Get-ChildItem -LiteralPath $images -File -Filter '*.tmp').Count -eq 0) | Out-Null
            }
            Point '(13) glisser vers une cible' {
                $h = $script:h11; $c = $script:c11
                $fh = Join-Path $script:Essai.Dossier 'cible-12.txt'
                $fd = Join-Path $script:Essai.Dossier 'depots-12.txt'
                $null = Lancer-Aide 'cible-glisser.ps1' @('-X', '780', '-Y', '20', '-Cote', '220', '-FichierHwnd', $fh, '-FichierDepots', $fd) $fh
                $E::Poser($REPOS_X, $REPOS_Y)
                $plus = Element $h 'Plus' ([System.Windows.Automation.ControlType]::Button)
                $p = Centre-Stable $plus
                $E::Cliquer($p[0], $p[1])
                $g = Element $h 'Glisser vers une appli' ([System.Windows.Automation.ControlType]::MenuItem)
                $q = Centre-Stable $g
                $E::Cliquer($q[0], $q[1])
                $E::Poser($REPOS_X, $REPOS_Y)
                # La vignette de prise (avenant §3) : le glisser réel part d'elle.
                $prise = Element $h 'Glisser vers une appli' ([System.Windows.Automation.ControlType]::Button)
                $q = Centre-Stable $prise
                $debuts = Compter-Motif -Motif "editeur glisser $($c.Id) debut$" -Dossier $da
                $E::Poser($q[0], $q[1]); Start-Sleep -Milliseconds 100
                $E::BoutonBas()
                $E::Deplacer($q[0], $q[1], $q[0] + 12, $q[1] - 6, 6, 20)
                try {
                    $null = Attendre-Suivante "editeur glisser $($c.Id) debut$" $da $debuts '(13) glisser debut' 10
                    $E::Deplacer($q[0] + 12, $q[1] - 6, 890, 130, 25, 20)
                    Start-Sleep -Milliseconds 300
                }
                finally { $E::BoutonHaut() }
                $null = Attendre-Motif -Motif "editeur glisser $($c.Id) depose$" -Dossier $da -DelaiS 10 -Quoi '(13) glisser depose'
                $limite = [DateTime]::UtcNow.AddSeconds(5)
                while (-not (Test-Path -LiteralPath $fd) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
                $recus = @(if (Test-Path -LiteralPath $fd) { Get-Content -LiteralPath $fd | Where-Object { $_ -ne '' } })
                $ok = $recus.Count -eq 1 -and (Split-Path -Leaf $recus[0]) -match $MODELE
                Verifier '(13) un fichier reçu, au modèle' $ok "$($recus.Count) reçu(s)" | Out-Null
                if ($ok) {
                    $dernierRecent = @(Fichiers-Png $recents)[-1].FullName
                    $pareil = [System.Linq.Enumerable]::SequenceEqual([System.IO.File]::ReadAllBytes($recus[0]), [System.IO.File]::ReadAllBytes($dernierRecent))
                    Verifier '(13) mêmes octets que le « Récents » du dernier export' $pareil | Out-Null
                }
                Verifier '(13) l''éditeur reste ouvert après le dépôt' ($E::IsWindow($h)) | Out-Null
            }

            # (14) --quitter éditeur modifié.
            Point '(14) quitter' {
                $h = $script:h11; $c = $script:c11
                Ramener-Devant $h '(14)'
                $v = Ajuster $da $c.Id $reference
                Tracer-Image $v ([byte]0x52) 500 300 600 400
                Fermer-Cas $script:proc $da '(14)'
                $ok = (Compter-Motif -Motif "editeur ferme $($c.Id) \(arret\)$" -Dossier $da) -eq 1
                Verifier '(14) --quitter : ferme (arret), arret propre' $ok "$(Diagnostic-Journal $da)" | Out-Null
            }
            Arreter-Tout

            # ===================== Cas B : mémoire, 20 ouvertures ==============================
            Point '(15) mémoire et stabilité' {
                $db = Dossier-Cas 'b'
                $imagesB = Join-Path $db 'images'
                New-Item -ItemType Directory -Force -Path $imagesB | Out-Null
                Ecrire-Reglages -Dossier $db -Images $imagesB
                $script:proc = Demarrer-Cas -Dossier $db -Quoi '(15)'
                Start-Sleep -Seconds 3
                $froid = Memoire-Privee-Mo -Processus $script:proc
                $d0 = Detail-Memoire $script:proc
                Noter-Mesure -Nom 'memoire privee au repos (cas B)' -Valeur $froid -Unite 'Mo'
                Noter-Mesure -Nom 'detail au repos (cas B)' -Valeur $d0.Texte -Unite 'Mo'
                $chaud = $null; $fils = $null; $fenetres = $null; $ouvert = $null; $dOuvert = $null; $d1 = $null
                for ($i = 1; $i -le 20; $i++) {
                    $c = Capturer $db "(15) $i"
                    $h = Ouvrir-Editeur $db $c "(15) $i"
                    if ($i -eq 1) {
                        Start-Sleep -Seconds 2
                        $ouvert = Memoire-Privee-Mo -Processus $script:proc
                        $dOuvert = Detail-Memoire $script:proc
                        Noter-Mesure -Nom 'memoire privee editeur ouvert' -Valeur $ouvert -Unite 'Mo'
                        Noter-Mesure -Nom 'detail editeur ouvert' -Valeur $dOuvert.Texte -Unite 'Mo'
                    }
                    $null = $E::PostMessage($h, $WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
                    $null = Attendre-Motif -Motif "editeur pixels rendus $($c.Id)$" -Dossier $db -DelaiS 10 -Quoi "(15) $i fermeture"
                    $limite = [DateTime]::UtcNow.AddSeconds(5)
                    while ($E::IsWindow($h) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
                    if ($E::IsWindow($h)) { throw "(15) $i : fenêtre non détruite" }
                    if ($i -eq 1) {
                        Start-Sleep -Seconds 10
                        $chaud = Memoire-Privee-Mo -Processus $script:proc
                        $d1 = Detail-Memoire $script:proc
                        $script:proc.Refresh()
                        $fils = $script:proc.Threads.Count
                        $fenetres = (Get-Process -Id $script:proc.Id).HandleCount
                        Noter-Mesure -Nom 'memoire privee apres une fermeture (repos chaud)' -Valeur $chaud -Unite 'Mo'
                        Noter-Mesure -Nom 'detail apres une fermeture' -Valeur $d1.Texte -Unite 'Mo'
                    }
                }
                Start-Sleep -Seconds 10
                $apres = Memoire-Privee-Mo -Processus $script:proc
                $d20 = Detail-Memoire $script:proc
                Noter-Mesure -Nom 'memoire privee 10 s apres 20 fermetures' -Valeur $apres -Unite 'Mo'
                Noter-Mesure -Nom 'detail 10 s apres 20 fermetures' -Valeur $d20.Texte -Unite 'Mo'
                Noter-Mesure -Nom 'ecart au repos chaud 10 s apres 20 fermetures' -Valeur ([math]::Round($apres - $chaud, 1)) -Unite 'Mo'
                Verifier '(15) éditeur ouvert ≤ repos + 90 Mo' ($ouvert -le $froid + 90) "$ouvert Mo, repos $froid Mo" | Out-Null
                # La page de l'éditeur a son processus de rendu : il part avec la fenêtre.
                $rendus = "repos $($d0.Rendus), ouvert $($dOuvert.Rendus), après 1 fermeture $($d1.Rendus), après 20 $($d20.Rendus)"
                Verifier '(15) le processus de rendu de l''éditeur part à la fermeture' (($d0.Rendus -ge 1) -and ($dOuvert.Rendus -gt $d0.Rendus) -and ($d1.Rendus -eq $d0.Rendus) -and ($d20.Rendus -eq $d0.Rendus)) $rendus | Out-Null
                # Le processus de l'app tient les pixels des captures : une image gardée par
                # ouverture ferait 3 Mo de plus à chaque fois (60 Mo ici).
                Verifier '(15) processus de l''app : après 20 fermetures ≤ après la première + 8 Mo' ($d20.App -le $d1.App + 8) "$($d20.App) Mo, après la première $($d1.App) Mo" | Out-Null
                # Tout l'arbre (app et WebView2) : le repos varie de quelques Mo d'une mesure à
                # l'autre et les pages permanentes grossissent avec les captures, éditeur ou
                # non ; une page d'éditeur gardée ferait 50 Mo de plus.
                Verifier '(15) app et WebView2 : 10 s après 20 fermetures ≤ repos chaud + 40 Mo' ($apres -le $chaud + 40) "$apres Mo, repos chaud $chaud Mo" | Out-Null
                $script:proc.Refresh()
                $filsApres = $script:proc.Threads.Count
                $poignees = (Get-Process -Id $script:proc.Id).HandleCount
                Noter-Mesure -Nom 'fils apres 20 fermetures' -Valeur "$fils -> $filsApres" -Unite ''
                Verifier '(15) fils et poignées stables' (($filsApres -le $fils + 3) -and ($poignees -le $fenetres + 60)) "fils $fils -> $filsApres, poignées $fenetres -> $poignees" | Out-Null
                Verifier '(15) aucune fenêtre « Annoter la capture » restante' (($E::FenetresTitre($script:proc.Id, $TITRE)).Count -eq 0) | Out-Null
            }
            # Noté, sans seuil : 40 captures sans éditeur (chaque vignette remplace la
            # précédente, la dernière est fermée). La mémoire suit-elle le nombre de captures ?
            Point '(15) mémoire après 40 captures sans éditeur' {
                $c = $null
                for ($i = 1; $i -le 40; $i++) { $c = Capturer $db "(15) capture seule $i" }
                Cliquer-Bouton $db $c 'fermer'
                $null = Attendre-Motif -Motif "vignette fermee $($c.Id) fermer$" -Dossier $db -DelaiS 5 -Quoi '(15) vignette fermée'
                $E::Poser($REPOS_X, $REPOS_Y)
                Start-Sleep -Seconds 10
                Noter-Mesure -Nom 'memoire privee 10 s apres 40 captures sans editeur' -Valeur (Memoire-Privee-Mo -Processus $script:proc) -Unite 'Mo'
                Noter-Mesure -Nom 'detail 10 s apres 40 captures sans editeur' -Valeur (Detail-Memoire $script:proc).Texte -Unite 'Mo'
            }
            Point '(15) arrêt' { Fermer-Cas $script:proc $db '(15)' }
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
    foreach ($a in $script:Auxiliaires) { try { $a.Kill() } catch { } }
    Terminer-Essai
}
