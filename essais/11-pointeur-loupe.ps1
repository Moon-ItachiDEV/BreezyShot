# Version 0.4 : le pointeur dans l'image et la loupe de la sélection de zone (contrat
# 0.4 §6, §7, §11.3), sur le vrai .exe, avec de vraies entrées clavier et souris.
#
# (1) Windows montre-t-il le pointeur sur cette machine (GetCursorInfo) ? Sinon, tous les
# points du pointeur ÉCHOUENT en le disant : rien n'est réussi sans preuve.
# Cas « éteints » (pointeur et loupe éteints) :
# (2) pointe au milieu du carré haut gauche du témoin, Ctrl+Maj+2 : aucune ligne
# « pointeur », le carré de 32 × 32 pixels sous la pointe est à la couleur du témoin.
# (4) Ctrl+Maj+1 : aucune « loupe montree », la fenêtre BreezyShotLoupe jamais visible
# (guettée toutes les 2 ms pendant 1 s), Échap.
# Cas « allumés » :
# (3) même geste que (2) : « pointeur pris », « pointeur pose » ; au moins 20 pixels
# différents dans le carré, dont un presque noir et un presque blanc (flèche standard) ;
# hors d'un cadre autour du pointeur, le témoin est intact.
# (5) Ctrl+Maj+1, pointe loin du témoin : loupe visible moins de 200 ms après la
# sélection ; styles (outil, sans activation, traversée, calque, toujours au-dessus) ;
# au-dessus de la sélection ; WindowFromPoint en son centre rend la sélection ; à 24 px en
# bas à droite de la pointe ; pointe déplacée : la loupe suit en moins de 100 ms ; près du
# coin bas droit, elle passe de l'autre côté et reste sur l'écran ; pointe sur la
# frontière de deux carrés : la case centrale (copie d'écran) a la couleur du pixel visé
# (± 6), la case voisine celle du carré voisin ; glisser : « zone choisie » exacte,
# « pointeur hors zone » (la pointe était loin à la photo), pixels égaux au témoin ;
# « loupe cachee ». Puis Ctrl+Maj+1, Échap : « loupe cachee », fenêtre cachée.
# (6) Fenêtres stables d'une capture à l'autre, --quitter → arret propre.
#
# Tout tourne avec BREEZYSHOT_ESSAI_CAPTURABLE=1 : la loupe n'est pas exclue des captures,
# sinon la copie d'écran du point (5) ne verrait rien.
# Ce que la machine GitHub ne peut pas vérifier : la loupe et le pointeur à 125, 150 et
# 200 % (un seul écran à 100 %), plusieurs écrans, un pointeur agrandi ou animé.
# Hors Windows : seul le démarrage est vérifié.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '11-pointeur-loupe' -Exe $Exe -Sortie $Sortie | Out-Null

$script:Auxiliaires = [System.Collections.Generic.List[object]]::new()
$script:Pwsh = (Get-Process -Id $PID).Path

# --- Aides (même forme que les essais 04 et 05) -------------------------------------------

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
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $limite)
    $vu = Compter-Motif -Motif $Motif -Dossier $Dossier
    throw "$Quoi : rien dans le journal en $DelaiS s (motif « $Motif », attendu $Fois fois, vu $vu)"
}

