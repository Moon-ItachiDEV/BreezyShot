# Étape 8 : la capture d'une fenêtre, de bout en bout (ARCHITECTURE.md §3.6), sur le vrai
# .exe, avec de vraies entrées clavier et souris.
#
# Témoin : une vraie fenêtre à cadre (temoin.ps1 -Cadre, toujours au-dessus) au milieu de
# la zone utile de l'écran principal. Référence : son vrai bord,
# DWMWA_EXTENDED_FRAME_BOUNDS, lu par l'essai une fois la fenêtre montrée et stable.
# (1) Démarrage : « raccourci fenetre: Ctrl+Maj+3 ». (2) Souris au milieu du vrai bord,
# Ctrl+Maj+3 : Repos -> Gel, chrono fenetres, Gel -> Selection (PhotoPrise fenetre),
# « selection montree … mode fenetre fenetres <n ≥ 1> ». (3) Clic gauche : « selection fin
# <id> fenetre », Selection -> Decoupe (ZoneChoisie), « fenetre choisie » exactement sur le
# vrai bord (coins 1111), Decoupe -> Repos, « Copié et enregistré ». (4) Le PNG a la taille
# du vrai bord ; le rayon est lu dans « fenetre choisie », jamais supposé : rayon ≥ 4,
# coins d'alpha 0 ; rayon de 1 à 3, coins d'alpha < 255 (le pixel du coin n'est à 0 qu'à
# partir de 4, capture/decoupe.rs) ; dans les deux cas centre et milieux des bords d'alpha
# 255 ; rayon 0 : tout opaque. Mesure « rayon ». (5) Presse-papiers : GetImage (CF_BITMAP
# que Windows tire de CF_DIB) de la bonne taille, coin blanc si rayon ≥ 4 ; le format
# « PNG » lu en octets (rayon > 0 : présent, coin d'alpha 0 si rayon ≥ 4) ; CF_DIB lu en
# octets et décodé à la main (en-tête de 40 octets, 32 bits, BI_RGB, de bas en haut, BGR) :
# taille, coin blanc si rayon ≥ 4, centre égal au centre du PNG. (6) Le contenu de la
# témoin : des pixels de ses carrés, à leur place. (7) Souris sur le bureau (aucune
# fenêtre dessous, vérifié avant par l'essai avec les règles de la liste de l'app ; sinon
# point sauté et dit), clic : la sélection reste ; Échap : Selection -> Repos
# (SelectionAnnulee). (8) Glisser : aucune fin, la sélection reste ; clic droit : annulée.
# (9) Après chaque capture : écran figé et sélection cachés, premier plan rendu.
# (10) --quitter → arret propre, aucune instance restée en vie.
#
# Ce que la machine GitHub ne peut pas vérifier : une fenêtre à cheval sur deux écrans
# (coupée, coins partiels : tests purs de `capture::zone_fenetre` et `coins_a_arrondir`),
# des écrans d'échelles différentes, le menu de l'icône (ATTENTE_MENU non essayé). Sur une
# machine sans coins arrondis (avant Windows 11), seul le cas « rayon 0 » est joué.
#
# BREEZYSHOT_ESSAI_CAPTURABLE=1 comme pour 04 et 05. Hors Windows : seul le démarrage.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '06-fenetre' -Exe $Exe -Sortie $Sortie | Out-Null

$script:Auxiliaires = [System.Collections.Generic.List[object]]::new()
$script:Dossiers = [System.Collections.Generic.List[string]]::new()
$script:Pwsh = (Get-Process -Id $PID).Path

# --- Aides (même forme que les essais 04 et 05) -------------------------------------------

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

function Attendre-Motif([string]$Motif, [string]$Dossier, [double]$DelaiS, [string]$Quoi, [int]$Fois = 1) {
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        if ((Compter-Motif -Motif $Motif -Dossier $Dossier) -ge $Fois) {
            return (Derniere-Ligne -Motif $Motif -Dossier $Dossier)
        }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $limite)
    $vu = Compter-Motif -Motif $Motif -Dossier $Dossier
    throw "$Quoi : rien dans le journal en $DelaiS s (motif « $Motif », attendu $Fois fois, vu $vu)"
}

function Lignes-Depuis([string]$Dossier, [int]$Debut) {
    $toutes = @(Lignes-Journal $Dossier)
    if ($Debut -ge $toutes.Count) { return @() }
    return @($toutes[$Debut..($toutes.Count - 1)])
}

