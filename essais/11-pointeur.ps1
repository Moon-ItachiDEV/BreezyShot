# Version 0.4 : le pointeur dans l'image (contrat 0.4 §6, §11.3), sur le vrai .exe, avec de
# vraies entrées clavier et souris.
#
# (1) Windows montre-t-il le pointeur sur cette machine (GetCursorInfo) ? D'abord un vrai
# mouvement (SendInput) ; s'il reste caché, MouseKeys est allumé (remis à la fin). Sinon,
# tous les points du pointeur ÉCHOUENT en le disant : rien n'est réussi sans preuve.
# Cas « éteint » (réglage Inclure le pointeur éteint) :
# (2) pointe au milieu du carré haut gauche du témoin, Ctrl+Maj+2 : aucune ligne
# « pointeur », le carré de 32 × 32 pixels sous la pointe est à la couleur du témoin.
# Cas « allumé » :
# (3) même geste : « pointeur pris », « pointeur pose » ; au moins 20 pixels différents
# dans le carré, dont un presque noir et un presque blanc (flèche standard) ; hors d'un
# cadre autour du pointeur, le témoin est intact.
# (4) Ctrl+Maj+1, pointe loin du témoin, glisser dans le carré bas droit : « zone choisie »
# exacte, « pointeur hors zone » (la pointe était loin à la photo), pixels du témoin.
# (5) Fenêtres stables d'une capture à l'autre, --quitter → arret propre.
#
# Ce que la machine GitHub ne peut pas vérifier : le pointeur à 125, 150 et 200 % (un seul
# écran à 100 %), plusieurs écrans, un pointeur agrandi, coloré ou animé.
# Hors Windows : seul le démarrage est vérifié.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '11-pointeur' -Exe $Exe -Sortie $Sortie | Out-Null

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
# (l'image se lit dans « Récents »), vignette coupée, pointeur selon le cas.
function Ecrire-Reglages([string]$Dossier, [bool]$Pointeur) {
    $r = [ordered]@{
        version                   = 3
        raccourcis                = [ordered]@{ zone = @('Ctrl+Maj+1'); plein_ecran = @('Ctrl+Maj+2'); fenetre = @(); derniere_zone = @() }
        copier_apres_capture      = $true
        enregistrer_apres_capture = $false
        dossier_enregistrement    = $null
        vignette                  = [ordered]@{ afficher = $false; duree_s = 6; coin = 'bas_gauche' }
        selection                 = [ordered]@{ loupe = $false }
        capture                   = [ordered]@{ retardateur_s = 0; pointeur = $Pointeur }
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
    Verifier "(5) $Quoi : --quitter → arret propre" $true | Out-Null
}

try {
    if (-not $IsWindows) {
        $commun = Dossier-Cas 'commun'
        Ecrire-Reglages -Dossier $commun -Pointeur $true
        $p = Demarrer-Cas -Dossier $commun -Quoi 'commun'
        Fermer-Cas $p $commun 'commun'
        Write-Host '[saute] (1) à (4) : sautés hors Windows'
    }
    else {
        Add-Type -Namespace BreezyShotEssai -Name Pointeur -MemberDefinition @'
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
public static extern int GetSystemMetrics(int index);
[StructLayout(LayoutKind.Sequential)]
public struct POINT { public int X; public int Y; }
[StructLayout(LayoutKind.Sequential)]
public struct CURSORINFO { public int cbSize; public int flags; public IntPtr hCursor; public POINT pt; }
[DllImport("user32.dll", SetLastError = true)]
static extern bool GetCursorInfo(ref CURSORINFO info);
[DllImport("user32.dll")]
static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
delegate bool Rappel(IntPtr h, IntPtr p);
[DllImport("user32.dll")]
static extern bool EnumWindows(Rappel f, IntPtr p);

[StructLayout(LayoutKind.Sequential)]
struct MOUSEINPUT { public int dx; public int dy; public uint data; public uint flags; public uint time; public IntPtr extra; }
[StructLayout(LayoutKind.Sequential)]
struct INPUT { public int type; public MOUSEINPUT mi; }
[DllImport("user32.dll", SetLastError = true)]
static extern uint SendInput(uint n, INPUT[] entrees, int taille);
// Un vrai mouvement de souris (1 px puis retour) : rend le nombre d'entrées acceptées.
public static uint Bouger() {
    var e = new INPUT[2];
    e[0].type = 0; e[0].mi.dx = 1; e[0].mi.flags = 0x1;
    e[1].type = 0; e[1].mi.dx = -1; e[1].mi.flags = 0x1;
    uint n = SendInput(2, e, Marshal.SizeOf(typeof(INPUT)));
    System.Threading.Thread.Sleep(50);
    return n;
}
[StructLayout(LayoutKind.Sequential)]
struct MOUSEKEYS { public uint cbSize; public uint flags; public uint maxSpeed; public uint timeToMax; public uint ctrlSpeed; public uint r1; public uint r2; }
[DllImport("user32.dll", SetLastError = true)]
static extern bool SystemParametersInfo(uint action, uint param, ref MOUSEKEYS v, uint winIni);
static bool mouseKeysLu = false;
static MOUSEKEYS mouseKeysOrigine;
// Allume MouseKeys (MKF_MOUSEKEYSON | MKF_AVAILABLE), en gardant le réglage d'origine.
public static bool AllumerMouseKeys() {
    var m = new MOUSEKEYS(); m.cbSize = (uint)Marshal.SizeOf(typeof(MOUSEKEYS));
    if (!SystemParametersInfo(0x36, m.cbSize, ref m, 0)) return false;   // SPI_GETMOUSEKEYS
    if (!mouseKeysLu) { mouseKeysOrigine = m; mouseKeysLu = true; }
    m.flags |= 0x3;
    return SystemParametersInfo(0x37, m.cbSize, ref m, 0);               // SPI_SETMOUSEKEYS
}
// Remet MouseKeys comme avant AllumerMouseKeys (rien si jamais allumé).
public static bool RemettreMouseKeys() {
    if (!mouseKeysLu) return true;
    var m = mouseKeysOrigine;
    bool ok = SystemParametersInfo(0x37, m.cbSize, ref m, 0);
    if (ok) mouseKeysLu = false;
    return ok;
}
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
        try { [void][BreezyShotEssai.Pointeur]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms

        $CTRL_MAJ_1 = [byte[]]@(0x11, 0x10, 0x31)
        $CTRL_MAJ_2 = [byte[]]@(0x11, 0x10, 0x32)
        $COTE = 200
        $texteCouleurs = '220,40,40;40,180,60;40,80,220;240,200,30'
        $COULEURS_PLAT = [int[]]@(220, 40, 40, 40, 180, 60, 40, 80, 220, 240, 200, 30)
        $ecran = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        $utile = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        Write-Host "écran principal $ecran ; zone utile $utile"
        $TX = [int]($utile.X + 80)
        $TY = [int]($utile.Y + 80)
        # La pointe pour le pointeur : au milieu du carré haut gauche.
        $PX = $TX + 30; $PY = $TY + 30
        # Loin du témoin, pour le pointeur hors zone.
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

        # Un plein écran par Ctrl+Maj+2, la pointe posée en ($X, $Y) : rend l'image de
        # « Récents » et la ligne « plein ecran » (origine de l'image).
        function Plein-Ecran([string]$Dossier, [string]$Quoi, [int]$X, [int]$Y) {
            $recents = Join-Path $Dossier 'local/Recents'
            $avant = @(Fichiers-Png $recents).Count
            $annonces = Compter-Motif -Motif 'sortie annonce ' -Dossier $Dossier
            [BreezyShotEssai.Pointeur]::Poser($X, $Y)
            Start-Sleep -Milliseconds 150
            [BreezyShotEssai.Pointeur]::Touches($CTRL_MAJ_2)
            $null = Attendre-Motif -Motif 'sortie annonce ' -Dossier $Dossier -DelaiS 10 -Quoi "$Quoi : annonce de la sortie" -Fois ($annonces + 1)
            $pngs = @(Fichiers-Png $recents)
            if ($pngs.Count -ne $avant + 1) { throw "$Quoi : $($pngs.Count - $avant) image(s) nouvelle(s) dans Récents au lieu d'une" }
            $ligne = Derniere-Ligne -Motif 'plein ecran -?\d+,-?\d+ ' -Dossier $Dossier
            if ($ligne -notmatch 'plein ecran (-?\d+),(-?\d+) ') { throw "$Quoi : ligne « plein ecran » illisible : $ligne" }
            return [pscustomobject]@{ Image = (Lire-Png $pngs[-1].FullName); OX = [int]$Matches[1]; OY = [int]$Matches[2] }
        }

        # (1) Le pointeur est-il montré par Windows ici ? D'abord un vrai mouvement ; s'il
        # reste caché (session sans souris), MouseKeys est allumé (remis à la fin).
        function Pointeur-Montre([int]$D) { return ($D -ge 0) -and (($D -band 1) -ne 0) -and (($D -band 2) -eq 0) }
        [BreezyShotEssai.Pointeur]::Poser($LOIN[0], $LOIN[1])
        $entrees = [BreezyShotEssai.Pointeur]::Bouger()
        $drapeaux = [BreezyShotEssai.Pointeur]::DrapeauxPointeur()
        $detail = "après SetCursorPos et SendInput ($entrees entrées) : drapeaux $drapeaux"
        if (-not (Pointeur-Montre $drapeaux)) {
            $allume = [BreezyShotEssai.Pointeur]::AllumerMouseKeys()
            Start-Sleep -Milliseconds 200
            $null = [BreezyShotEssai.Pointeur]::Bouger()
            $drapeaux = [BreezyShotEssai.Pointeur]::DrapeauxPointeur()
            $detail += " ; MouseKeys allumé : $allume, drapeaux $drapeaux"
        }
        Verifier '(1) Windows montre le pointeur sur cette machine (sinon les points du pointeur ne prouvent rien)' (Pointeur-Montre $drapeaux) $detail | Out-Null

        $temoin = Lancer-Temoin

        # --- Cas « éteint » -----------------------------------------------------------------
        $de = Dossier-Cas 'eteint'
        Ecrire-Reglages -Dossier $de -Pointeur $false
        $p = Demarrer-Cas -Dossier $de -Quoi 'éteint'
        $fenetresAvant = [BreezyShotEssai.Pointeur]::FenetresDe($p.Id)

        # (2) Pointeur éteint : rien sous la pointe.
        $c = Plein-Ecran -Dossier $de -Quoi '(2)' -X $PX -Y $PY
        $i = $c.Image
        $n = [BreezyShotEssai.Pointeur]::Compter($i.Octets, $i.Stride, $i.L, $i.H, ($PX - $c.OX), ($PY - $c.OY), 32, 32, 220, 40, 40)
        Verifier '(2) pointeur éteint : le carré de 32 × 32 sous la pointe est à la couleur du témoin' ($n[0] -eq 0) "$($n[0]) pixels différents" | Out-Null
        Verifier '(2) pointeur éteint : aucune ligne « pointeur »' ((Compter-Motif -Motif 'pointeur (pris|absent|pose|hors zone)' -Dossier $de) -eq 0) | Out-Null

        $fenetresApres = [BreezyShotEssai.Pointeur]::FenetresDe($p.Id)
        Verifier '(5) éteint : fenêtres stables' ($fenetresApres -eq $fenetresAvant) "$fenetresAvant → $fenetresApres" | Out-Null
        Fermer-Cas $p $de 'éteint'

        # --- Cas « allumé » -----------------------------------------------------------------
        $da = Dossier-Cas 'allume'
        Ecrire-Reglages -Dossier $da -Pointeur $true
        $p = Demarrer-Cas -Dossier $da -Quoi 'allumé'
        $fenetresAvant = [BreezyShotEssai.Pointeur]::FenetresDe($p.Id)

        # (3) Pointeur allumé : posé sous la pointe, rien ailleurs.
        $c = Plein-Ecran -Dossier $da -Quoi '(3)' -X $PX -Y $PY
        $i = $c.Image
        $pris = Derniere-Ligne -Motif 'pointeur (pris \d+x\d+ en \d+ ms|absent .*)$' -Dossier $da
        Verifier '(3) « pointeur pris <l>x<h> en <ms> ms »' (($null -ne $pris) -and ($pris -match 'pointeur pris \d+x\d+ en \d+ ms$')) "$pris" | Out-Null
        Verifier '(3) « pointeur pose »' ((Compter-Motif -Motif 'pointeur pose$' -Dossier $da) -eq 1) | Out-Null
        $n = [BreezyShotEssai.Pointeur]::Compter($i.Octets, $i.Stride, $i.L, $i.H, ($PX - $c.OX), ($PY - $c.OY), 32, 32, 220, 40, 40)
        Verifier '(3) au moins 20 pixels changés sous la pointe, un presque noir et un presque blanc' (($n[0] -ge 20) -and ($n[1] -ge 1) -and ($n[2] -ge 1)) "différents $($n[0]), noirs $($n[1]), blancs $($n[2])" | Out-Null
        # Hors d'un cadre de 16 px avant la pointe à 64 px après (le pointeur standard fait
        # 32 × 32 à 100 %, point chaud en haut à gauche), le témoin est intact.
        $e = [BreezyShotEssai.Pointeur]::EcartsTemoin($i.Octets, $i.Stride, $i.L, $i.H, ($TX - $c.OX), ($TY - $c.OY), $COTE, $COULEURS_PLAT, ($PX - 16 - $c.OX), ($PY - 16 - $c.OY), 80, 80)
        Verifier '(3) hors du pointeur, le témoin est intact' ($e[0] -eq 0) "$($e[0]) écarts, premier en $($e[1]),$($e[2]) de l'image" | Out-Null

        # (4) Zone tracée loin de la pointe : la pointe était loin au moment de la photo.
        $gels = Compter-Motif -Motif 'selection montree \d+ ' -Dossier $da
        $pointeursAvant = Compter-Motif -Motif 'pointeur (pose|hors zone)$' -Dossier $da
        [BreezyShotEssai.Pointeur]::Poser($LOIN[0], $LOIN[1])
        Start-Sleep -Milliseconds 150
        [BreezyShotEssai.Pointeur]::Touches($CTRL_MAJ_1)
        $null = Attendre-Motif -Motif 'selection montree \d+ ' -Dossier $da -DelaiS 5 -Quoi '(4) sélection montrée' -Fois ($gels + 1)
        $null = Attendre-Motif -Motif 'chrono selection prete \d+' -Dossier $da -DelaiS 5 -Quoi '(4) page de sélection prête' -Fois ($gels + 1)
        Start-Sleep -Milliseconds 150
        # Glisser dans le carré bas droit : zone exacte, pointeur hors zone, pixels du témoin.
        $recents = Join-Path $da 'local/Recents'
        $avant = @(Fichiers-Png $recents).Count
        $annonces = Compter-Motif -Motif 'sortie annonce ' -Dossier $da
        $zx = $TX + 110; $zy = $TY + 110
        [BreezyShotEssai.Pointeur]::Glisser($zx, $zy, ($zx + 79), ($zy + 79))
        $null = Attendre-Motif -Motif 'sortie annonce ' -Dossier $da -DelaiS 10 -Quoi '(4) annonce de la sortie' -Fois ($annonces + 1)
        $zone = Derniere-Ligne -Motif 'zone choisie ' -Dossier $da
        Verifier '(4) glisser : zone choisie exacte' ($zone -match "zone choisie $zx,$zy 80x80$") "$zone" | Out-Null
        $hors = Lignes-Journal $da | Where-Object { $_ -match 'pointeur (pose|hors zone)$' } | Select-Object -Last 1
        Verifier '(4) « pointeur hors zone » (la pointe était loin à la photo)' ((Compter-Motif -Motif 'pointeur (pose|hors zone)$' -Dossier $da) -eq ($pointeursAvant + 1) -and ("$hors" -match 'pointeur hors zone$')) "$hors" | Out-Null
        $pngs = @(Fichiers-Png $recents)
        if ($pngs.Count -eq $avant + 1) {
            $i = Lire-Png $pngs[-1].FullName
            $n = [BreezyShotEssai.Pointeur]::Compter($i.Octets, $i.Stride, $i.L, $i.H, 0, 0, $i.L, $i.H, 240, 200, 30)
            Verifier '(4) image de la zone : 80 × 80, toute à la couleur du carré bas droit (sans pointeur)' (($i.L -eq 80) -and ($i.H -eq 80) -and ($n[0] -eq 0)) "$($i.L)x$($i.H), $($n[0]) pixels différents" | Out-Null
        }
        else { Verifier '(4) une image de la zone dans Récents' $false "$($pngs.Count - $avant) image(s) nouvelle(s)" | Out-Null }
        $fenetresApres = [BreezyShotEssai.Pointeur]::FenetresDe($p.Id)
        Verifier '(5) allumé : fenêtres stables' ($fenetresApres -eq $fenetresAvant) "$fenetresAvant → $fenetresApres" | Out-Null
        Fermer-Cas $p $da 'allumé'
        $null = $temoin
    }
}
catch {
    Verifier "déroulement de l'essai" $false "$_" | Out-Null
}
finally {
    if ($IsWindows -and ('BreezyShotEssai.Pointeur' -as [type])) {
        if (-not [BreezyShotEssai.Pointeur]::RemettreMouseKeys()) { Write-Host '[attention] MouseKeys non remis comme avant' }
    }
    foreach ($a in $script:Auxiliaires) { try { if (-not $a.HasExited) { $a.Kill() } } catch { } }
    Terminer-Essai
}
