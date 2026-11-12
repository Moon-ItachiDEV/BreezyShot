# Version 0.4 « Confort », chantier Parcours : refaire la dernière zone et le retardateur,
# sur le vrai .exe, avec de vraies entrées clavier et souris (contrat 0.4 §11.3, essai 10).
#
# Cas A (retardateur 0, vignette coupée) :
# (1) démarrage : `raccourci derniere_zone: Ctrl+Maj+4` ; menu de l'icône lu par l'API des
#     menus : les quatre captures dans l'ordre, « Refaire la dernière zone » avec
#     « Ctrl + Maj + 4 » ;
# (2) Ctrl + Maj + 4 avant toute zone : `derniere zone: aucune, selection`, sélection
#     montrée en mode zone, Échap, rien d'écrit ;
# (3) zone tracée sur le témoin : PNG A ; Ctrl + Maj + 4 : `derniere zone` aux nombres de
#     `zone choisie`, `PhotoPrise derniere_zone`, écran figé et sélection jamais visibles
#     (guettés toutes les 2 ms), PNG B identique à A pixel pour pixel ;
# (4) par le vrai menu : zone retracée sur la place du menu (lue à une première ouverture),
#     un témoin dessous ; Ctrl + Maj + 4 (PNG K), puis clic sur « Refaire la dernière zone »
#     : `menu: capturer derniere_zone`, photo au moins 350 ms après (horodatages du
#     journal), PNG identique à K : le menu n'est pas sur la photo ;
# (12) mémoire privée au repos et chrono `selection prete` notés.
# Cas B (retardateur 3 s, vignette coupée) :
# (5) Ctrl + Maj + 2 : `DemandeDecompte plein_ecran`, `echap global reserve`, `bandeau
#     decompte`, fenêtre du bandeau visible, centrée sur la zone utile (± 2 px), sans
#     WS_EX_APPWINDOW, premier plan inchangé, disque peint (pixels à la couleur `accent`
#     ± 6) ; `fini en <ms>` entre 2950 et 3300 ; `bandeau decompte cache` avant `chrono gel
#     photo` ; le PNG, à la place de la fenêtre du décompte, égal (± 2) à la copie d'écran
#     faite avant l'appui ;
# (6) second appui pendant ce décompte : `capture deja en cours: appui ignore`, une photo ;
# (7) Échap pendant un décompte : `annule (echap)`, `Decompte -> Repos`, `echap global
#     libere`, aucune photo en 4 s, bandeau caché ; puis une capture normale ;
# (8) croix : clic réel au centre du rectangle de `bandeau zone` → `bandeau action <id>
#     annuler`, `annule (croix)`, premier plan inchangé ;
# (9) zone avec retardateur : décompte, puis sélection en mode zone, glisser, PNG juste ;
# (11) --quitter pendant un décompte : `annule (arret)`, aucune photo, `arret propre`.
# Cas C (retardateur 3 s, vignette allumée) :
# (10) depuis la vignette : `vignette fermee <id> capture` à l'entrée du décompte.
#
# Ce que la machine GitHub ne peut pas vérifier : plusieurs écrans et échelles (dernière
# zone après un écran débranché, place du décompte à 150 %) ; l'aspect du disque ; un menu
# d'une autre application resté ouvert pendant le décompte ; un vrai verrouillage de session.
#
# Tout l'essai tourne avec BREEZYSHOT_ESSAI_CAPTURABLE=1 : le bandeau, l'écran figé et la
# sélection ne sont pas exclus des captures, sinon (4) et (5) passeraient sans rien prouver.
# Hors Windows : seul le démarrage est vérifié.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '10-confort' -Exe $Exe -Sortie $Sortie | Out-Null

$script:Auxiliaires = [System.Collections.Generic.List[object]]::new()
$script:Dossiers = [System.Collections.Generic.List[string]]::new()
$script:Pwsh = (Get-Process -Id $PID).Path

# --- Aides (même forme que les essais 04 à 08) -------------------------------------------

function Dossier-Cas([string]$Nom) {
    $d = Join-Path $script:Essai.Dossier $Nom
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $script:Dossiers.Add($d)
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

# Index (dans le journal) de la première ligne qui correspond à $Motif à partir de l'index
# $Depuis ; -1 sinon.
function Index-Motif([string]$Motif, [string]$Dossier, [int]$Depuis = 0) {
    $lignes = Lignes-Journal $Dossier
    for ($i = $Depuis; $i -lt $lignes.Count; $i++) { if ($lignes[$i] -match $Motif) { return $i } }
    return -1
}

# Attend qu'au moins $Fois lignes correspondent à $Motif ; rend la dernière, ou lève une
# erreur qui dit quoi, quel motif, combien de fois, et les dernières lignes vues.
function Attendre-Motif([string]$Motif, [string]$Dossier, [double]$DelaiS, [string]$Quoi, [int]$Fois = 1) {
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        if ((Compter-Motif -Motif $Motif -Dossier $Dossier) -ge $Fois) {
            return (Derniere-Ligne -Motif $Motif -Dossier $Dossier)
        }
        Start-Sleep -Milliseconds 20
    } while ([DateTime]::UtcNow -lt $limite)
    $vu = Compter-Motif -Motif $Motif -Dossier $Dossier
    $fin = (Lignes-Journal $Dossier | Select-Object -Last 8) -join ' | '
    throw "$Quoi : rien dans le journal en $DelaiS s (motif « $Motif », attendu $Fois fois, vu $vu ; fin du journal : $fin)"
}

# Attend la ligne suivante (après $Vus) : rend la dernière qui correspond.
function Attendre-Suivante([string]$Motif, [string]$Dossier, [int]$Vus, [string]$Quoi, [double]$DelaiS = 5) {
    return (Attendre-Motif -Motif $Motif -Dossier $Dossier -DelaiS $DelaiS -Quoi $Quoi -Fois ($Vus + 1))
}

# L'horodatage d'une ligne du journal (« 2026-11-11T18:40:12.345+01:00 INFO … »).
function Horodatage([string]$Ligne) {
    $t = ($Ligne -split ' ', 2)[0]
    return [DateTimeOffset]::Parse($t, [cultureinfo]::InvariantCulture)
}

# Réglages du cas : Ctrl+Maj+1 (zone), Ctrl+Maj+2 (écran), Ctrl+Maj+4 (dernière zone),
# copie seule, retardateur et vignette au choix. Version 3 complète : le fichier n'est pas
# réécrit.
function Ecrire-Reglages([string]$Dossier, [int]$Retardateur, [bool]$Vignette) {
    $r = [ordered]@{
        version                   = 3
        raccourcis                = [ordered]@{ zone = @('Ctrl+Maj+1'); plein_ecran = @('Ctrl+Maj+2'); fenetre = @(); derniere_zone = @('Ctrl+Maj+4') }
        copier_apres_capture      = $true
        enregistrer_apres_capture = $false
        dossier_enregistrement    = $null
        vignette                  = [ordered]@{ afficher = $Vignette; duree_s = 6; coin = 'bas_gauche' }
        selection                 = [ordered]@{ loupe = $false }
        capture                   = [ordered]@{ retardateur_s = $Retardateur; pointeur = $false }
        demarrer_avec_systeme     = $false
        modele_nom                = 'BreezyShot {date} à {heure}'
        animations                = 'systeme'
        theme                     = 'systeme'
    }
    $d = Join-Path $Dossier 'reglages'
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $r | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $d 'reglages.json') -Encoding utf8
}