# Réglages du cas : zone Ctrl+Maj+1, plein écran Ctrl+Maj+2, copie, pas d'enregistrement
# (l'image se lit dans « Récents »), vignette coupée, pointeur et loupe selon le cas.
function Ecrire-Reglages([string]$Dossier, [bool]$Allumes) {
    $r = [ordered]@{
        version                   = 3
        raccourcis                = [ordered]@{ zone = @('Ctrl+Maj+1'); plein_ecran = @('Ctrl+Maj+2'); fenetre = @(); derniere_zone = @() }
        copier_apres_capture      = $true
        enregistrer_apres_capture = $false
        dossier_enregistrement    = $null
        vignette                  = [ordered]@{ afficher = $false; duree_s = 6; coin = 'bas_gauche' }
        selection                 = [ordered]@{ loupe = $Allumes }
        capture                   = [ordered]@{ retardateur_s = 0; pointeur = $Allumes }
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
    Verifier "(6) $Quoi : --quitter → arret propre" $true | Out-Null
}

try {
    if (-not $IsWindows) {
        $commun = Dossier-Cas 'commun'
        Ecrire-Reglages -Dossier $commun -Allumes $true
        $p = Demarrer-Cas -Dossier $commun -Quoi 'commun'
        Verifier '(hors Windows) aucune fenêtre de loupe' ((Compter-Motif -Motif '^.*loupe fenetre ' -Dossier $commun) -eq 0) | Out-Null
        Fermer-Cas $p $commun 'commun'
        Write-Host '[saute] (1) à (5) : sautés hors Windows'
    }
    else {
        Add-Type -Namespace BreezyShotEssai -Name Loupe -MemberDefinition @'
[DllImport("user32.dll")]
static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
[DllImport("user32.dll")]
static extern uint MapVirtualKey(uint code, uint type);
[DllImport("user32.dll")]
static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
[DllImport("user32.dll")]
static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SetProcessDpiAwarenessContext(IntPtr contexte);
[DllImport("user32.dll")]
public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")]
public static extern int GetSystemMetrics(int index);
[DllImport("user32.dll")]
public static extern IntPtr GetWindowLongPtr(IntPtr h, int index);
[DllImport("user32.dll")]
static extern IntPtr GetWindow(IntPtr h, uint cmd);
[DllImport("user32.dll")]
static extern IntPtr GetAncestor(IntPtr h, uint flags);
[StructLayout(LayoutKind.Sequential)]
public struct POINT { public int X; public int Y; }
[DllImport("user32.dll")]
static extern IntPtr WindowFromPoint(POINT p);
[StructLayout(LayoutKind.Sequential)]
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
[DllImport("user32.dll")]
public static extern bool GetWindowRect(IntPtr h, out RECT r);
[StructLayout(LayoutKind.Sequential)]
public struct CURSORINFO { public int cbSize; public int flags; public IntPtr hCursor; public POINT pt; }
[DllImport("user32.dll", SetLastError = true)]
static extern bool GetCursorInfo(ref CURSORINFO info);
[DllImport("user32.dll")]
static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
delegate bool Rappel(IntPtr h, IntPtr p);
[DllImport("user32.dll")]
static extern bool EnumWindows(Rappel f, IntPtr p);

// Drapeaux de GetCursorInfo (-1 si l'appel échoue). 1 : montré ; 2 : supprimé.
public static int DrapeauxPointeur() {
    var i = new CURSORINFO();
    i.cbSize = Marshal.SizeOf(typeof(CURSORINFO));
    return GetCursorInfo(ref i) ? i.flags : -1;
}
const uint RELACHEE = 0x2;
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
// Glisser bouton gauche de (x1, y1) à (x2, y2), comme l'essai 04.
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
// Appuie les touches, puis guette $h toutes les 2 ms pendant $dureeMs : nombre de relevés
// où elle était visible.
public static int AppuyerEnGuettant(byte[] vks, IntPtr h, int dureeMs) {
    int vues = 0;
    var chrono = System.Diagnostics.Stopwatch.StartNew();
    Touches(vks);
    while (chrono.ElapsedMilliseconds < dureeMs) {
        if (IsWindowVisible(h)) vues++;
        System.Threading.Thread.Sleep(2);
    }
    return vues;
}
// Appuie les touches, puis guette les deux fenêtres toutes les 2 ms : rend { ms où la
// première est devenue visible, ms où la seconde l'est devenue }, -1 si jamais en $maxMs.
public static long[] AppuyerEtChronometrer(byte[] vks, IntPtr premiere, IntPtr seconde, int maxMs) {
    long a = -1, b = -1;
    var chrono = System.Diagnostics.Stopwatch.StartNew();
    Touches(vks);
    while (chrono.ElapsedMilliseconds < maxMs && (a < 0 || b < 0)) {
        if (a < 0 && IsWindowVisible(premiere)) a = chrono.ElapsedMilliseconds;
        if (b < 0 && IsWindowVisible(seconde)) b = chrono.ElapsedMilliseconds;
        System.Threading.Thread.Sleep(2);
    }
    return new long[] { a, b };
}
// Pose la pointe, puis attend que le coin haut gauche de $h soit ($x, $y) : ms, ou -1.
public static long PoserEtSuivre(IntPtr h, int px, int py, int x, int y, int maxMs) {
    var chrono = System.Diagnostics.Stopwatch.StartNew();
    Poser(px, py);
    while (chrono.ElapsedMilliseconds < maxMs) {
        RECT r;
        if (GetWindowRect(h, out r) && r.Left == x && r.Top == y) return chrono.ElapsedMilliseconds;
        System.Threading.Thread.Sleep(2);
    }
    return -1;
}
// Vrai si $haut est au-dessus de $bas dans l'ordre des fenêtres (descendre depuis $haut
// rencontre $bas).
public static bool AuDessus(IntPtr haut, IntPtr bas) {
    IntPtr h = haut;
    for (int i = 0; i < 5000 && h != IntPtr.Zero; i++) {
        h = GetWindow(h, 2);   // GW_HWNDNEXT
        if (h == bas) return true;
    }
    return false;
}
// La fenêtre de premier niveau sous le point (GA_ROOT).
public static IntPtr RacineSous(int x, int y) {
    var p = new POINT(); p.X = x; p.Y = y;
    IntPtr h = WindowFromPoint(p);
    return h == IntPtr.Zero ? h : GetAncestor(h, 2);
}
public static int FenetresDe(int pid) {
    int n = 0;
    EnumWindows((h, p) => { uint q; GetWindowThreadProcessId(h, out q); if ((int)q == pid) n++; return true; }, IntPtr.Zero);
    return n;
}
// Pixels du rectangle (x, y, l, h) d'une image BGRA (LockBits, Format32bppArgb), borné à
// l'image : { différents de rgb, presque noirs (< 60), presque blancs (> 200) }.
public static int[] Compter(byte[] o, int stride, int il, int ih, int x, int y, int l, int h, int r, int v, int b) {
    int diff = 0, noirs = 0, blancs = 0;
    for (int j = Math.Max(0, y); j < Math.Min(ih, y + h); j++) {
        for (int i = Math.Max(0, x); i < Math.Min(il, x + l); i++) {
            int k = j * stride + 4 * i;
            int pr = o[k + 2], pv = o[k + 1], pb = o[k];
            if (pr != r || pv != v || pb != b) diff++;
            if (pr < 60 && pv < 60 && pb < 60) noirs++;
            if (pr > 200 && pv > 200 && pb > 200) blancs++;
        }
    }
    return new int[] { diff, noirs, blancs };
}
// Pixels du témoin (tx, ty, cote ; c : r,v,b des quatre carrés), dans l'image, qui n'ont
// pas sa couleur, hors du rectangle exclu (ex, ey, el, eh). Rend { nombre, x, y du
// premier }.
public static int[] EcartsTemoin(byte[] o, int stride, int il, int ih, int tx, int ty, int cote, int[] c, int ex, int ey, int el, int eh) {
    int n = 0, px = -1, py = -1, m = cote / 2;
    for (int j = Math.Max(0, ty); j < Math.Min(ih, ty + cote); j++) {
        for (int i = Math.Max(0, tx); i < Math.Min(il, tx + cote); i++) {
            if (i >= ex && i < ex + el && j >= ey && j < ey + eh) continue;
            int q = ((j - ty) < m ? 0 : 2) + ((i - tx) < m ? 0 : 1);
            int k = j * stride + 4 * i;
            if (o[k + 2] != c[3 * q] || o[k + 1] != c[3 * q + 1] || o[k] != c[3 * q + 2]) {
                if (n == 0) { px = i; py = j; }
                n++;
            }
        }
    }
    return new int[] { n, px, py };
}
'@
        try { [void][BreezyShotEssai.Loupe]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms

        $CTRL_MAJ_1 = [byte[]]@(0x11, 0x10, 0x31)
        $CTRL_MAJ_2 = [byte[]]@(0x11, 0x10, 0x32)
        $ECHAP = [byte[]]@(0x1B)
        $COTE = 200
        $MOITIE = 100
        $COULEURS = @(@(220, 40, 40), @(40, 180, 60), @(40, 80, 220), @(240, 200, 30))
        $texteCouleurs = '220,40,40;40,180,60;40,80,220;240,200,30'
        $COULEURS_PLAT = [int[]]@(220, 40, 40, 40, 180, 60, 40, 80, 220, 240, 200, 30)
        # Mesures de la loupe à 100 % (constantes de systeme/gel/loupe.rs).
        $LOUPE_COTE = 120; $LOUPE_DECALAGE = 24; $LOUPE_CASE = 8

        $ecran = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        $utile = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        Write-Host "écran principal $ecran ; zone utile $utile"
        $TX = [int]($utile.X + 80)
        $TY = [int]($utile.Y + 80)
        # La pointe pour le pointeur : au milieu du carré haut gauche.
        $PX = $TX + 30; $PY = $TY + 30
        # Loin du témoin, pour la loupe et le pointeur hors zone.
        $LOIN = @(($utile.X + $utile.Width - 300), ($utile.Y + 300))

        function Lancer-Temoin {
            $f = Join-Path $script:Essai.Dossier ("temoin-" + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.txt')
            # Pas de -WindowStyle Hidden (il cacherait le témoin) : -CacherConsole.
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -ArgumentList @(
                '-NoProfile', '-File', (Join-Path $PSScriptRoot 'temoin.ps1'),
                '-X', $TX, '-Y', $TY, '-Cote', $COTE, '-Couleurs', $texteCouleurs, '-Fichier', $f, '-CacherConsole')
            $script:Auxiliaires.Add($aide)
            $limite = [DateTime]::UtcNow.AddSeconds(30)
            while (-not (Test-Path -LiteralPath $f)) {
                if ([DateTime]::UtcNow -ge $limite) { throw "fenêtre témoin : aucun HWND en 30 s" }
                Start-Sleep -Milliseconds 100
            }
            Start-Sleep -Milliseconds 500
            return [IntPtr][long](Get-Content -LiteralPath $f -Raw).Trim()
        }

        function Hwnd-Journal([string]$Motif, [string]$Dossier, [string]$Quoi) {
            $l = Attendre-Motif -Motif "$Motif (\d+)$" -Dossier $Dossier -DelaiS 5 -Quoi $Quoi
            if ($l -notmatch "$Motif (\d+)$") { throw "$Quoi : ligne illisible : $l" }
            return [IntPtr][long]$Matches[1]
        }

        function Visible([IntPtr]$h) { return [BreezyShotEssai.Loupe]::IsWindowVisible($h) }

        function Rect-De([IntPtr]$h) {
            $r = [BreezyShotEssai.Loupe+RECT]::new()
            [void][BreezyShotEssai.Loupe]::GetWindowRect($h, [ref]$r)
            return [pscustomobject]@{ X = $r.Left; Y = $r.Top; L = ($r.Right - $r.Left); H = ($r.Bottom - $r.Top) }
        }

        function Fichiers-Png([string]$Dossier) {
            if (-not (Test-Path -LiteralPath $Dossier)) { return @() }
            return @(Get-ChildItem -LiteralPath $Dossier -File -Filter '*.png' | Sort-Object LastWriteTimeUtc, Name)
        }

        # Octets BGRA d'un PNG : { Octets, Stride, L, H }.
        function Lire-Png([string]$Chemin) {
            $octets = [System.IO.File]::ReadAllBytes($Chemin)
            $flux = [System.IO.MemoryStream]::new($octets)
            try {
                $img = [System.Drawing.Bitmap]::new($flux)
                try { return (Octets-Image $img) }
                finally { $img.Dispose() }
            }
            finally { $flux.Dispose() }
        }

        function Octets-Image([System.Drawing.Bitmap]$Image) {
            $r = [System.Drawing.Rectangle]::new(0, 0, $Image.Width, $Image.Height)
            $d = $Image.LockBits($r, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
            try {
                $n = $d.Stride * $Image.Height
                $o = [byte[]]::new($n)
                [System.Runtime.InteropServices.Marshal]::Copy($d.Scan0, $o, 0, $n)
                return [pscustomobject]@{ Octets = $o; Stride = $d.Stride; L = $Image.Width; H = $Image.Height }
            }
            finally { $Image.UnlockBits($d) }
        }

        # Couleur d'un point de l'écran (copie d'écran de l'essai).
        function Couleur-Ecran([int]$X, [int]$Y) {
            $b = [System.Drawing.Bitmap]::new(1, 1)
            try {
                $g = [System.Drawing.Graphics]::FromImage($b)
                try { $g.CopyFromScreen($X, $Y, 0, 0, [System.Drawing.Size]::new(1, 1)) }
                finally { $g.Dispose() }
                $c = $b.GetPixel(0, 0)
                return @($c.R, $c.G, $c.B)
            }
            finally { $b.Dispose() }
        }

        function Proche([int[]]$A, [int[]]$B, [int]$Seuil) {
            for ($i = 0; $i -lt 3; $i++) { if ([math]::Abs($A[$i] - $B[$i]) -gt $Seuil) { return $false } }
            return $true
        }

        # Un plein écran par Ctrl+Maj+2, la pointe posée en ($X, $Y) : rend l'image de
        # « Récents » et la ligne « plein ecran » (origine de l'image).
        function Plein-Ecran([string]$Dossier, [string]$Quoi, [int]$X, [int]$Y) {
            $recents = Join-Path $Dossier 'local/Recents'
            $avant = @(Fichiers-Png $recents).Count
            $annonces = Compter-Motif -Motif 'sortie annonce ' -Dossier $Dossier
            [BreezyShotEssai.Loupe]::Poser($X, $Y)
            Start-Sleep -Milliseconds 150
            [BreezyShotEssai.Loupe]::Touches($CTRL_MAJ_2)
            $null = Attendre-Motif -Motif 'sortie annonce ' -Dossier $Dossier -DelaiS 10 -Quoi "$Quoi : annonce de la sortie" -Fois ($annonces + 1)
            $pngs = @(Fichiers-Png $recents)
            if ($pngs.Count -ne $avant + 1) { throw "$Quoi : $($pngs.Count - $avant) image(s) nouvelle(s) dans Récents au lieu d'une" }
            $ligne = Derniere-Ligne -Motif 'plein ecran -?\d+,-?\d+ ' -Dossier $Dossier
            if ($ligne -notmatch 'plein ecran (-?\d+),(-?\d+) ') { throw "$Quoi : ligne « plein ecran » illisible : $ligne" }
            return [pscustomobject]@{ Image = (Lire-Png $pngs[-1].FullName); OX = [int]$Matches[1]; OY = [int]$Matches[2] }
        }

        # (1) Le pointeur est-il montré par Windows ici ?
        $drapeaux = [BreezyShotEssai.Loupe]::DrapeauxPointeur()
        $pointeurMontre = ($drapeaux -ge 0) -and (($drapeaux -band 1) -ne 0) -and (($drapeaux -band 2) -eq 0)
        Verifier '(1) Windows montre le pointeur sur cette machine (sinon les points du pointeur ne prouvent rien)' $pointeurMontre "GetCursorInfo : drapeaux $drapeaux" | Out-Null

        $temoin = Lancer-Temoin

        # --- Cas « éteints » ----------------------------------------------------------------
        $de = Dossier-Cas 'eteints'
        Ecrire-Reglages -Dossier $de -Allumes $false
        $p = Demarrer-Cas -Dossier $de -Quoi 'éteints'
        $loupeE = Hwnd-Journal 'loupe fenetre' $de 'éteints : HWND de la loupe (créée cachée au démarrage)'
        Verifier '(éteints) loupe exclue des captures: non (essai)' ((Compter-Lignes -Texte 'loupe exclue des captures: non (essai)' -Dossier $de) -eq 1) | Out-Null
        $fenetresAvant = [BreezyShotEssai.Loupe]::FenetresDe($p.Id)

        # (2) Pointeur éteint : rien sous la pointe.
        $c = Plein-Ecran -Dossier $de -Quoi '(2)' -X $PX -Y $PY
        $i = $c.Image
        $n = [BreezyShotEssai.Loupe]::Compter($i.Octets, $i.Stride, $i.L, $i.H, ($PX - $c.OX), ($PY - $c.OY), 32, 32, 220, 40, 40)
        Verifier '(2) pointeur éteint : le carré de 32 × 32 sous la pointe est à la couleur du témoin' ($n[0] -eq 0) "$($n[0]) pixels différents" | Out-Null
        Verifier '(2) pointeur éteint : aucune ligne « pointeur »' ((Compter-Motif -Motif 'pointeur (pris|absent|pose|hors zone)' -Dossier $de) -eq 0) | Out-Null

        # (4) Loupe éteinte : jamais visible.
        $gels = Compter-Motif -Motif 'selection montree \d+ ' -Dossier $de
        [BreezyShotEssai.Loupe]::Poser($LOIN[0], $LOIN[1])
        $vues = [BreezyShotEssai.Loupe]::AppuyerEnGuettant($CTRL_MAJ_1, $loupeE, 1000)
        $null = Attendre-Motif -Motif 'selection montree \d+ ' -Dossier $de -DelaiS 5 -Quoi '(4) sélection montrée' -Fois ($gels + 1)
        Verifier '(4) loupe éteinte : fenêtre BreezyShotLoupe jamais visible (guettée 1 s)' ($vues -eq 0) "$vues relevés visibles" | Out-Null
        Verifier '(4) loupe éteinte : aucune « loupe montree »' ((Compter-Lignes -Texte 'loupe montree' -Dossier $de) -eq 0) | Out-Null
        $repos = Compter-Motif -Motif 'etat \w+ -> Repos' -Dossier $de
        [BreezyShotEssai.Loupe]::Touches($ECHAP)
        $null = Attendre-Motif -Motif 'etat \w+ -> Repos' -Dossier $de -DelaiS 5 -Quoi '(4) Échap' -Fois ($repos + 1)
        $fenetresApres = [BreezyShotEssai.Loupe]::FenetresDe($p.Id)
        Verifier '(6) éteints : fenêtres stables' ($fenetresApres -eq $fenetresAvant) "$fenetresAvant → $fenetresApres" | Out-Null
        Fermer-Cas $p $de 'éteints'

        # --- Cas « allumés » ----------------------------------------------------------------
        $da = Dossier-Cas 'allumes'
        Ecrire-Reglages -Dossier $da -Allumes $true
        $p = Demarrer-Cas -Dossier $da -Quoi 'allumés'
        $loupe = Hwnd-Journal 'loupe fenetre' $da 'allumés : HWND de la loupe'
        $sel = Hwnd-Journal 'selection fenetre' $da 'allumés : HWND de la sélection'
        $fenetresAvant = [BreezyShotEssai.Loupe]::FenetresDe($p.Id)

        # (3) Pointeur allumé : posé sous la pointe, rien ailleurs.
        $c = Plein-Ecran -Dossier $da -Quoi '(3)' -X $PX -Y $PY
        $i = $c.Image
        $pris = Derniere-Ligne -Motif 'pointeur (pris \d+x\d+ en \d+ ms|absent .*)$' -Dossier $da
        Verifier '(3) « pointeur pris <l>x<h> en <ms> ms »' (($null -ne $pris) -and ($pris -match 'pointeur pris \d+x\d+ en \d+ ms$')) "$pris" | Out-Null
        Verifier '(3) « pointeur pose »' ((Compter-Motif -Motif 'pointeur pose$' -Dossier $da) -eq 1) | Out-Null
        $n = [BreezyShotEssai.Loupe]::Compter($i.Octets, $i.Stride, $i.L, $i.H, ($PX - $c.OX), ($PY - $c.OY), 32, 32, 220, 40, 40)
        Verifier '(3) au moins 20 pixels changés sous la pointe, un presque noir et un presque blanc' (($n[0] -ge 20) -and ($n[1] -ge 1) -and ($n[2] -ge 1)) "différents $($n[0]), noirs $($n[1]), blancs $($n[2])" | Out-Null
        # Hors d'un cadre de 16 px avant la pointe à 64 px après (le pointeur standard fait
        # 32 × 32 à 100 %, point chaud en haut à gauche), le témoin est intact.
        $e = [BreezyShotEssai.Loupe]::EcartsTemoin($i.Octets, $i.Stride, $i.L, $i.H, ($TX - $c.OX), ($TY - $c.OY), $COTE, $COULEURS_PLAT, ($PX - 16 - $c.OX), ($PY - 16 - $c.OY), 80, 80)
        Verifier '(3) hors du pointeur, le témoin est intact' ($e[0] -eq 0) "$($e[0]) écarts, premier en $($e[1]),$($e[2]) de l'image" | Out-Null

        # (5) Loupe allumée.
        $gels = Compter-Motif -Motif 'selection montree \d+ ' -Dossier $da
        $pointeursAvant = Compter-Motif -Motif 'pointeur (pose|hors zone)$' -Dossier $da
        [BreezyShotEssai.Loupe]::Poser($LOIN[0], $LOIN[1])
        Start-Sleep -Milliseconds 150
        $t = [BreezyShotEssai.Loupe]::AppuyerEtChronometrer($CTRL_MAJ_1, $sel, $loupe, 3000)
        $null = Attendre-Motif -Motif 'selection montree \d+ ' -Dossier $da -DelaiS 5 -Quoi '(5) sélection montrée' -Fois ($gels + 1)
        $null = Attendre-Motif -Motif 'loupe montree$' -Dossier $da -DelaiS 2 -Quoi '(5) loupe montree'
        Verifier '(5) loupe visible moins de 200 ms après la sélection' (($t[0] -ge 0) -and ($t[1] -ge 0) -and (($t[1] - $t[0]) -lt 200)) "sélection à $($t[0]) ms, loupe à $($t[1]) ms" | Out-Null
        Noter-Mesure -Nom 'loupe visible après la sélection' -Valeur ($t[1] - $t[0]) -Unite 'ms'
        $prete = Derniere-Ligne -Motif 'chrono selection prete \d+' -Dossier $da
        if ($prete -match 'chrono selection prete (\d+)') { Noter-Mesure -Nom 'chrono selection prete (loupe allumée)' -Valeur ([int]$Matches[1]) -Unite 'ms' }
        $style = [long][BreezyShotEssai.Loupe]::GetWindowLongPtr($loupe, -20)
        $voulus = @(@(0x8, 'toujours au-dessus'), @(0x80, 'outil'), @(0x8000000, 'sans activation'), @(0x20, 'traversée'), @(0x80000, 'calque'))
        $manque = @($voulus | Where-Object { ($style -band $_[0]) -eq 0 } | ForEach-Object { $_[1] })
        Verifier '(5) styles de la loupe : outil, sans activation, traversée, calque, toujours au-dessus' ($manque.Count -eq 0) ("style 0x{0:x}, manque : {1}" -f $style, ($manque -join ', ')) | Out-Null
        Verifier '(5) loupe au-dessus de la sélection' ([BreezyShotEssai.Loupe]::AuDessus($loupe, $sel)) | Out-Null
        $r = Rect-De $loupe
        $racine = [BreezyShotEssai.Loupe]::RacineSous(($r.X + [int]($r.L / 2)), ($r.Y + [int]($r.H / 2)))
        Verifier '(5) WindowFromPoint au centre de la loupe rend la sélection (les clics traversent)' ($racine -eq $sel) "racine $racine, sélection $sel" | Out-Null
        $attendu = "$($LOIN[0] + $LOUPE_DECALAGE),$($LOIN[1] + $LOUPE_DECALAGE) ${LOUPE_COTE}x$LOUPE_COTE"
        $vu = "$($r.X),$($r.Y) $($r.L)x$($r.H)"
        Verifier '(5) loupe à 24 px en bas à droite de la pointe, 120 × 120' ($vu -eq $attendu) "attendu $attendu, vu $vu" | Out-Null

        $nx = $LOIN[0] - 100; $ny = $LOIN[1] + 50
        $ms = [BreezyShotEssai.Loupe]::PoserEtSuivre($loupe, $nx, $ny, ($nx + $LOUPE_DECALAGE), ($ny + $LOUPE_DECALAGE), 1000)
        Verifier '(5) pointe déplacée : la loupe suit en moins de 100 ms' (($ms -ge 0) -and ($ms -lt 100)) "$ms ms" | Out-Null
        Noter-Mesure -Nom 'loupe suit la pointe' -Valeur $ms -Unite 'ms'

        $cx = $ecran.X + $ecran.Width - 5; $cy = $ecran.Y + $ecran.Height - 5
        $ms = [BreezyShotEssai.Loupe]::PoserEtSuivre($loupe, $cx, $cy, ($cx - $LOUPE_DECALAGE - $LOUPE_COTE), ($cy - $LOUPE_DECALAGE - $LOUPE_COTE), 1000)
        $r = Rect-De $loupe
        $dedans = ($r.X -ge $ecran.X) -and ($r.Y -ge $ecran.Y) -and (($r.X + $r.L) -le ($ecran.X + $ecran.Width)) -and (($r.Y + $r.H) -le ($ecran.Y + $ecran.Height))
        Verifier '(5) près du coin bas droit : de l''autre côté de la pointe, sur l''écran' (($ms -ge 0) -and $dedans) "$ms ms, loupe $($r.X),$($r.Y) $($r.L)x$($r.H)" | Out-Null

        # Pointe sur le dernier pixel du carré haut gauche, le carré haut droit juste à droite.
        $fx = $TX + $MOITIE - 1; $fy = $TY + 50
        $ms = [BreezyShotEssai.Loupe]::PoserEtSuivre($loupe, $fx, $fy, ($fx + $LOUPE_DECALAGE), ($fy + $LOUPE_DECALAGE), 1000)
        Start-Sleep -Milliseconds 100
        $r = Rect-De $loupe
        $centre = 7 * $LOUPE_CASE + [int]($LOUPE_CASE / 2)
        $vise = Couleur-Ecran ($r.X + $centre) ($r.Y + $centre)
        $voisin = Couleur-Ecran ($r.X + $centre + $LOUPE_CASE) ($r.Y + $centre)
        Verifier '(5) case centrale de la loupe à la couleur du pixel visé (± 6)' (($ms -ge 0) -and (Proche $vise $COULEURS[0] 6)) "loupe en $($r.X),$($r.Y) ($ms ms), vu $($vise -join ','), attendu $($COULEURS[0] -join ',')" | Out-Null
        Verifier '(5) case voisine à la couleur du carré voisin (± 6)' (Proche $voisin $COULEURS[1] 6) "vu $($voisin -join ','), attendu $($COULEURS[1] -join ',')" | Out-Null

        # Glisser dans le carré bas droit : zone exacte, pointeur hors zone, pixels du témoin.
        $recents = Join-Path $da 'local/Recents'
        $avant = @(Fichiers-Png $recents).Count
        $annonces = Compter-Motif -Motif 'sortie annonce ' -Dossier $da
        $cachees = Compter-Motif -Motif 'loupe cachee$' -Dossier $da
        $zx = $TX + 110; $zy = $TY + 110
        [BreezyShotEssai.Loupe]::Glisser($zx, $zy, ($zx + 79), ($zy + 79))
        $null = Attendre-Motif -Motif 'sortie annonce ' -Dossier $da -DelaiS 10 -Quoi '(5) annonce de la sortie' -Fois ($annonces + 1)
        $zone = Derniere-Ligne -Motif 'zone choisie ' -Dossier $da
        Verifier '(5) glisser : zone choisie exacte' ($zone -match "zone choisie $zx,$zy 80x80$") "$zone" | Out-Null
        $hors = Lignes-Journal $da | Where-Object { $_ -match 'pointeur (pose|hors zone)$' } | Select-Object -Last 1
        Verifier '(5) « pointeur hors zone » (la pointe était loin à la photo)' ((Compter-Motif -Motif 'pointeur (pose|hors zone)$' -Dossier $da) -eq ($pointeursAvant + 1) -and ("$hors" -match 'pointeur hors zone$')) "$hors" | Out-Null
        $pngs = @(Fichiers-Png $recents)
        if ($pngs.Count -eq $avant + 1) {
            $i = Lire-Png $pngs[-1].FullName
            $n = [BreezyShotEssai.Loupe]::Compter($i.Octets, $i.Stride, $i.L, $i.H, 0, 0, $i.L, $i.H, 240, 200, 30)
            Verifier '(5) image de la zone : 80 × 80, toute à la couleur du carré bas droit (ni pointeur ni loupe)' (($i.L -eq 80) -and ($i.H -eq 80) -and ($n[0] -eq 0)) "$($i.L)x$($i.H), $($n[0]) pixels différents" | Out-Null
        }
        else { Verifier '(5) une image de la zone dans Récents' $false "$($pngs.Count - $avant) image(s) nouvelle(s)" | Out-Null }
        $null = Attendre-Motif -Motif 'loupe cachee$' -Dossier $da -DelaiS 3 -Quoi '(5) loupe cachee après le glisser' -Fois ($cachees + 1)
        Verifier '(5) après le glisser : « loupe cachee », fenêtre cachée' (-not (Visible $loupe)) | Out-Null

        # Échap : la loupe se cache.
        $gels = Compter-Motif -Motif 'selection montree \d+ ' -Dossier $da
        $montrees = Compter-Motif -Motif 'loupe montree$' -Dossier $da
        [BreezyShotEssai.Loupe]::Poser($LOIN[0], $LOIN[1])
        [BreezyShotEssai.Loupe]::Touches($CTRL_MAJ_1)
        $null = Attendre-Motif -Motif 'selection montree \d+ ' -Dossier $da -DelaiS 5 -Quoi '(5) seconde sélection' -Fois ($gels + 1)
        $null = Attendre-Motif -Motif 'loupe montree$' -Dossier $da -DelaiS 2 -Quoi '(5) seconde loupe' -Fois ($montrees + 1)
        $cachees = Compter-Motif -Motif 'loupe cachee$' -Dossier $da
        [BreezyShotEssai.Loupe]::Touches($ECHAP)
        $null = Attendre-Motif -Motif 'loupe cachee$' -Dossier $da -DelaiS 3 -Quoi '(5) Échap : loupe cachee' -Fois ($cachees + 1)
        Start-Sleep -Milliseconds 100
        Verifier '(5) Échap : « loupe cachee », fenêtre cachée' (-not (Visible $loupe)) | Out-Null
        Verifier '(5) aucune « loupe impossible »' ((Compter-Lignes -Texte 'loupe impossible' -Dossier $da) -eq 0) | Out-Null

        $fenetresApres = [BreezyShotEssai.Loupe]::FenetresDe($p.Id)
        Verifier '(6) allumés : fenêtres stables' ($fenetresApres -eq $fenetresAvant) "$fenetresAvant → $fenetresApres" | Out-Null
        Fermer-Cas $p $da 'allumés'
        $null = $temoin
    }
}
catch {
    Verifier "déroulement de l'essai" $false "$_" | Out-Null
}
finally {
    foreach ($a in $script:Auxiliaires) { try { if (-not $a.HasExited) { $a.Kill() } } catch { } }
    Terminer-Essai
}