# Réglages : Ctrl+Maj+1 pour la zone, Ctrl+Maj+3 pour la fenêtre, rien pour le plein
# écran (liste vide : aucune annonce), copie, enregistrement vrai, vignette coupée (elle a
# son essai 07).
function Ecrire-Reglages([string]$Dossier) {
    $r = [ordered]@{
        version                   = 3
        raccourcis                = [ordered]@{ zone = @('Ctrl+Maj+1'); plein_ecran = @(); fenetre = @('Ctrl+Maj+3'); derniere_zone = @() }
        copier_apres_capture      = $true
        enregistrer_apres_capture = $true
        dossier_enregistrement    = $null
        vignette                  = [ordered]@{ afficher = $false; duree_s = 6; coin = 'bas_gauche' }
        selection                 = [ordered]@{ loupe = $false }
        capture                   = [ordered]@{ retardateur_s = 0; pointeur = $false }
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

# (10) --quitter → arret propre, processus sorti, aucune fenêtre BreezyShot restante.
function Fermer-Cas($Processus, [string]$Dossier, [string]$Quoi) {
    $arrets = Compter-Motif -Motif 'arret propre$' -Dossier $Dossier
    $q = Quitter-App -DelaiS 10 -Dossier $Dossier
    if (-not $q.Fini) { throw "$Quoi : --quitter n'a pas répondu en 10 s" }
    if (-not $Processus.WaitForExit(10000)) { throw "$Quoi : l'app ne s'est pas fermée en 10 s" }
    $null = Attendre-Motif -Motif 'arret propre$' -Dossier $Dossier -DelaiS 3 -Quoi "$Quoi : arrêt" -Fois ($arrets + 1)
    Verifier "(10) $Quoi : --quitter → arret propre" $true | Out-Null
    if ($IsWindows) {
        $figees = [BreezyShotEssai.Fenetre]::FenetresDeClasse('BreezyShotEcranFige')
        $restes = [BreezyShotEssai.Fenetre]::FenetresDe([int[]]@($Processus.Id))
        Verifier "(10) $Quoi : aucune instance BreezyShot restée en vie (toutes les fenêtres énumérées)" (($figees.Length -eq 0) -and ($restes.Length -eq 0)) "écrans figés $($figees.Length), fenêtres du PID $($Processus.Id) : $($restes.Length)" | Out-Null
    }
}

try {
    if (-not $IsWindows) {
        # --- Hors Windows : démarrage seulement --------------------------------------------
        $commun = Dossier-Cas 'commun'
        Ecrire-Reglages -Dossier $commun
        $p = Demarrer-Cas -Dossier $commun -Quoi 'commun'
        Verifier '(1) ligne « raccourci fenetre: … »' ((Compter-Lignes -Texte 'raccourci fenetre: ' -Dossier $commun) -eq 1) | Out-Null
        Fermer-Cas $p $commun 'commun'
        Write-Host '[saute] (2) à (9) : sautés hors Windows'
    }
    else {
        # --- Windows ---------------------------------------------------------------------
        Add-Type -Namespace BreezyShotEssai -Name Fenetre -MemberDefinition @'
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
public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")]
public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")]
public static extern uint GetClipboardSequenceNumber();
[DllImport("user32.dll")]
static extern bool IsIconic(IntPtr h);
[DllImport("user32.dll", EntryPoint = "GetWindowLongW")]
static extern int GetWindowLong(IntPtr h, int index);
[DllImport("user32.dll")]
static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
delegate bool Rappel(IntPtr h, IntPtr p);
[DllImport("user32.dll")]
static extern bool EnumWindows(Rappel f, IntPtr p);
[DllImport("user32.dll", CharSet = CharSet.Unicode)]
static extern int GetClassName(IntPtr h, System.Text.StringBuilder nom, int taille);
[StructLayout(LayoutKind.Sequential)]
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
[StructLayout(LayoutKind.Sequential)]
public struct POINT { public int X; public int Y; }
[DllImport("user32.dll")]
static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll")]
static extern bool ClientToScreen(IntPtr h, ref POINT p);
[DllImport("dwmapi.dll")]
static extern int DwmGetWindowAttribute(IntPtr h, int attribut, out RECT r, int taille);
[DllImport("dwmapi.dll", EntryPoint = "DwmGetWindowAttribute")]
static extern int DwmGetWindowAttributeEntier(IntPtr h, int attribut, out int valeur, int taille);
[DllImport("user32.dll", SetLastError = true)]
static extern bool OpenClipboard(IntPtr h);
[DllImport("user32.dll")]
static extern bool CloseClipboard();
[DllImport("user32.dll")]
static extern bool IsClipboardFormatAvailable(uint format);
[DllImport("user32.dll")]
static extern IntPtr GetClipboardData(uint format);
[DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
public static extern uint RegisterClipboardFormatW(string nom);
[DllImport("kernel32.dll")]
static extern IntPtr GlobalLock(IntPtr h);
[DllImport("kernel32.dll")]
static extern bool GlobalUnlock(IntPtr h);
[DllImport("kernel32.dll")]
static extern UIntPtr GlobalSize(IntPtr h);
[DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
static extern IntPtr CreateWindowExW(uint exStyle, string classe, string titre, uint style, int x, int y, int l, int h, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr param);
[DllImport("user32.dll", SetLastError = true)]
static extern bool DestroyWindow(IntPtr h);

const uint RELACHEE = 0x2;
// Appuie les touches dans l'ordre, puis les relâche dans l'ordre inverse.
public static void Touches(byte[] vks) {
    foreach (var vk in vks) keybd_event(vk, (byte)MapVirtualKey(vk, 0), 0u, UIntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    for (int i = vks.Length - 1; i >= 0; i--) keybd_event(vks[i], (byte)MapVirtualKey(vks[i], 0), RELACHEE, UIntPtr.Zero);
}
// Pose le curseur et envoie un vrai mouvement d'entrée (relatif nul) à cet endroit.
public static void Poser(int x, int y) {
    SetCursorPos(x, y);
    mouse_event(0x1, 0, 0, 0, UIntPtr.Zero);
}
// Glisser bouton gauche de (x1, y1) à (x2, y2) par 16 positions intermédiaires (comme 04).
public static void Glisser(int x1, int y1, int x2, int y2) {
    Poser(x1, y1);
    System.Threading.Thread.Sleep(120);
    mouse_event(0x2, 0, 0, 0, UIntPtr.Zero);
    System.Threading.Thread.Sleep(80);
    for (int i = 1; i <= 16; i++) {
        Poser(x1 + (x2 - x1) * i / 16, y1 + (y2 - y1) * i / 16);
        System.Threading.Thread.Sleep(30);
    }
    Poser(x2, y2);
    System.Threading.Thread.Sleep(150);
    mouse_event(0x4, 0, 0, 0, UIntPtr.Zero);
}
// Clic gauche (ou droit) sans déplacer la souris entre l'appui et le relâché (comme 04).
public static void Cliquer(int x, int y, bool droit) {
    Poser(x, y);
    System.Threading.Thread.Sleep(120);
    mouse_event(droit ? 0x8u : 0x2u, 0, 0, 0, UIntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    mouse_event(droit ? 0x10u : 0x4u, 0, 0, 0, UIntPtr.Zero);
}
// Le vrai bord (DWMWA_EXTENDED_FRAME_BOUNDS = 9) : { x, y, l, h } ; null si illisible.
public static int[] BordDwm(IntPtr h) {
    RECT r;
    if (DwmGetWindowAttribute(h, 9, out r, Marshal.SizeOf(typeof(RECT))) != 0) return null;
    return new int[] { r.Left, r.Top, r.Right - r.Left, r.Bottom - r.Top };
}
// Coin haut gauche de la zone cliente, en pixels du bureau.
public static int[] OrigineCliente(IntPtr h) {
    var p = new POINT();
    if (!ClientToScreen(h, ref p)) return null;
    return new int[] { p.X, p.Y };
}
static string Classe(IntPtr h) {
    var nom = new System.Text.StringBuilder(256);
    return GetClassName(h, nom, nom.Capacity) > 0 ? nom.ToString() : "";
}
// Fenêtres de premier niveau des processus donnés.
public static IntPtr[] FenetresDe(int[] pids) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    var ensemble = new System.Collections.Generic.HashSet<int>(pids);
    EnumWindows((h, p) => {
        uint pid; GetWindowThreadProcessId(h, out pid);
        if (ensemble.Contains((int)pid)) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
// Toutes les fenêtres de haut niveau (de tous les processus) de cette classe.
public static IntPtr[] FenetresDeClasse(string classe) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, p) => {
        if (Classe(h) == classe) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
// Les fenêtres que la liste de l'app garderait (systeme/gel : visible, pas réduite, pas
// masquée par DWM, pas calque et transparente à la fois, pas du bureau, pas de
// BreezyShot) et dont le vrai bord contient (x, y) : leurs classes. Vide : rien sous le
// point, un clic n'y choisit aucune fenêtre.
public static string[] FenetresSous(int x, int y, int pidExclu) {
    var r = new System.Collections.Generic.List<string>();
    EnumWindows((h, p) => {
        uint pid; GetWindowThreadProcessId(h, out pid);
        if ((int)pid == pidExclu || !IsWindowVisible(h) || IsIconic(h)) return true;
        int masque;
        if (DwmGetWindowAttributeEntier(h, 14, out masque, 4) == 0 && masque != 0) return true;
        int ex = GetWindowLong(h, -20);
        if ((ex & 0x80000) != 0 && (ex & 0x20) != 0) return true;
        string c = Classe(h);
        if (c == "Progman" || c == "WorkerW") return true;
        RECT b;
        if (DwmGetWindowAttribute(h, 9, out b, Marshal.SizeOf(typeof(RECT))) != 0 && !GetWindowRect(h, out b)) return true;
        if (b.Right <= b.Left || b.Bottom <= b.Top) return true;
        if (x >= b.Left && x < b.Right && y >= b.Top && y < b.Bottom) r.Add(c);
        return true;
    }, IntPtr.Zero);
    return r.ToArray();
}
// Les octets d'un format du presse-papiers, lus directement (OpenClipboard au nom d'une
// fenêtre de messages de l'essai, 10 essais à 20 ms, GetClipboardData, GlobalLock,
// GlobalSize). null si le format est absent ou illisible ; « ouvert » dit si le
// presse-papiers a pu être ouvert.
public static byte[] LireFormat(uint format, out bool ouvert) {
    ouvert = false;
    IntPtr fenetre = CreateWindowExW(0, "STATIC", "", 0, 0, 0, 0, 0, new IntPtr(-3), IntPtr.Zero, IntPtr.Zero, IntPtr.Zero);
    if (fenetre == IntPtr.Zero) return null;
    try {
        for (int i = 0; i < 10 && !ouvert; i++) {
            if (i > 0) System.Threading.Thread.Sleep(20);
            ouvert = OpenClipboard(fenetre);
        }
        if (!ouvert) return null;
        try {
            if (!IsClipboardFormatAvailable(format)) return null;
            IntPtr h = GetClipboardData(format);
            if (h == IntPtr.Zero) return null;
            IntPtr p = GlobalLock(h);
            if (p == IntPtr.Zero) return null;
            try {
                long n = (long)GlobalSize(h).ToUInt64();
                if (n <= 0 || n > int.MaxValue) return null;
                var o = new byte[n];
                Marshal.Copy(p, o, 0, (int)n);
                return o;
            }
            finally { GlobalUnlock(h); }
        }
        finally { CloseClipboard(); }
    }
    finally { DestroyWindow(fenetre); }
}
// En-tête d'un CF_DIB : { biSize, largeur, hauteur, plans, bits, compression,
// couleurs utilisées } ; null si trop court.
public static int[] EnteteDib(byte[] d) {
    if (d == null || d.Length < 40) return null;
    return new int[] { BitConverter.ToInt32(d, 0), BitConverter.ToInt32(d, 4), BitConverter.ToInt32(d, 8),
        BitConverter.ToInt16(d, 12), BitConverter.ToInt16(d, 14), BitConverter.ToInt32(d, 16), BitConverter.ToInt32(d, 32) };
}
// Pixel (x, y), compté depuis le haut, d'un CF_DIB 32 bits BI_RGB de bas en haut à
// en-tête de 40 octets sans table : { r, v, b } ; null hors de l'image ou des octets.
public static int[] PixelDib(byte[] d, int x, int y) {
    int[] e = EnteteDib(d);
    if (e == null || e[0] != 40 || e[4] != 32 || e[5] != 0 || e[6] != 0 || e[2] <= 0) return null;
    int l = e[1], h = e[2];
    if (x < 0 || y < 0 || x >= l || y >= h) return null;
    long k = 40 + ((long)(h - 1 - y) * l + x) * 4;
    if (k + 3 >= d.Length) return null;
    return new int[] { d[k + 2], d[k + 1], d[k] };
}
// Nombre de pixels d'alpha différent de 255 (octets BGRA de LockBits, Format32bppArgb).
public static int NonOpaques(byte[] o, int stride, int l, int h) {
    int n = 0;
    for (int y = 0; y < h; y++) for (int x = 0; x < l; x++) if (o[y * stride + 4 * x + 3] != 255) n++;
    return n;
}
'@
        try {
            $dpiOk = [BreezyShotEssai.Fenetre]::SetProcessDpiAwarenessContext([IntPtr]::new(-4))
            Write-Host "SetProcessDpiAwarenessContext(-4) : $(if ($dpiOk) { 'ok' } else { "refusé ($([System.Runtime.InteropServices.Marshal]::GetLastWin32Error()))" })"
        } catch { Write-Host "SetProcessDpiAwarenessContext(-4) indisponible : $_" }
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms

        $CTRL_MAJ_3 = [byte[]]@(0x11, 0x10, 0x33)
        $ECHAP = [byte[]]@(0x1B)
        $COTE = 240
        # Haut gauche, haut droite, bas gauche, bas droite (r, v, b).
        $COULEURS = @(@(220, 40, 40), @(40, 180, 60), @(40, 80, 220), @(240, 200, 30))
        $texteCouleurs = '220,40,40;40,180,60;40,80,220;240,200,30'
        $FORMAT_PNG = [BreezyShotEssai.Fenetre]::RegisterClipboardFormatW('PNG')
        $CF_DIB = [uint32]8

        # La témoin au milieu de la zone utile de l'écran principal, loin de ses bords.
        $utile = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        $PX = [int]($utile.X + ($utile.Width - $COTE) / 2)
        $PY = [int]($utile.Y + ($utile.Height - $COTE) / 2 - 20)
        Write-Host "zone utile $utile ; témoin posée en $PX,$PY, zone cliente ${COTE}x$COTE"

        function Lancer-Temoin {
            $f = Join-Path $script:Essai.Dossier ("temoin-" + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.txt')
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -ArgumentList @(
                '-NoProfile', '-File', (Join-Path $PSScriptRoot 'temoin.ps1'),
                '-X', $PX, '-Y', $PY, '-Cote', $COTE, '-Couleurs', $texteCouleurs, '-Fichier', $f, '-Cadre')
            $script:Auxiliaires.Add($aide)
            $limite = [DateTime]::UtcNow.AddSeconds(30)
            while (-not (Test-Path -LiteralPath $f)) {
                if ([DateTime]::UtcNow -ge $limite) { throw "fenêtre témoin : aucun HWND en 30 s" }
                Start-Sleep -Milliseconds 100
            }
            return [IntPtr][long](Get-Content -LiteralPath $f -Raw).Trim()
        }

        # Le vrai bord de la témoin, une fois stable (deux lectures égales à 300 ms).
        function Bord-Stable([IntPtr]$h) {
            $avant = $null
            for ($i = 0; $i -lt 20; $i++) {
                Start-Sleep -Milliseconds 300
                $b = [BreezyShotEssai.Fenetre]::BordDwm($h)
                if ($null -ne $b) {
                    $texte = $b -join ','
                    if ($texte -eq $avant) { return [pscustomobject]@{ X = $b[0]; Y = $b[1]; L = $b[2]; H = $b[3] } }
                    $avant = $texte
                }
            }
            throw "témoin : vrai bord (DWMWA_EXTENDED_FRAME_BOUNDS) jamais stable ou illisible"
        }

        function Hwnd-Journal([string]$Motif, [string]$Dossier, [string]$Quoi) {
            $l = Attendre-Motif -Motif "$Motif (\d+)$" -Dossier $Dossier -DelaiS 5 -Quoi $Quoi
            if ($l -notmatch "$Motif (\d+)$") { throw "$Quoi : ligne illisible : $l" }
            return [IntPtr][long]$Matches[1]
        }

        function Visible([IntPtr]$h) { return [BreezyShotEssai.Fenetre]::IsWindowVisible($h) }

        function Octets-Image([System.Drawing.Bitmap]$Image) {
            $r = [System.Drawing.Rectangle]::new(0, 0, $Image.Width, $Image.Height)
            $d = $Image.LockBits($r, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
            try {
                $n = $d.Stride * $Image.Height
                $o = [byte[]]::new($n)
                [System.Runtime.InteropServices.Marshal]::Copy($d.Scan0, $o, 0, $n)
                return [pscustomobject]@{ Octets = $o; Stride = $d.Stride }
            }
            finally { $Image.UnlockBits($d) }
        }

        # GetImage, lu dans un fil STA (seul permis par Windows). $null si vide.
        function Lire-PressePapiers {
            $rs = [runspacefactory]::CreateRunspace()
            $rs.ApartmentState = [System.Threading.ApartmentState]::STA
            $rs.ThreadOptions = [System.Management.Automation.Runspaces.PSThreadOptions]::ReuseThread
            $rs.Open()
            try {
                $ps = [powershell]::Create()
                $ps.Runspace = $rs
                [void]$ps.AddScript({
                        Add-Type -AssemblyName System.Windows.Forms
                        Add-Type -AssemblyName System.Drawing
                        for ($i = 0; $i -lt 10; $i++) {
                            try {
                                $img = [System.Windows.Forms.Clipboard]::GetImage()
                                if ($null -ne $img) {
                                    $copie = [System.Drawing.Bitmap]::new($img)
                                    $img.Dispose()
                                    return , $copie
                                }
                            } catch { }
                            Start-Sleep -Milliseconds 100
                        }
                        return $null
                    })
                try { $r = $ps.Invoke() }
                finally { $ps.Dispose() }
                foreach ($x in $r) {
                    if ($null -ne $x) {
                        $b = $x.psobject.BaseObject
                        if ($b -is [System.Drawing.Bitmap]) { return $b }
                    }
                }
                return $null
            }
            finally { $rs.Dispose() }
        }

        # Les octets d'un format du presse-papiers : { Ouvert, Octets } (Octets $null si absent).
        function Lire-Format([uint32]$Format) {
            $ouvert = $false
            $o = [BreezyShotEssai.Fenetre]::LireFormat($Format, [ref]$ouvert)
            return [pscustomobject]@{ Ouvert = $ouvert; Octets = $o }
        }

        function Fichiers-Png([string]$Dossier) {
            if (-not (Test-Path -LiteralPath $Dossier)) { return @() }
            return @(Get-ChildItem -LiteralPath $Dossier -File -Filter '*.png' | Sort-Object LastWriteTimeUtc, Name)
        }

        function Png-Octets([byte[]]$Octets) {
            $flux = [System.IO.MemoryStream]::new($Octets)
            try {
                $img = [System.Drawing.Bitmap]::new($flux)
                try { return [System.Drawing.Bitmap]::new($img) }
                finally { $img.Dispose() }
            }
            finally { $flux.Dispose() }
        }

        # Les quatre coins, le centre et le milieu de chaque bord d'une image de l x h.
        function Points-Forme([int]$L, [int]$H) {
            return [pscustomobject]@{
                Coins   = @(@(0, 0), @(($L - 1), 0), @(0, ($H - 1)), @(($L - 1), ($H - 1)))
                Pleins  = @(@([int]($L / 2), [int]($H / 2)), @([int]($L / 2), 0), @([int]($L / 2), ($H - 1)), @(0, [int]($H / 2)), @(($L - 1), [int]($H / 2)))
            }
        }

        # (4) Alpha d'une image d'après le rayon lu : rend vrai si tout va.
        function Verifier-Alpha([string]$Nom, [System.Drawing.Bitmap]$Image, [int]$Rayon) {
            $pts = Points-Forme $Image.Width $Image.Height
            $coins = @($pts.Coins | ForEach-Object { $Image.GetPixel($_[0], $_[1]).A })
            $pleins = @($pts.Pleins | ForEach-Object { $Image.GetPixel($_[0], $_[1]).A })
            $detail = "rayon $Rayon, coins $($coins -join ','), centre et milieux des bords $($pleins -join ',')"
            $pleinsOk = @($pleins | Where-Object { $_ -ne 255 }).Count -eq 0
            if ($Rayon -ge 4) {
                return (Verifier "$Nom : coins d'alpha 0, centre et milieux des bords d'alpha 255" ((@($coins | Where-Object { $_ -ne 0 }).Count -eq 0) -and $pleinsOk) $detail)
            }
            if ($Rayon -gt 0) {
                return (Verifier "$Nom : coins d'alpha < 255 (rayon < 4), centre et milieux des bords d'alpha 255" ((@($coins | Where-Object { $_ -eq 255 }).Count -eq 0) -and $pleinsOk) $detail)
            }
            $o = Octets-Image $Image
            $n = [BreezyShotEssai.Fenetre]::NonOpaques($o.Octets, $o.Stride, $Image.Width, $Image.Height)
            return (Verifier "$Nom : rayon 0, tout est opaque" ($n -eq 0) "$n pixel(s) non opaques")
        }

        # (6) Le contenu de la témoin : dans chaque carré, cinq points loin de ses bords (et
        # des coins arrondis de la fenêtre), aux bonnes couleurs. (OX, OY) : coin haut
        # gauche de la zone cliente dans l'image.
        function Verifier-Contenu([string]$Nom, [System.Drawing.Bitmap]$Image, [int]$OX, [int]$OY) {
            $moitie = [int]($COTE / 2)
            $places = @(@(0, 0), @($moitie, 0), @(0, $moitie), @($moitie, $moitie))
            $q4 = [int]($moitie / 4); $q34 = [int](3 * $moitie / 4); $q2 = [int]($moitie / 2)
            $points = @(@($q4, $q4), @($q34, $q4), @($q4, $q34), @($q34, $q34), @($q2, $q2))
            $ecarts = [System.Collections.Generic.List[string]]::new()
            for ($q = 0; $q -lt 4; $q++) {
                foreach ($pt in $points) {
                    $x = $OX + $places[$q][0] + $pt[0]; $y = $OY + $places[$q][1] + $pt[1]
                    if (($x -lt 0) -or ($y -lt 0) -or ($x -ge $Image.Width) -or ($y -ge $Image.Height)) { $ecarts.Add("$x,$y hors de l'image"); continue }
                    $c = $Image.GetPixel($x, $y)
                    $a = $COULEURS[$q]
                    if (($c.R -ne $a[0]) -or ($c.G -ne $a[1]) -or ($c.B -ne $a[2]) -or ($c.A -ne 255)) {
                        $ecarts.Add("$x,$y attendu $($a -join ',') obtenu $($c.R),$($c.G),$($c.B) alpha $($c.A)")
                    }
                }
            }
            $ok = $ecarts.Count -eq 0
            if (-not $ok) { $Image.Save((Join-Path $script:Essai.Sortie ("06-" + ($Nom -replace '[^\w]+', '-') + '.png')), [System.Drawing.Imaging.ImageFormat]::Png) }
            Verifier "$Nom : le contenu de la témoin est à sa place, aux bonnes couleurs" $ok (@($ecarts | Select-Object -First 8) -join ' ; ') | Out-Null
        }

        # Ctrl+Maj+3 et la sélection prête. Rend { Id, Debut, Avant (premier plan avant) }.
        function Ouvrir-Selection([string]$Dossier, [string]$Quoi) {
            $debut = @(Lignes-Journal $Dossier).Count
            $avant = [BreezyShotEssai.Fenetre]::GetForegroundWindow()
            $gels = Compter-Motif -Motif 'etat Repos -> Gel' -Dossier $Dossier
            $pretes = Compter-Motif -Motif 'chrono selection prete \d+' -Dossier $Dossier
            [BreezyShotEssai.Fenetre]::Touches($CTRL_MAJ_3)
            $null = Attendre-Motif -Motif 'etat Repos -> Gel' -Dossier $Dossier -DelaiS 5 -Quoi "$Quoi : raccourci" -Fois ($gels + 1)
            $m = Attendre-Motif -Motif 'selection montree (\d+) ' -Dossier $Dossier -DelaiS 5 -Quoi "$Quoi : sélection montrée" -Fois ($gels + 1)
            $null = Attendre-Motif -Motif 'chrono selection prete \d+' -Dossier $Dossier -DelaiS 5 -Quoi "$Quoi : page de sélection prête" -Fois ($pretes + 1)
            $null = $m -match 'selection montree (\d+) '
            $id = [int]$Matches[1]
            Start-Sleep -Milliseconds 300
            return [pscustomobject]@{ Id = $id; Debut = $debut; Avant = $avant }
        }

        # (9) Retour au Repos : écran figé et sélection cachés, premier plan rendu.
        function Verifier-Fin([string]$Dossier, [string]$Quoi, [int]$Fois, $S, [IntPtr]$Sel, [IntPtr]$Fige) {
            $null = Attendre-Motif -Motif 'etat \w+ -> Repos' -Dossier $Dossier -DelaiS 10 -Quoi "$Quoi : retour au Repos" -Fois $Fois
            Start-Sleep -Milliseconds 300
            Verifier "(9) $Quoi : écran figé et sélection cachés" ((-not (Visible $Sel)) -and (-not (Visible $Fige))) "sélection $(Visible $Sel), figé $(Visible $Fige)" | Out-Null
            $apres = [BreezyShotEssai.Fenetre]::GetForegroundWindow()
            Verifier "(9) $Quoi : la fenêtre qui avait le premier plan l'a retrouvé" ($apres -eq $S.Avant) "avant $($S.Avant), après $apres" | Out-Null
        }

        function Repos-Vus([string]$Dossier) { return (Compter-Motif -Motif 'etat \w+ -> Repos' -Dossier $Dossier) }

        # La sélection reste ouverte après un geste sans choix : visible 800 ms après, aucune
        # fin pour cet id, aucun Repos de plus.
        function Verifier-Reste-Ouverte([string]$Dossier, [string]$Quoi, $S, [IntPtr]$Sel, [int]$Repos) {
            Start-Sleep -Milliseconds 800
            $fins = Compter-Motif -Motif "selection fin $($S.Id) " -Dossier $Dossier
            $finsSeules = Compter-Motif -Motif "selection fin $($S.Id)$" -Dossier $Dossier
            $r = Repos-Vus $Dossier
            Verifier "$Quoi : la sélection reste ouverte (visible, aucune fin, aucun Repos)" ((Visible $Sel) -and ($fins -eq 0) -and ($finsSeules -eq 0) -and ($r -eq $Repos)) "visible $(Visible $Sel), fins $fins, Repos $Repos → $r" | Out-Null
        }

        try {
            $temoin = Lancer-Temoin
            $bord = Bord-Stable $temoin
            $origine = [BreezyShotEssai.Fenetre]::OrigineCliente($temoin)
            if ($null -eq $origine) { throw 'témoin : ClientToScreen impossible' }
            $MX = [int]($bord.X + [math]::Floor($bord.L / 2)); $MY = [int]($bord.Y + [math]::Floor($bord.H / 2))
            $TEXTE_BORD = "$($bord.X),$($bord.Y) $($bord.L)x$($bord.H)"
            Write-Host "témoin $temoin : vrai bord $TEXTE_BORD, zone cliente en $($origine[0]),$($origine[1]), milieu $MX,$MY"
            Verifier 'témoin : le vrai bord est sur la zone utile de l''écran principal' (($bord.X -ge $utile.X) -and ($bord.Y -ge $utile.Y) -and ($bord.X + $bord.L -le $utile.X + $utile.Width) -and ($bord.Y + $bord.H -le $utile.Y + $utile.Height)) "bord $TEXTE_BORD, zone utile $utile" | Out-Null

            $da = Dossier-Cas 'a'
            Ecrire-Reglages -Dossier $da
            $p = Demarrer-Cas -Dossier $da -Quoi '(A)'
            try {
                $images = Join-Path $da 'images'

                # (1) Démarrage.
                Verifier '(1) raccourci fenetre: Ctrl+Maj+3' ((Compter-Lignes -Texte 'raccourci fenetre: Ctrl+Maj+3' -Dossier $da) -eq 1) | Out-Null
                Verifier '(1) aucune annonce au démarrage' ((Compter-Lignes -Texte 'annonce raccourci' -Dossier $da) -eq 0) | Out-Null
                $sel = Hwnd-Journal 'selection fenetre' $da 'HWND de la sélection'
                $fige = Hwnd-Journal 'ecran fige fenetre' $da 'HWND de l''écran figé'

                # (2) Souris au milieu du vrai bord, Ctrl+Maj+3.
                [BreezyShotEssai.Fenetre]::Poser($MX, $MY)
                Start-Sleep -Milliseconds 200
                $reposAvant = Repos-Vus $da
                $annonces = Compter-Motif -Motif 'sortie annonce ' -Dossier $da
                $seqAvant = [BreezyShotEssai.Fenetre]::GetClipboardSequenceNumber()
                $s = Ouvrir-Selection -Dossier $da -Quoi '(2)'
                $l2 = @(Lignes-Depuis $da $s.Debut)
                $t2 = $l2 -join "`n"
                Verifier '(2) etat Repos -> Gel (DemandeCapture fenetre)' ($t2.Contains('etat Repos -> Gel (DemandeCapture fenetre)')) | Out-Null
                Verifier '(2) chrono gel photo … total … visible …' (@($l2 | Where-Object { $_ -match 'chrono gel photo \d+ total \d+ visible \d+$' }).Count -eq 1) | Out-Null
                $chronoF = @($l2 | Where-Object { $_ -match 'chrono fenetres (\d+) nombre (\d+)$' })
                Verifier '(2) chrono fenetres <ms> nombre <n>' ($chronoF.Count -eq 1) ($chronoF -join ' ; ') | Out-Null
                if (($chronoF.Count -ge 1) -and ($chronoF[0] -match 'chrono fenetres (\d+) nombre (\d+)$')) {
                    Noter-Mesure -Nom 'chrono fenetres' -Valeur ([int]$Matches[1]) -Unite 'ms'
                    Noter-Mesure -Nom 'fenêtres listées' -Valeur ([int]$Matches[2])
                }
                Verifier '(2) etat Gel -> Selection (PhotoPrise fenetre)' ($t2.Contains('etat Gel -> Selection (PhotoPrise fenetre)')) | Out-Null
                $montree = @($l2 | Where-Object { $_ -match "selection montree $($s.Id) voulu \S+ reel \S+ echelle \S+ placements \d+ mode fenetre fenetres (\d+)$" })
                $nf = 0
                if (($montree.Count -ge 1) -and ($montree[0] -match 'fenetres (\d+)$')) { $nf = [int]$Matches[1] }
                Verifier '(2) selection montree … mode fenetre fenetres <n ≥ 1>' (($montree.Count -eq 1) -and ($nf -ge 1)) ($montree -join ' ; ') | Out-Null
                Verifier '(2) echap global reserve' ($t2.Contains('echap global reserve')) | Out-Null
                Verifier '(2) écran figé et sélection visibles' ((Visible $sel) -and (Visible $fige)) | Out-Null

                # (3) Clic gauche au milieu du vrai bord.
                [BreezyShotEssai.Fenetre]::Cliquer($MX, $MY, $false)
                $null = Attendre-Motif -Motif "selection fin $($s.Id) fenetre$" -Dossier $da -DelaiS 5 -Quoi '(3) fin de la sélection'
                $null = Attendre-Motif -Motif 'sortie annonce ' -Dossier $da -DelaiS 10 -Quoi '(3) annonce de la sortie' -Fois ($annonces + 1)
                Verifier-Fin -Dossier $da -Quoi '(3)' -Fois ($reposAvant + 1) -S $s -Sel $sel -Fige $fige
                $l3 = @(Lignes-Depuis $da $s.Debut)
                $t3 = $l3 -join "`n"
                Verifier "(3) selection fin $($s.Id) fenetre" $true | Out-Null
                Verifier '(3) etat Selection -> Decoupe (ZoneChoisie)' ($t3.Contains('etat Selection -> Decoupe (ZoneChoisie)')) | Out-Null
                $choisie = @($l3 | Where-Object { $_ -match 'fenetre choisie ' })
                $rayon = -1; $coinsLus = ''
                $motifChoisie = 'fenetre choisie (-?\d+),(-?\d+) (\d+)x(\d+) rayon (\d+) coins ([01]{4})$'
                if (($choisie.Count -eq 1) -and ($choisie[0] -match $motifChoisie)) {
                    $rayon = [int]$Matches[5]; $coinsLus = $Matches[6]
                    $lu = "$($Matches[1]),$($Matches[2]) $($Matches[3])x$($Matches[4])"
                    Verifier '(3) fenetre choisie = le vrai bord de la témoin (DWMWA_EXTENDED_FRAME_BOUNDS)' ($lu -eq $TEXTE_BORD) "journal $lu, vrai bord $TEXTE_BORD" | Out-Null
                    Verifier '(3) fenetre choisie : coins 1111 (fenêtre entière sur un écran)' ($coinsLus -eq '1111') $choisie[0] | Out-Null
                }
                else {
                    Verifier '(3) fenetre choisie <x>,<y> <l>x<h> rayon <r> coins <hg><hd><bg><bd>' $false ($choisie -join ' ; ') | Out-Null
                }
                Verifier '(3) aucune ligne « zone choisie »' (-not $t3.Contains('zone choisie ')) | Out-Null
                Verifier '(3) sortie copie faite recent ok fichier fait' ($t3.Contains('sortie copie faite recent ok fichier fait')) | Out-Null
                Verifier '(3) sortie annonce reussite: Copié et enregistré' ($t3.Contains('sortie annonce reussite: Copié et enregistré')) | Out-Null
                Verifier '(3) etat Decoupe -> Repos (ImagePreteSansVignette)' ($t3.Contains('etat Decoupe -> Repos (ImagePreteSansVignette)')) | Out-Null
                Verifier '(3) echap global libere' ($t3.Contains('echap global libere')) | Out-Null
                Noter-Mesure -Nom 'rayon' -Valeur $rayon -Unite 'px'
                # Windows 11 et suivants (build ≥ 22000) arrondissent une fenêtre à cadre : un
                # rayon de 0 y cacherait une règle des coins cassée (tout opaque passerait).
                $build = [Environment]::OSVersion.Version.Build
                if ($build -ge 22000) {
                    Verifier '(3) Windows arrondit les fenêtres à cadre : rayon > 0' ($rayon -gt 0) "build $build, rayon $rayon" | Out-Null
                }
                if ($rayon -ge 4) { Write-Host "cas joué : coins arrondis (rayon $rayon)" }
                elseif ($rayon -gt 0) { Write-Host "cas joué : petits arrondis (rayon $rayon < 4)" }
                elseif ($rayon -eq 0) { Write-Host 'cas joué : coins droits (rayon 0)' }

                # (4) Le PNG enregistré.
                $L = $bord.L; $H = $bord.H
                $OX = $origine[0] - $bord.X; $OY = $origine[1] - $bord.Y
                $pngs = @(Fichiers-Png $images)
                Verifier '(4) un PNG dans le dossier Images › BreezyShot de l''essai' ($pngs.Count -eq 1) "$($pngs.Count) fichier(s)" | Out-Null
                $centrePng = $null
                if (($pngs.Count -ge 1) -and ($rayon -ge 0)) {
                    $image = Png-Octets ([System.IO.File]::ReadAllBytes($pngs[-1].FullName))
                    try {
                        $taille = "$($image.Width)x$($image.Height)"
                        if (Verifier "(4) PNG : taille du vrai bord (${L}x$H)" ($taille -eq "${L}x$H") "obtenu $taille") {
                            Verifier-Alpha '(4) PNG' $image $rayon | Out-Null
                            # (6) Le contenu, sur le fichier.
                            Verifier-Contenu '(6) PNG' $image $OX $OY
                            $c = $image.GetPixel([int]($L / 2), [int]($H / 2))
                            $centrePng = @([int]$c.R, [int]$c.G, [int]$c.B)
                        }
                        else { $image.Save((Join-Path $script:Essai.Sortie '06-png.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
                    }
                    finally { $image.Dispose() }
                }

                # (5) Le presse-papiers : changé, puis chaque format lu explicitement.
                Verifier '(5) le presse-papiers a changé' ([BreezyShotEssai.Fenetre]::GetClipboardSequenceNumber() -ne $seqAvant) | Out-Null
                $copie = Lire-PressePapiers
                if ($null -eq $copie) { Verifier '(5) GetImage : une image' $false 'aucune image' | Out-Null }
                else {
                    try {
                        $taille = "$($copie.Width)x$($copie.Height)"
                        if (Verifier "(5) GetImage : taille du vrai bord (${L}x$H)" ($taille -eq "${L}x$H") "obtenu $taille") {
                            if ($rayon -ge 4) {
                                $c = $copie.GetPixel(0, 0)
                                Verifier '(5) GetImage : pixel du coin blanc' (($c.R -eq 255) -and ($c.G -eq 255) -and ($c.B -eq 255)) "obtenu $($c.R),$($c.G),$($c.B)" | Out-Null
                            }
                            Verifier-Contenu '(6) GetImage' $copie $OX $OY
                        }
                    }
                    finally { $copie.Dispose() }
                }
                $fp = Lire-Format $FORMAT_PNG
                if (-not $fp.Ouvert) { Verifier '(5) presse-papiers ouvert par l''essai' $false 'OpenClipboard refusé 10 fois' | Out-Null }
                elseif ($rayon -gt 0) {
                    if (Verifier '(5) le format « PNG » existe' ($null -ne $fp.Octets) "format $FORMAT_PNG") {
                        $imgPng = Png-Octets $fp.Octets
                        try {
                            $taille = "$($imgPng.Width)x$($imgPng.Height)"
                            if (Verifier "(5) format « PNG » : taille du vrai bord (${L}x$H)" ($taille -eq "${L}x$H") "obtenu $taille, $($fp.Octets.Length) octets") {
                                Verifier-Alpha '(5) format « PNG »' $imgPng $rayon | Out-Null
                            }
                        }
                        finally { $imgPng.Dispose() }
                    }
                }
                else { Write-Host "(5) rayon 0 : format « PNG » $(if ($null -ne $fp.Octets) { 'présent' } else { 'absent' }) (non exigé)" }
                $fd = Lire-Format $CF_DIB
                if (Verifier '(5) CF_DIB lisible' ($fd.Ouvert -and ($null -ne $fd.Octets)) "ouvert $($fd.Ouvert)") {
                    $e = [BreezyShotEssai.Fenetre]::EnteteDib($fd.Octets)
                    $enteteOk = ($null -ne $e) -and ($e[0] -eq 40) -and ($e[1] -eq $L) -and ($e[2] -eq $H) -and ($e[3] -eq 1) -and ($e[4] -eq 32) -and ($e[5] -eq 0) -and ($e[6] -eq 0) -and ($fd.Octets.Length -ge 40 + 4 * $L * $H)
                    $texteEntete = if ($null -eq $e) { 'trop court' } else { "biSize $($e[0]), $($e[1])x$($e[2]), plans $($e[3]), bits $($e[4]), compression $($e[5]), couleurs $($e[6]), $($fd.Octets.Length) octets" }
                    if (Verifier "(5) CF_DIB : en-tête de 40 octets, ${L}x$H positif (de bas en haut), 32 bits, BI_RGB" $enteteOk $texteEntete) {
                        if ($rayon -ge 4) {
                            $c = [BreezyShotEssai.Fenetre]::PixelDib($fd.Octets, 0, 0)
                            Verifier '(5) CF_DIB : pixel du coin blanc (composé sur blanc)' (($null -ne $c) -and (($c -join ',') -eq '255,255,255')) "obtenu $(if ($null -ne $c) { $c -join ',' } else { 'rien' })" | Out-Null
                        }
                        $c = [BreezyShotEssai.Fenetre]::PixelDib($fd.Octets, [int]($L / 2), [int]($H / 2))
                        Verifier '(5) CF_DIB : centre égal au centre du PNG (BGR, de bas en haut)' (($null -ne $c) -and ($null -ne $centrePng) -and (($c -join ',') -eq ($centrePng -join ','))) "DIB $(if ($null -ne $c) { $c -join ',' } else { 'rien' }), PNG $(if ($null -ne $centrePng) { $centrePng -join ',' } else { 'rien' })" | Out-Null
                        # Le pixel du milieu du carré haut gauche de la témoin, à sa place.
                        $c = [BreezyShotEssai.Fenetre]::PixelDib($fd.Octets, $OX + [int]($COTE / 4), $OY + [int]($COTE / 4))
                        Verifier '(6) CF_DIB : carré haut gauche de la témoin à sa place' (($null -ne $c) -and (($c -join ',') -eq ($COULEURS[0] -join ','))) "obtenu $(if ($null -ne $c) { $c -join ',' } else { 'rien' })" | Out-Null
                    }
                }

                # (7) Clic sur le bureau : un point sans aucune fenêtre dessous, cherché avant
                # la capture avec les règles de la liste de l'app ; sinon point sauté.
                $libre = $null
                foreach ($fy in @(0.85, 0.15, 0.5, 0.7, 0.3)) {
                    foreach ($fx in @(0.08, 0.92, 0.25, 0.75, 0.5)) {
                        $x = [int]($utile.X + $utile.Width * $fx); $y = [int]($utile.Y + $utile.Height * $fy)
                        if (($x -ge $bord.X - 20) -and ($x -lt $bord.X + $bord.L + 20) -and ($y -ge $bord.Y - 20) -and ($y -lt $bord.Y + $bord.H + 20)) { continue }
                        $sous = @([BreezyShotEssai.Fenetre]::FenetresSous($x, $y, $p.Id))
                        if ($sous.Count -eq 0) { $libre = @($x, $y); break }
                    }
                    if ($null -ne $libre) { break }
                }
                if ($null -eq $libre) {
                    Write-Host '[saute] (7) clic sur le bureau : aucun point de la zone utile sans fenêtre dessous, point sauté (non compté réussi)'
                    Noter-Mesure -Nom '(7) clic sur le bureau' -Valeur 'sauté'
                }
                else {
                    Write-Host "(7) point du bureau sans fenêtre : $($libre[0]),$($libre[1])"
                    $reposAvant = Repos-Vus $da
                    $s7 = Ouvrir-Selection -Dossier $da -Quoi '(7)'
                    # Encore libre, vérifié juste avant le clic (rien n'est apparu entre-temps).
                    $sous = @([BreezyShotEssai.Fenetre]::FenetresSous($libre[0], $libre[1], $p.Id))
                    Verifier '(7) toujours aucune fenêtre sous le point au moment du clic' ($sous.Count -eq 0) ($sous -join ', ') | Out-Null
                    [BreezyShotEssai.Fenetre]::Cliquer($libre[0], $libre[1], $false)
                    Verifier-Reste-Ouverte -Dossier $da -Quoi '(7) clic sur le bureau' -S $s7 -Sel $sel -Repos $reposAvant
                    [BreezyShotEssai.Fenetre]::Touches($ECHAP)
                    Verifier-Fin -Dossier $da -Quoi '(7) Échap' -Fois ($reposAvant + 1) -S $s7 -Sel $sel -Fige $fige
                    Verifier '(7) Échap : etat Selection -> Repos (SelectionAnnulee)' ((Derniere-Ligne -Motif 'etat \w+ -> Repos' -Dossier $da) -match 'etat Selection -> Repos \(SelectionAnnulee\)$') | Out-Null
                }

                # (8) Glisser sur la témoin : aucune fin ; clic droit : annulée.
                $reposAvant = Repos-Vus $da
                $s8 = Ouvrir-Selection -Dossier $da -Quoi '(8)'
                [BreezyShotEssai.Fenetre]::Glisser(($MX - 60), ($MY - 40), ($MX + 60), ($MY + 40))
                Verifier-Reste-Ouverte -Dossier $da -Quoi '(8) glisser' -S $s8 -Sel $sel -Repos $reposAvant
                [BreezyShotEssai.Fenetre]::Cliquer($MX, $MY, $true)
                $null = Attendre-Motif -Motif "selection fin $($s8.Id) annulee clic_droit$" -Dossier $da -DelaiS 5 -Quoi '(8) clic droit'
                Verifier-Fin -Dossier $da -Quoi '(8) clic droit' -Fois ($reposAvant + 1) -S $s8 -Sel $sel -Fige $fige
                Verifier '(8) clic droit : etat Selection -> Repos (SelectionAnnulee)' ((Derniere-Ligne -Motif 'etat \w+ -> Repos' -Dossier $da) -match 'etat Selection -> Repos \(SelectionAnnulee\)$') | Out-Null
                Verifier '(8) un seul PNG en tout (les gestes sans choix n''ont rien produit)' (@(Fichiers-Png $images).Count -eq 1) "$(@(Fichiers-Png $images).Count) fichier(s)" | Out-Null

                Verifier '(A) aucune garde déclenchée, aucune erreur de capture' (((Compter-Lignes -Texte 'arret force' -Dossier $da) -eq 0) -and ((Compter-Lignes -Texte 'capture impossible' -Dossier $da) -eq 0)) | Out-Null
            }
            finally {
                Fermer-Cas $p $da '(A)'
            }
        }
        finally {
            foreach ($a in $script:Auxiliaires) {
                try { if (-not $a.HasExited) { $a.Kill(); $null = $a.WaitForExit(5000) } } catch { }
            }
            Arreter-Tout
        }
    }
}
catch {
    Verifier "déroulement de l'essai" $false "$_" | Out-Null
}
finally {
    foreach ($d in $script:Dossiers) {
        $chronos = @(Lignes-Journal $d | Where-Object { $_ -match ' chrono ' })
        if ($chronos.Count -gt 0) {
            Write-Host "--- chronos du cas $(Split-Path -Leaf $d) ---"
            $chronos | ForEach-Object { Write-Host $_ }
        }
    }
    if (@($script:Verifications | Where-Object { -not $_.Reussi }).Count -gt 0) {
        foreach ($d in $script:Dossiers) {
            Write-Host "--- fin du journal du cas $(Split-Path -Leaf $d) ---"
            Lignes-Journal $d | Select-Object -Last 40 | ForEach-Object { Write-Host $_ }
        }
    }
    foreach ($d in $script:Dossiers) {
        $source = Join-Path $d 'local/Journal'
        if (Test-Path -LiteralPath $source) {
            $cible = Join-Path $script:Essai.Sortie ("journaux/06-fenetre-" + (Split-Path -Leaf $d))
            New-Item -ItemType Directory -Force -Path $cible | Out-Null
            Copy-Item -Path (Join-Path $source '*') -Destination $cible -Force
        }
    }
    Terminer-Essai
}