function Demarrer-Cas([string]$Dossier, [string]$Quoi) {
    $env:BREEZYSHOT_ESSAI_CAPTURABLE = '1'
    try { $p = Demarrer-App -Dossier $Dossier }
    finally { Remove-Item Env:BREEZYSHOT_ESSAI_CAPTURABLE -ErrorAction SilentlyContinue }
    $null = Attendre-Motif -Motif 'application +pret$' -Dossier $Dossier -DelaiS 30 -Quoi "$Quoi : démarrage"
    $null = Attendre-Motif -Motif 'ecran fige pret$' -Dossier $Dossier -DelaiS 1 -Quoi "$Quoi : écran figé préparé"
    $null = Attendre-Motif -Motif 'selection prete a servir$' -Dossier $Dossier -DelaiS 1 -Quoi "$Quoi : sélection créée"
    $null = Attendre-Motif -Motif 'bandeau pret$' -Dossier $Dossier -DelaiS 20 -Quoi "$Quoi : page du bandeau"
    $null = Attendre-Motif -Motif 'selection chargee$' -Dossier $Dossier -DelaiS 20 -Quoi "$Quoi : page de la sélection"
    return $p
}

function Fermer-Cas($Processus, [string]$Dossier, [string]$Quoi) {
    $arrets = Compter-Motif -Motif 'arret propre$' -Dossier $Dossier
    $q = Quitter-App -DelaiS 10 -Dossier $Dossier
    if (-not $q.Fini) { throw "$Quoi : --quitter n'a pas répondu en 10 s" }
    if (-not $Processus.WaitForExit(10000)) { throw "$Quoi : l'app ne s'est pas fermée en 10 s" }
    $null = Attendre-Motif -Motif 'arret propre$' -Dossier $Dossier -DelaiS 3 -Quoi "$Quoi : arrêt" -Fois ($arrets + 1)
    Verifier "$Quoi : --quitter → arret propre" $true | Out-Null
}

# Une fonction rend ses éléments un à un : chaque appel est entouré de @().
function Fichiers-Png([string]$Dossier) {
    if (-not (Test-Path -LiteralPath $Dossier)) { return @() }
    return @(Get-ChildItem -LiteralPath $Dossier -File -Filter '*.png' | Sort-Object LastWriteTimeUtc)
}

try {
    if (-not $IsWindows) {
        $commun = Dossier-Cas 'commun'
        Ecrire-Reglages -Dossier $commun -Retardateur 3 -Vignette $false
        $p = Demarrer-Cas -Dossier $commun -Quoi 'commun'
        Verifier '(1) raccourci derniere_zone: Ctrl+Maj+4' ((Compter-Motif -Motif 'raccourci derniere_zone: Ctrl\+Maj\+4$' -Dossier $commun) -eq 1) | Out-Null
        Fermer-Cas $p $commun 'commun'
        Write-Host '[saute] (1) menu à (12) : sautés hors Windows'
    }
    else {
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms
        Add-Type -Namespace BreezyShotEssai -Name Confort -MemberDefinition @'
[DllImport("user32.dll")]
static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
[DllImport("user32.dll")]
static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
[DllImport("user32.dll")]
public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SetProcessDpiAwarenessContext(IntPtr contexte);
[DllImport("user32.dll")]
public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")]
static extern uint MapVirtualKey(uint code, uint type);
[DllImport("user32.dll")]
public static extern bool IsWindowVisible(IntPtr h);
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
[DllImport("user32.dll")]
static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll", CharSet = CharSet.Unicode)]
static extern int GetClassName(IntPtr h, System.Text.StringBuilder nom, int taille);
delegate bool Rappel(IntPtr h, IntPtr p);
[DllImport("user32.dll")]
static extern bool EnumWindows(Rappel f, IntPtr p);
[DllImport("user32.dll")]
public static extern int GetMenuItemCount(IntPtr menu);
[DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "GetMenuStringW")]
static extern int GetMenuString(IntPtr menu, uint item, System.Text.StringBuilder texte, int taille, uint drapeaux);
[DllImport("user32.dll")]
public static extern bool GetMenuItemRect(IntPtr h, IntPtr menu, uint item, out RECT r);

const uint ETENDUE = 0x1, RELACHEE = 0x2;
static uint Drapeaux(byte vk) { return (vk == 0x2C || vk == 0x2E) ? ETENDUE : 0u; }
static void Appuyer(byte[] vks) { foreach (var vk in vks) keybd_event(vk, (byte)MapVirtualKey(vk, 0), Drapeaux(vk), UIntPtr.Zero); }
static void Relacher(byte[] vks) { for (int i = vks.Length - 1; i >= 0; i--) keybd_event(vks[i], (byte)MapVirtualKey(vks[i], 0), Drapeaux(vks[i]) | RELACHEE, UIntPtr.Zero); }
// Appuie les touches dans l'ordre, puis les relâche dans l'ordre inverse.
public static void Touches(byte[] vks) { Appuyer(vks); System.Threading.Thread.Sleep(60); Relacher(vks); }
// Appuie les touches et guette les fenêtres données toutes les 2 ms pendant `ms` (appui
// compris ; attente active : `Sleep(2)` dormirait 15 ms au pas de l'horloge de Windows).
// Rend { tours de guet, tours où l'une d'elles était visible }.
public static int[] TouchesEtGuet(byte[] vks, IntPtr[] fenetres, int ms) {
    var chrono = System.Diagnostics.Stopwatch.StartNew();
    int tours = 0, vues = 0; bool relache = false;
    Appuyer(vks);
    while (chrono.ElapsedMilliseconds < ms) {
        if (!relache && chrono.ElapsedMilliseconds >= 60) { Relacher(vks); relache = true; }
        tours++;
        foreach (var h in fenetres) { if (IsWindowVisible(h)) { vues++; break; } }
        long suivant = (long)tours * 2;
        while (chrono.ElapsedMilliseconds < suivant) System.Threading.Thread.SpinWait(200);
    }
    if (!relache) Relacher(vks);
    return new int[] { tours, vues };
}
static void Poser(int x, int y) { SetCursorPos(x, y); mouse_event(0x1, 0, 0, 0, UIntPtr.Zero); }
public static void Glisser(int x1, int y1, int x2, int y2) {
    Poser(x1, y1); System.Threading.Thread.Sleep(120);
    mouse_event(0x2, 0, 0, 0, UIntPtr.Zero); System.Threading.Thread.Sleep(80);
    for (int i = 1; i <= 16; i++) { Poser(x1 + (x2 - x1) * i / 16, y1 + (y2 - y1) * i / 16); System.Threading.Thread.Sleep(30); }
    Poser(x2, y2); System.Threading.Thread.Sleep(150);
    mouse_event(0x4, 0, 0, 0, UIntPtr.Zero);
}
public static void Cliquer(int x, int y) {
    Poser(x, y); System.Threading.Thread.Sleep(120);
    mouse_event(0x2, 0, 0, 0, UIntPtr.Zero); System.Threading.Thread.Sleep(60);
    mouse_event(0x4, 0, 0, 0, UIntPtr.Zero);
}
// Fenêtres de premier niveau d'un processus, d'une classe donnée.
public static IntPtr[] Fenetres(int pid, string classe) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, p) => {
        uint proprio; GetWindowThreadProcessId(h, out proprio);
        if ((int)proprio != pid) return true;
        var nom = new System.Text.StringBuilder(256);
        GetClassName(h, nom, nom.Capacity);
        if (nom.ToString() == classe) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
// Fenêtres visibles de cette classe, tous processus confondus.
public static IntPtr[] Visibles(string classe) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, p) => {
        var nom = new System.Text.StringBuilder(256);
        if (IsWindowVisible(h) && GetClassName(h, nom, nom.Capacity) > 0 && nom.ToString() == classe) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
public static IntPtr MenuDe(IntPtr fenetre) { return SendMessage(fenetre, 0x01E1, IntPtr.Zero, IntPtr.Zero); }
// Texte complet de chaque position (tabulation et raccourci compris) ; "" : séparateur.
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
// Les images arrivent en octets BGRA (LockBits, Format32bppArgb) : aucune référence à
// System.Drawing dans ce code (comme l'essai 04).
// Compare la région (ax, ay, l, h) de `a` à la région (bx, by, l, h) de `b`, canal par canal
// à `tol` près. Rend « total N » puis les `max` premiers écarts.
public static string[] Comparer(byte[] oa, int sa, int ax, int ay, byte[] ob, int sb, int bx, int by, int l, int h, int tol, int max) {
    var r = new System.Collections.Generic.List<string>();
    int total = 0;
    for (int y = 0; y < h; y++) for (int x = 0; x < l; x++) {
        int i = (ay + y) * sa + 4 * (ax + x), j = (by + y) * sb + 4 * (bx + x);
        bool ok = true;
        for (int c = 0; c < 3; c++) if (Math.Abs(oa[i + c] - ob[j + c]) > tol) ok = false;
        if (!ok) {
            total++;
            if (r.Count < max) r.Add(string.Format("{0},{1} {2},{3},{4} contre {5},{6},{7}", x, y, oa[i + 2], oa[i + 1], oa[i], ob[j + 2], ob[j + 1], ob[j]));
        }
    }
    r.Insert(0, "total " + total);
    return r.ToArray();
}
// Pixels à la couleur (r, v, b) à `tol` près, canal par canal.
public static int Compter(byte[] o, int s, int l, int h, int rr, int vv, int bb, int tol) {
    int n = 0;
    for (int y = 0; y < h; y++) for (int x = 0; x < l; x++) {
        int i = y * s + 4 * x;
        if (Math.Abs(o[i + 2] - rr) <= tol && Math.Abs(o[i + 1] - vv) <= tol && Math.Abs(o[i] - bb) <= tol) n++;
    }
    return n;
}
// Compare une image aux quatre carrés du témoin (r,v,b : haut gauche, haut droite, bas
// gauche, bas droite). Rend « total N » puis les `max` premiers écarts.
public static string[] Ecarts(byte[] o, int s, int l, int h, int[] c, int max) {
    var r = new System.Collections.Generic.List<string>();
    int total = 0, mx = l / 2, my = h / 2;
    for (int y = 0; y < h; y++) for (int x = 0; x < l; x++) {
        int q = (y < my ? 0 : 2) + (x < mx ? 0 : 1), i = y * s + 4 * x;
        if (o[i + 2] != c[3 * q] || o[i + 1] != c[3 * q + 1] || o[i] != c[3 * q + 2]) {
            total++;
            if (r.Count < max) r.Add(string.Format("{0},{1} attendu {2},{3},{4} obtenu {5},{6},{7}", x, y, c[3 * q], c[3 * q + 1], c[3 * q + 2], o[i + 2], o[i + 1], o[i]));
        }
    }
    r.Insert(0, "total " + total);
    return r.ToArray();
}
'@
        $C = [BreezyShotEssai.Confort]
        try { $null = $C::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }

        $CTRL_MAJ_1 = [byte[]]@(0x11, 0x10, 0x31)
        $CTRL_MAJ_2 = [byte[]]@(0x11, 0x10, 0x32)
        $CTRL_MAJ_4 = [byte[]]@(0x11, 0x10, 0x34)
        $ECHAP = [byte[]]@(0x1B)
        $GWL_EXSTYLE = -20
        $COTE = 200
        $COULEURS = [int[]]@(220, 40, 40, 40, 180, 60, 40, 80, 220, 240, 200, 30)
        $texteCouleurs = '220,40,40;40,180,60;40,80,220;240,200,30'
        # La couleur `accent` des jetons (thème clair, puis sombre).
        $ACCENTS = @(@(0x00, 0x67, 0xC0), @(0x4C, 0xC2, 0xFF))
        $utile = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        $ecranP = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        # Le témoin, en haut à gauche de la zone utile : loin du centre (décompte) et du coin
        # bas droit (menu de l'icône).
        $TX = [int]($utile.X + 40)
        $TY = [int]($utile.Y + 40)
        Write-Host "écran $ecranP ; zone utile $utile ; témoin $TX,$TY ${COTE}x$COTE"

        function Lancer-Temoin([int]$X, [int]$Y, [int]$Cote) {
            $f = Join-Path $script:Essai.Dossier ("temoin-" + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.txt')
            # -CacherConsole : la console de l'aide ne couvre rien de ce que l'essai clique
            # ou photographie (pas de -WindowStyle Hidden : il cacherait le témoin).
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -ArgumentList @(
                '-NoProfile', '-File', (Join-Path $PSScriptRoot 'temoin.ps1'),
                '-X', $X, '-Y', $Y, '-Cote', $Cote, '-Couleurs', $texteCouleurs, '-Fichier', $f, '-CacherConsole')
            $script:Auxiliaires.Add($aide)
            $limite = [DateTime]::UtcNow.AddSeconds(30)
            while (-not (Test-Path -LiteralPath $f)) {
                if ([DateTime]::UtcNow -ge $limite) { throw "fenêtre témoin : aucun HWND en 30 s" }
                Start-Sleep -Milliseconds 100
            }
            Start-Sleep -Milliseconds 700   # composée, console cachée
            return [pscustomobject]@{ Hwnd = [IntPtr][long](Get-Content -LiteralPath $f -Raw).Trim(); Processus = $aide }
        }

        function Fermer-Auxiliaires {
            foreach ($a in $script:Auxiliaires) {
                try { if (-not $a.HasExited) { $a.Kill(); $null = $a.WaitForExit(5000) } } catch { }
            }
            $script:Auxiliaires.Clear()
        }

        function Hwnd-Journal([string]$Motif, [string]$Dossier, [string]$Quoi) {
            $l = Attendre-Motif -Motif "$Motif (\d+)$" -Dossier $Dossier -DelaiS 5 -Quoi $Quoi
            if ($l -notmatch "$Motif (\d+)$") { throw "$Quoi : ligne illisible : $l" }
            return [IntPtr][long]$Matches[1]
        }

        function Rect-Fenetre([IntPtr]$h) {
            $r = [BreezyShotEssai.Confort+RECT]::new()
            [void]$C::GetWindowRect($h, [ref]$r)
            return [pscustomobject]@{ X = $r.Left; Y = $r.Top; L = $r.Right - $r.Left; H = $r.Bottom - $r.Top }
        }

        function Lire-Png([string]$Chemin) {
            $octets = [System.IO.File]::ReadAllBytes($Chemin)
            $flux = [System.IO.MemoryStream]::new($octets)
            try {
                $img = [System.Drawing.Bitmap]::new($flux)
                try { return [System.Drawing.Bitmap]::new($img) }
                finally { $img.Dispose() }
            }
            finally { $flux.Dispose() }
        }

        # Copie d'écran d'un rectangle (pixels réels).
        function Copie-Ecran([int]$X, [int]$Y, [int]$Largeur, [int]$Hauteur) {
            $b = [System.Drawing.Bitmap]::new($Largeur, $Hauteur, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
            $g = [System.Drawing.Graphics]::FromImage($b)
            try { $g.CopyFromScreen($X, $Y, 0, 0, [System.Drawing.Size]::new($Largeur, $Hauteur)) }
            finally { $g.Dispose() }
            return $b
        }

        # Octets BGRA d'une image (Format32bppArgb).
        function Octets-Image([System.Drawing.Bitmap]$Image) {
            $r = [System.Drawing.Rectangle]::new(0, 0, $Image.Width, $Image.Height)
            $d = $Image.LockBits($r, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
            try {
                $o = [byte[]]::new($d.Stride * $Image.Height)
                [System.Runtime.InteropServices.Marshal]::Copy($d.Scan0, $o, 0, $o.Length)
                return [pscustomobject]@{ Octets = $o; Stride = $d.Stride }
            }
            finally { $Image.UnlockBits($d) }
        }

        # Compare la région ($X, $Y, $Largeur, $Hauteur) de deux images (même place dans
        # chacune), à $Tol près par canal. Rend « total N » et les premiers écarts ; « total
        # -1 » si la région sort d'une image.
        function Comparer-Images($A, $B, [int]$X, [int]$Y, [int]$Largeur, [int]$Hauteur, [int]$Tol) {
            foreach ($i in @($A, $B)) {
                if ($X -lt 0 -or $Y -lt 0 -or ($X + $Largeur) -gt $i.Width -or ($Y + $Hauteur) -gt $i.Height) {
                    return @('total -1', "région $X,$Y ${Largeur}x$Hauteur hors d'une image $($i.Width)x$($i.Height)")
                }
            }
            $oa = Octets-Image $A; $ob = Octets-Image $B
            return $C::Comparer($oa.Octets, $oa.Stride, $X, $Y, $ob.Octets, $ob.Stride, $X, $Y, $Largeur, $Hauteur, $Tol, 8)
        }

        function Ecarts-Temoin($Image, [int]$Max) {
            $o = Octets-Image $Image
            return $C::Ecarts($o.Octets, $o.Stride, $Image.Width, $Image.Height, $COULEURS, $Max)
        }

        function Garder-Image([string]$Nom, $Image) {
            $Image.Save((Join-Path $script:Essai.Sortie ("10-" + ($Nom -replace '[^\w]+', '-') + '.png')), [System.Drawing.Imaging.ImageFormat]::Png)
        }

        # Attend un PNG de plus dans « Récents » (le n-ième) et le rend (dernier écrit).
        function Attendre-Png([string]$Recents, [int]$Avant, [string]$Quoi) {
            $limite = [DateTime]::UtcNow.AddSeconds(10)
            do {
                $pngs = @(Fichiers-Png $Recents)
                if ($pngs.Count -gt $Avant) { Start-Sleep -Milliseconds 200; return (Lire-Png (@(Fichiers-Png $Recents))[-1].FullName) }
                Start-Sleep -Milliseconds 50
            } while ([DateTime]::UtcNow -lt $limite)
            throw "$Quoi : aucun PNG de plus dans « Récents » en 10 s (avant $Avant, maintenant $(@(Fichiers-Png $Recents).Count))"
        }

        # Le témoin, pixel pour pixel, dans une image de ${COTE}x$COTE.
        function Verifier-Temoin([string]$Nom, $Image) {
            if (-not (Verifier "$Nom : taille ${COTE}x$COTE" (($Image.Width -eq $COTE) -and ($Image.Height -eq $COTE)) "obtenu $($Image.Width)x$($Image.Height)")) {
                Garder-Image $Nom $Image; return
            }
            $e = Ecarts-Temoin $Image 8
            if ($e[0] -ne 'total 0') { Garder-Image $Nom $Image }
            Verifier "$Nom : chaque pixel à la couleur du témoin" ($e[0] -eq 'total 0') ($e -join ' ; ') | Out-Null
        }

        # Deux images identiques pixel pour pixel (tolérance 0).
        function Verifier-Identiques([string]$Nom, $A, $B) {
            if (-not (Verifier "$Nom : même taille" (($A.Width -eq $B.Width) -and ($A.Height -eq $B.Height)) "$($A.Width)x$($A.Height) contre $($B.Width)x$($B.Height)")) {
                Garder-Image "$Nom-a" $A; Garder-Image "$Nom-b" $B; return
            }
            $e = Comparer-Images $A $B 0 0 $A.Width $A.Height 0
            if ($e[0] -ne 'total 0') { Garder-Image "$Nom-a" $A; Garder-Image "$Nom-b" $B }
            Verifier "$Nom : identiques pixel pour pixel" ($e[0] -eq 'total 0') ($e -join ' ; ') | Out-Null
        }

        # Le menu de l'icône, ouvert comme par un clic droit, la souris posée en $Souris
        # (le menu s'ouvre à la souris). Rend { Fenetre, Menu, Textes, Rect }.
        function Ouvrir-Menu([int]$IdProc, [int[]]$Souris, [string]$Quoi) {
            $icones = @($C::Fenetres($IdProc, 'tray_icon_app'))
            if ($icones.Count -lt 1) { throw "$Quoi : aucune fenêtre tray_icon_app" }
            $avant = @($C::Visibles('#32768') | ForEach-Object { [long]$_ })
            [void]$C::SetCursorPos($Souris[0], $Souris[1])
            Start-Sleep -Milliseconds 100
            # WM_USER_TRAYICON (6002) avec WM_RBUTTONUP (0x205) : ce que Windows envoie au clic droit.
            foreach ($h in $icones) { [void]$C::PostMessage($h, 6002, [IntPtr]::Zero, [IntPtr]0x205) }
            $limite = [DateTime]::UtcNow.AddSeconds(3)
            $lus = 'aucune fenêtre #32768 nouvelle'
            do {
                foreach ($f in @($C::Visibles('#32768') | Where-Object { $avant -notcontains [long]$_ })) {
                    $hm = $C::MenuDe($f)
                    if ($hm -eq [IntPtr]::Zero) { $lus = "fenêtre $f sans HMENU"; continue }
                    $textes = $C::Textes($hm)
                    if ($null -eq $textes) { $lus = "HMENU $hm illisible"; continue }
                    Start-Sleep -Milliseconds 300   # fondu d'ouverture fini
                    return [pscustomobject]@{ Fenetre = $f; Menu = $hm; Textes = @($textes); Rect = (Rect-Fenetre $f) }
                }
                Start-Sleep -Milliseconds 30
            } while ([DateTime]::UtcNow -lt $limite)
            throw "$Quoi : le menu de l'icône ne s'ouvre pas en 3 s ($lus)"
        }

        function Fermer-Menu($Menu) {
            [BreezyShotEssai.Confort]::Touches($ECHAP)
            $limite = [DateTime]::UtcNow.AddSeconds(2)
            while (($C::IsWindowVisible($Menu.Fenetre)) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 30 }
        }

        # Une capture de zone au clavier (Ctrl+Maj+1) : sélection prête, glisser. Rend la
        # ligne `zone choisie`.
        function Tracer-Zone([string]$D, [int]$X, [int]$Y, [int]$L, [int]$H, [string]$Quoi) {
            $pretes = Compter-Motif -Motif 'chrono selection prete \d+' -Dossier $D
            $zones = Compter-Motif -Motif 'zone choisie ' -Dossier $D
            $C::Touches($CTRL_MAJ_1)
            $null = Attendre-Suivante 'chrono selection prete \d+' $D $pretes "$Quoi : sélection prête"
            Start-Sleep -Milliseconds 150
            $C::Glisser($X, $Y, $X + $L - 1, $Y + $H - 1)
            return (Attendre-Suivante 'zone choisie ' $D $zones "$Quoi : zone choisie")
        }

        try {
            $t = Lancer-Temoin $TX $TY $COTE
            Write-Host "témoin $($t.Hwnd)"

            # ===== Cas A : retardateur 0 ============================================================
            try {
                $da = Dossier-Cas 'a'
                Ecrire-Reglages -Dossier $da -Retardateur 0 -Vignette $false
                $p = Demarrer-Cas -Dossier $da -Quoi '(A)'
                $recents = Join-Path $da 'local/Recents'
                $sel = Hwnd-Journal 'selection fenetre' $da 'HWND de la sélection'
                $fige = Hwnd-Journal 'ecran fige fenetre' $da 'HWND de l''écran figé'

                # (1) Démarrage et menu.
                Verifier '(1) raccourci derniere_zone: Ctrl+Maj+4' ((Compter-Motif -Motif 'raccourci derniere_zone: Ctrl\+Maj\+4$' -Dossier $da) -eq 1) | Out-Null
                $SOURIS_MENU = @(($utile.Right - 60), ($utile.Bottom - 10))
                $m = Ouvrir-Menu $p.Id $SOURIS_MENU '(1)'
                $attendus = @("Capturer une zone`tCtrl + Maj + 1", "Capturer l'écran`tCtrl + Maj + 2", 'Capturer une fenêtre', "Refaire la dernière zone`tCtrl + Maj + 4")
                $lus = @($m.Textes | Select-Object -First 4)
                $vus = ($m.Textes | ForEach-Object { "« $($_ -replace "`t", ' ⇥ ') »" }) -join ', '
                Verifier '(1) menu : les quatre captures dans l''ordre, « Refaire la dernière zone » avec Ctrl + Maj + 4' (($lus.Count -eq 4) -and (($lus -join '|') -ceq ($attendus -join '|'))) $vus | Out-Null
                $rectMenu = $m.Rect
                Write-Host "menu en $($rectMenu.X),$($rectMenu.Y) $($rectMenu.L)x$($rectMenu.H)"
                Fermer-Menu $m

                # (12) Mémoire au repos.
                Start-Sleep -Seconds 2
                Noter-Mesure -Nom 'mémoire privée au repos (A)' -Valeur (Memoire-Privee-Mo -Processus (Get-Process -Id $p.Id)) -Unite 'Mo'

                # (2) Avant toute zone : la sélection s'ouvre, Échap, rien d'écrit.
                $montrees = Compter-Motif -Motif 'selection montree \d+ ' -Dossier $da
                $repos = Compter-Motif -Motif 'etat \w+ -> Repos' -Dossier $da
                $C::Touches($CTRL_MAJ_4)
                $null = Attendre-Motif -Motif 'raccourci presse: derniere_zone$' -Dossier $da -DelaiS 5 -Quoi '(2) raccourci'
                $null = Attendre-Motif -Motif 'derniere zone: aucune, selection$' -Dossier $da -DelaiS 5 -Quoi '(2) repli'
                $sm = Attendre-Suivante 'selection montree \d+ ' $da $montrees '(2) sélection montrée'
                Verifier '(2) derniere zone: aucune, selection ; sélection montrée en mode zone' ($sm -match ' mode zone ') $sm | Out-Null
                $null = Attendre-Motif -Motif 'chrono selection prete \d+' -Dossier $da -DelaiS 5 -Quoi '(2) sélection prête'
                $prete = Derniere-Ligne -Motif 'chrono selection prete (\d+)' -Dossier $da
                if ($prete -match 'chrono selection prete (\d+)') { Noter-Mesure -Nom 'chrono selection prete (repli de la dernière zone)' -Valeur ([int]$Matches[1]) -Unite 'ms' }
                Start-Sleep -Milliseconds 150
                $C::Touches($ECHAP)
                $null = Attendre-Suivante 'etat \w+ -> Repos' $da $repos '(2) Échap'
                Start-Sleep -Milliseconds 300
                Verifier '(2) Échap : rien d''écrit, écran figé et sélection cachés' ((@(Fichiers-Png $recents).Count -eq 0) -and -not ($C::IsWindowVisible($sel)) -and -not ($C::IsWindowVisible($fige))) "PNG $(@(Fichiers-Png $recents).Count)" | Out-Null

                # (3) Zone sur le témoin, puis Ctrl+Maj+4 sans rien montrer.
                $n = @(Fichiers-Png $recents).Count
                $zc = Tracer-Zone $da $TX $TY $COTE $COTE '(3) zone'
                $a = Attendre-Png $recents $n '(3) PNG A'
                Verifier '(3) zone choisie = le témoin' ($zc -match "zone choisie $TX,$TY ${COTE}x$COTE$") $zc | Out-Null
                Verifier '(3) derniere zone retenue = la zone choisie' ((Compter-Motif -Motif "derniere zone retenue $TX,$TY ${COTE}x$COTE$" -Dossier $da) -eq 1) | Out-Null
                Verifier-Temoin '(3) PNG A' $a
                Start-Sleep -Milliseconds 300
                $n = @(Fichiers-Png $recents).Count
                $refaites = Compter-Motif -Motif 'derniere zone -?\d+,' -Dossier $da
                $guet = $C::TouchesEtGuet($CTRL_MAJ_4, [IntPtr[]]@($sel, $fige), 1500)
                $l = Attendre-Suivante 'derniere zone -?\d+,' $da $refaites '(3) derniere zone'
                $b = Attendre-Png $recents $n '(3) PNG B'
                Verifier '(3) derniere zone aux nombres de zone choisie' ($l -match "derniere zone $TX,$TY ${COTE}x$COTE$") $l | Out-Null
                Verifier '(3) etat Gel -> Decoupe (PhotoPrise derniere_zone)' ((Compter-Motif -Motif 'etat Gel -> Decoupe \(PhotoPrise derniere_zone\)$' -Dossier $da) -eq 1) | Out-Null
                Verifier '(3) écran figé et sélection jamais visibles (guet toutes les 2 ms pendant 1,5 s)' (($guet[0] -ge 500) -and ($guet[1] -eq 0)) "tours $($guet[0]), visibles $($guet[1])" | Out-Null
                Verifier-Identiques '(3) PNG B contre A' $b $a
                $a.Dispose(); $b.Dispose()

                # (4) Par le vrai menu : zone sur la place du menu, témoin dessous.
                $cote4 = [math]::Max($rectMenu.L, $rectMenu.H) + 16
                $x4 = [math]::Max($utile.X, [math]::Min($rectMenu.X - 8, $utile.Right - $cote4))
                $y4 = [math]::Max($utile.Y, [math]::Min($rectMenu.Y - 8, $utile.Bottom - $cote4))
                $t4 = Lancer-Temoin $x4 $y4 $cote4
                $n = @(Fichiers-Png $recents).Count
                $zc4 = Tracer-Zone $da $x4 $y4 $cote4 $cote4 '(4) zone du menu'
                $z4 = Attendre-Png $recents $n '(4) PNG de la zone'
                Verifier '(4) zone retracée sur la place du menu' ($zc4 -match "zone choisie $x4,$y4 ${cote4}x$cote4$") "$zc4 (menu $($rectMenu.X),$($rectMenu.Y) $($rectMenu.L)x$($rectMenu.H))" | Out-Null
                $z4.Dispose()
                Start-Sleep -Milliseconds 300
                $n = @(Fichiers-Png $recents).Count
                $C::Touches($CTRL_MAJ_4)
                $k = Attendre-Png $recents $n '(4) PNG au clavier'
                Start-Sleep -Milliseconds 300
                $m = Ouvrir-Menu $p.Id $SOURIS_MENU '(4)'
                $pos = [array]::IndexOf([string[]]$m.Textes, "Refaire la dernière zone`tCtrl + Maj + 4")
                if (-not (Verifier '(4) entrée « Refaire la dernière zone » au menu' ($pos -ge 0) (($m.Textes) -join ' | '))) { throw '(4) pas d''entrée' }
                $ri = [BreezyShotEssai.Confort+RECT]::new()
                if (-not $C::GetMenuItemRect([IntPtr]::Zero, $m.Menu, [uint32]$pos, [ref]$ri)) { throw '(4) rectangle de l''entrée illisible' }
                # Le menu est bien au-dessus du témoin, dans la zone : sinon le point ne prouve rien.
                $vue = Copie-Ecran $x4 $y4 $cote4 $cote4
                $ecartsMenu = Ecarts-Temoin $vue 1
                $vue.Dispose()
                Verifier '(4) le menu ouvert couvre le témoin dans la zone' ($ecartsMenu[0] -ne 'total 0') ($ecartsMenu -join ' ; ') | Out-Null
                $n = @(Fichiers-Png $recents).Count
                $menus = Compter-Motif -Motif 'menu: capturer derniere_zone$' -Dossier $da
                $refaites = Compter-Motif -Motif 'derniere zone -?\d+,' -Dossier $da
                $C::Cliquer([int](($ri.Left + $ri.Right) / 2), [int](($ri.Top + $ri.Bottom) / 2))
                $lm = Attendre-Suivante 'menu: capturer derniere_zone$' $da $menus '(4) menu: capturer derniere_zone'
                $lz = Attendre-Suivante 'derniere zone -?\d+,' $da $refaites '(4) derniere zone'
                $mp = Attendre-Png $recents $n '(4) PNG par le menu'
                $ecart = [int]((Horodatage $lz) - (Horodatage $lm)).TotalMilliseconds
                Verifier '(4) photo au moins 350 ms après le clic' ($ecart -ge 350) "$ecart ms entre « $lm » et « $lz »" | Out-Null
                Verifier-Identiques '(4) PNG par le menu contre PNG au clavier (le menu n''est pas sur la photo)' $mp $k
                $mp.Dispose(); $k.Dispose()
                try { $t4.Processus.Kill() } catch { }

                Fermer-Cas $p $da '(A)'
            }
            catch { Verifier "(A) déroulement" $false "$_" | Out-Null }
            finally { Arreter-Tout }

            # ===== Cas B : retardateur 3 s ==========================================================
            try {
                $db = Dossier-Cas 'b'
                Ecrire-Reglages -Dossier $db -Retardateur 3 -Vignette $false
                $p = Demarrer-Cas -Dossier $db -Quoi '(B)'
                $recents = Join-Path $db 'local/Recents'
                $bandeau = Hwnd-Journal 'bandeau fenetre' $db 'HWND du bandeau'

                # (5) et (6) Plein écran avec retardateur.
                Start-Sleep -Milliseconds 500
                $avantAppui = Copie-Ecran $ecranP.X $ecranP.Y $ecranP.Width $ecranP.Height
                $premier = $C::GetForegroundWindow()
                $n = @(Fichiers-Png $recents).Count
                $debut = @(Lignes-Journal $db).Count
                $ignores = Compter-Motif -Motif 'capture deja en cours: appui ignore$' -Dossier $db
                $C::Touches($CTRL_MAJ_2)
                $ld = Attendre-Motif -Motif 'decompte (\d+) debut 3 s plein_ecran$' -Dossier $db -DelaiS 5 -Quoi '(5) decompte debut'
                $null = $ld -match 'decompte (\d+) debut'
                $num = $Matches[1]
                Verifier '(5) etat Repos -> Decompte (DemandeDecompte plein_ecran)' ((Compter-Motif -Motif 'etat Repos -> Decompte \(DemandeDecompte plein_ecran\)$' -Dossier $db) -eq 1) | Out-Null
                $null = Attendre-Motif -Motif 'echap global reserve$' -Dossier $db -DelaiS 2 -Quoi '(5) echap global reserve'
                $lb = Attendre-Motif -Motif 'bandeau decompte (\d+) ecran ' -Dossier $db -DelaiS 2 -Quoi '(5) bandeau decompte'
                Start-Sleep -Milliseconds 1200
                $rb = Rect-Fenetre $bandeau
                $cx = $rb.X + $rb.L / 2; $cy = $rb.Y + $rb.H / 2
                $ux = $utile.X + $utile.Width / 2; $uy = $utile.Y + $utile.Height / 2
                Verifier '(5) fenêtre du bandeau visible, centrée sur la zone utile (± 2 px)' (($C::IsWindowVisible($bandeau)) -and ([math]::Abs($cx - $ux) -le 2) -and ([math]::Abs($cy - $uy) -le 2)) "fenêtre $($rb.X),$($rb.Y) $($rb.L)x$($rb.H), centre $cx,$cy, zone utile $ux,$uy ; $lb" | Out-Null
                $ex = [long]$C::GetWindowLongPtr($bandeau, $GWL_EXSTYLE)
                Verifier '(5) sans WS_EX_APPWINDOW' (($ex -band 0x40000) -eq 0) ('0x{0:X8}' -f $ex) | Out-Null
                Verifier '(5) premier plan inchangé' ($C::GetForegroundWindow() -eq $premier) "avant $premier, pendant $($C::GetForegroundWindow())" | Out-Null
                $vueDisque = Copie-Ecran $rb.X $rb.Y $rb.L $rb.H
                $od = Octets-Image $vueDisque
                $accent = 0
                # (PowerShell ignore la casse des variables : `$c` écraserait `$C`.)
                foreach ($teinte in $ACCENTS) { $accent += $C::Compter($od.Octets, $od.Stride, $vueDisque.Width, $vueDisque.Height, $teinte[0], $teinte[1], $teinte[2], 6) }
                if ($accent -lt 20) { Garder-Image '(5) disque' $vueDisque }
                $vueDisque.Dispose()
                Verifier '(5) le disque est peint (au moins 20 pixels à la couleur accent ± 6)' ($accent -ge 20) "$accent pixels" | Out-Null
                # (6) Second appui pendant le décompte.
                $C::Touches($CTRL_MAJ_2)
                $null = Attendre-Suivante 'capture deja en cours: appui ignore$' $db $ignores '(6) second appui'
                Verifier '(6) second appui : capture deja en cours: appui ignore' $true | Out-Null
                $lf = Attendre-Motif -Motif "decompte $num fini en (\d+) ms$" -Dossier $db -DelaiS 6 -Quoi '(5) fin du décompte'
                $null = $lf -match 'fini en (\d+) ms'
                $msFin = [int]$Matches[1]
                Verifier '(5) fini en 2950 à 3300 ms' (($msFin -ge 2950) -and ($msFin -le 3300)) $lf | Out-Null
                Noter-Mesure -Nom 'décompte de 3 s' -Valeur $msFin -Unite 'ms'
                $png = Attendre-Png $recents $n '(5) PNG'
                $iCache = Index-Motif 'bandeau decompte cache \d+$' $db $debut
                $iPhoto = Index-Motif 'chrono gel photo ' $db $debut
                Verifier '(5) bandeau decompte cache avant chrono gel photo' (($iCache -ge 0) -and ($iPhoto -gt $iCache)) "lignes $iCache et $iPhoto" | Out-Null
                $e = Comparer-Images $png $avantAppui ($rb.X - $ecranP.X) ($rb.Y - $ecranP.Y) $rb.L $rb.H 2
                if ($e[0] -ne 'total 0') { Garder-Image '(5) photo' $png; Garder-Image '(5) avant' $avantAppui }
                Verifier '(5) la photo, à la place du décompte, égale (± 2) à la copie d''avant l''appui' ($e[0] -eq 'total 0') ($e -join ' ; ') | Out-Null
                $png.Dispose(); $avantAppui.Dispose()
                Start-Sleep -Milliseconds 500
                Verifier '(6) une seule photo' (@(Fichiers-Png $recents).Count -eq $n + 1) "$(@(Fichiers-Png $recents).Count - $n) PNG" | Out-Null
                $null = Attendre-Motif -Motif 'etat \w+ -> Repos' -Dossier $db -DelaiS 5 -Quoi '(5) Repos'

                # (7) Échap pendant le décompte.
                $n = @(Fichiers-Png $recents).Count
                $photos = Compter-Motif -Motif 'chrono gel photo ' -Dossier $db
                $liberes = Compter-Motif -Motif 'echap global libere$' -Dossier $db
                $C::Touches($CTRL_MAJ_2)
                $ld = Attendre-Motif -Motif 'decompte (\d+) debut 3 s plein_ecran$' -Dossier $db -DelaiS 5 -Quoi '(7) decompte debut' -Fois 2
                $null = $ld -match 'decompte (\d+) debut'
                $num = $Matches[1]
                Start-Sleep -Milliseconds 500
                $C::Touches($ECHAP)
                $la = Attendre-Motif -Motif "decompte $num annule \(\w+\)$" -Dossier $db -DelaiS 3 -Quoi '(7) annulation'
                Verifier '(7) annule (echap)' ($la -match 'annule \(echap\)$') $la | Out-Null
                Verifier '(7) etat Decompte -> Repos (DecompteAnnule)' ((Compter-Motif -Motif 'etat Decompte -> Repos \(DecompteAnnule\)$' -Dossier $db) -eq 1) | Out-Null
                $null = Attendre-Suivante 'echap global libere$' $db $liberes '(7) echap global libere'
                Start-Sleep -Seconds 4
                Verifier '(7) aucune photo en 4 s, bandeau caché' (((Compter-Motif -Motif 'chrono gel photo ' -Dossier $db) -eq $photos) -and (@(Fichiers-Png $recents).Count -eq $n) -and -not ($C::IsWindowVisible($bandeau))) "photos $photos → $(Compter-Motif -Motif 'chrono gel photo ' -Dossier $db), bandeau $($C::IsWindowVisible($bandeau))" | Out-Null
                $C::Touches($CTRL_MAJ_2)
                $null = Attendre-Png $recents $n '(7) capture normale après Échap'
                Verifier '(7) puis une capture normale' $true | Out-Null
                $null = Attendre-Motif -Motif 'etat \w+ -> Repos' -Dossier $db -DelaiS 5 -Quoi '(7) Repos' -Fois 3

                # (8) La croix.
                $n = @(Fichiers-Png $recents).Count
                $premier = $C::GetForegroundWindow()
                $debuts = Compter-Motif -Motif 'decompte \d+ debut ' -Dossier $db
                $C::Touches($CTRL_MAJ_2)
                $null = Attendre-Suivante 'decompte \d+ debut ' $db $debuts '(8) decompte debut'
                $lb = Attendre-Motif -Motif 'bandeau decompte (\d+) ecran ' -Dossier $db -DelaiS 2 -Quoi '(8) bandeau decompte' -Fois ($debuts + 1)
                $null = $lb -match 'bandeau decompte (\d+) ecran '
                $id = $Matches[1]
                $lzb = Attendre-Motif -Motif "bandeau zone $id (-?\d+),(-?\d+) (\d+)x(\d+)$" -Dossier $db -DelaiS 2 -Quoi '(8) bandeau zone de la croix'
                $null = $lzb -match "bandeau zone $id (-?\d+),(-?\d+) (\d+)x(\d+)$"
                $px = [int]$Matches[1] + [int]([int]$Matches[3] / 2); $py = [int]$Matches[2] + [int]([int]$Matches[4] / 2)
                $C::Cliquer($px, $py)
                $null = Attendre-Motif -Motif "bandeau action $id annuler$" -Dossier $db -DelaiS 2 -Quoi "(8) bandeau action (clic en $px,$py)"
                $la = Attendre-Motif -Motif 'decompte \d+ annule \(\w+\)$' -Dossier $db -DelaiS 2 -Quoi '(8) annulation' -Fois 2
                Verifier '(8) bandeau action annuler, annule (croix)' ($la -match 'annule \(croix\)$') $la | Out-Null
                Verifier '(8) premier plan inchangé' ($C::GetForegroundWindow() -eq $premier) "avant $premier, après $($C::GetForegroundWindow())" | Out-Null
                Start-Sleep -Seconds 4
                Verifier '(8) aucune photo' (@(Fichiers-Png $recents).Count -eq $n) | Out-Null

                # (9) Zone avec retardateur.
                $montrees = Compter-Motif -Motif 'selection montree \d+ ' -Dossier $db
                $pretes = Compter-Motif -Motif 'chrono selection prete \d+' -Dossier $db
                $zones = Compter-Motif -Motif 'zone choisie ' -Dossier $db
                $C::Touches($CTRL_MAJ_1)
                $null = Attendre-Motif -Motif 'etat Repos -> Decompte \(DemandeDecompte zone\)$' -Dossier $db -DelaiS 5 -Quoi '(9) décompte de zone'
                $null = Attendre-Motif -Motif 'etat Decompte -> Gel \(DecompteFini zone\)$' -Dossier $db -DelaiS 5 -Quoi '(9) fin du décompte'
                $sm = Attendre-Suivante 'selection montree \d+ ' $db $montrees '(9) sélection'
                Verifier '(9) décompte puis sélection en mode zone' ($sm -match ' mode zone ') $sm | Out-Null
                $null = Attendre-Suivante 'chrono selection prete \d+' $db $pretes '(9) sélection prête'
                Start-Sleep -Milliseconds 150
                $C::Glisser($TX, $TY, $TX + $COTE - 1, $TY + $COTE - 1)
                $zc = Attendre-Suivante 'zone choisie ' $db $zones '(9) zone choisie'
                $png = Attendre-Png $recents $n '(9) PNG'
                Verifier '(9) zone choisie = le témoin' ($zc -match "zone choisie $TX,$TY ${COTE}x$COTE$") $zc | Out-Null
                Verifier-Temoin '(9) PNG' $png
                $png.Dispose()
                $null = Attendre-Motif -Motif 'etat \w+ -> Repos' -Dossier $db -DelaiS 5 -Quoi '(9) Repos' -Fois 5

                # (11) --quitter pendant un décompte.
                $n = @(Fichiers-Png $recents).Count
                $photos = Compter-Motif -Motif 'chrono gel photo ' -Dossier $db
                $debuts = Compter-Motif -Motif 'decompte \d+ debut ' -Dossier $db
                $C::Touches($CTRL_MAJ_2)
                $null = Attendre-Suivante 'decompte \d+ debut ' $db $debuts '(11) decompte debut'
                Start-Sleep -Milliseconds 500
                $q = Quitter-App -DelaiS 10 -Dossier $db
                if (-not $q.Fini) { throw "(11) --quitter n'a pas répondu en 10 s" }
                if (-not $p.WaitForExit(10000)) { throw "(11) l'app ne s'est pas fermée en 10 s" }
                $null = Attendre-Motif -Motif 'arret propre$' -Dossier $db -DelaiS 3 -Quoi '(11) arrêt'
                Verifier '(11) annule (arret), aucune photo, arret propre' (((Compter-Motif -Motif 'decompte \d+ annule \(arret\)$' -Dossier $db) -eq 1) -and ((Compter-Motif -Motif 'chrono gel photo ' -Dossier $db) -eq $photos) -and (@(Fichiers-Png $recents).Count -eq $n)) | Out-Null
            }
            catch { Verifier "(B) déroulement" $false "$_" | Out-Null }
            finally { Arreter-Tout }

            # ===== Cas C : retardateur 3 s, vignette allumée ========================================
            try {
                $dc = Dossier-Cas 'c'
                Ecrire-Reglages -Dossier $dc -Retardateur 3 -Vignette $true
                $p = Demarrer-Cas -Dossier $dc -Quoi '(C)'
                $C::Touches($CTRL_MAJ_2)
                $null = Attendre-Motif -Motif 'etat \w+ -> Vignette' -Dossier $dc -DelaiS 10 -Quoi '(10) vignette de la première capture'
                Start-Sleep -Milliseconds 500
                $fermees = Compter-Motif -Motif 'vignette fermee \d+ capture$' -Dossier $dc
                $C::Touches($CTRL_MAJ_2)
                $null = Attendre-Motif -Motif 'etat Vignette -> Decompte \(DemandeDecompte plein_ecran\)$' -Dossier $dc -DelaiS 5 -Quoi '(10) Vignette -> Decompte'
                $null = Attendre-Suivante 'vignette fermee \d+ capture$' $dc $fermees '(10) vignette fermée à l''entrée du décompte' 2
                Verifier '(10) vignette fermee <id> capture à l''entrée du décompte' $true | Out-Null
                $C::Touches($ECHAP)
                $null = Attendre-Motif -Motif 'decompte \d+ annule \(echap\)$' -Dossier $dc -DelaiS 3 -Quoi '(10) Échap'
                Fermer-Cas $p $dc '(C)'
            }
            catch { Verifier "(C) déroulement" $false "$_" | Out-Null }
            finally { Arreter-Tout }
        }
        finally {
            Fermer-Auxiliaires
            Arreter-Tout
        }
    }
}
catch {
    Verifier "déroulement de l'essai" $false "$_" | Out-Null
}
finally {
    # En cas d'échec, les 30 dernières lignes du journal de chaque cas.
    if (@($script:Verifications | Where-Object { -not $_.Reussi }).Count -gt 0) {
        foreach ($d in $script:Dossiers) {
            Write-Host "--- fin du journal du cas $(Split-Path -Leaf $d) ---"
            Lignes-Journal $d | Select-Object -Last 30 | ForEach-Object { Write-Host $_ }
        }
    }
    foreach ($a in $script:Auxiliaires) { try { if (-not $a.HasExited) { $a.Kill() } } catch { } }
    foreach ($d in $script:Dossiers) {
        $source = Join-Path $d 'local/Journal'
        if (Test-Path -LiteralPath $source) {
            $cible = Join-Path $script:Essai.Sortie ("journaux/10-confort-" + (Split-Path -Leaf $d))
            New-Item -ItemType Directory -Force -Path $cible | Out-Null
            Copy-Item -Path (Join-Path $source '*') -Destination $cible -Force
        }
    }
    Terminer-Essai
}
