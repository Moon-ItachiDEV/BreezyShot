# Aide de l'essai 14 (Windows seulement) : le temoin de la capture defilante.
# PowerShell 7, texte ASCII seulement. Fenetre WPF sans bord, toujours au-dessus, absente de
# la barre des taches, posee en ($X, $Y) en pixels reels. Un ScrollViewer sans lissage
# (defilement physique, 16 px par ligne d'UI Automation, barre cachee : la page visible fait
# exactement $Largeur x $Hauteur pixels) sur $Lignes lignes de 50 px, chacune une image
# exacte (aucun texte, aucun lissage : le PNG de l'app se verifie ligne par ligne).
#
# Contenu, page de 600 px de large (formule lue aussi par le decodeur de l'essai 14) :
#   ligne k = y / 50 (k < 512), rangee r = y % 50 dans la ligne ;
#   x 0 a 239 : couleur de la ligne (R, V, B) = (64 + 20 (k & 7), 64 + 20 ((k >> 3) & 7),
#     64 + 20 ((k >> 6) & 7)), et, sur les rangees 17 a 32, 12 carres de 16 px en
#     x = 16 + 18 i qui ecrivent k (bit 11 - i ; 1 noir, 0 blanc) : la ligne numerotee ;
#   x 240 a 599 : 45 cases de 8 px, noires (1) ou blanches (0) ; les cases 0 a 15 ecrivent
#     y (bit 15 - case) ; les cases 16 a 44 sont un bruit fixe tire de (y, case) : deux
#     rangees de la page ne se ressemblent jamais (une rangee decalee differe sur environ
#     20 % de ses pixels, au-dela de la tolerance de l'assemblage).
#   -EnTete : 40 px fixes au-dessus de la liste, damier de 8 px de couleurs reservees
#     (255, 0, 255) et (128, 0, 128) (vert nul : jamais une couleur de ligne).
#
# Options : -Immobile (9 lignes, exactement la hauteur visible : rien ne defile, ni par UI
# Automation ni par la molette) ; -Lignes <n> (page plus longue, pour la limite) ;
# -Largeur <l> / -Hauteur <h> (fenetre plus petite : une sentinelle) ; -Png <dossier> :
# ecrit page-<Lignes>.bmp (600 x 50 Lignes) et entete.bmp (600 x 40), 24 bits, sans fenetre,
# puis sort (page de reference de l'essai, et page servie a Edge).
#
# La molette : chaque cran recu (WM_MOUSEWHEEL, de l'essai ou de l'app) est compte, puis
# applique toutes les 60 ms (crans x lignes de la molette du systeme x 16 px) : vingt crans
# envoyes d'un coup font un seul saut, comme une molette rapide ; un cran seul bouge la page
# en 60 ms au plus. UI Automation (Scroll, SmallIncrement) agit tout de suite.
#
# Ecrit dans $Fichier (temporaire puis renommage, toutes les 40 ms s'il y a du nouveau) :
#   hwnd <h> / pid <p> / fenetre x,y lxh / page x,y lxh / entete x,y lxh (avec -EnTete) /
#   contenu <px> / visible <px> / fait <n> / crans <n> / position <p> <ms> (une ligne par
#   changement de la position, en pixels, avec l'heure en millisecondes Unix UTC).
# Commande : l'essai ecrit "haut <n>" dans "$Fichier.cmd" ; le temoin remonte en haut,
# efface la commande et ecrit "fait <n>".
#   pwsh -NoProfile -File essais/temoin-defilant.ps1 -X 40 -Y 80 -Fichier t.txt [-EnTete]

param(
    [string]$Fichier = '',
    [int]$X = 40,
    [int]$Y = 80,
    [int]$Lignes = 120,
    [int]$Largeur = 600,
    [int]$Hauteur = 450,
    [switch]$EnTete,
    [switch]$Immobile,
    [string]$Png = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
namespace BreezyShotEssai {
public static class PageDefilante {
    public const int Largeur = 600;
    public const int HauteurLigne = 50;
    public const int HauteurEnTete = 40;
    [System.Runtime.InteropServices.DllImport("user32.dll", SetLastError = true)]
    public static extern bool SetProcessDpiAwarenessContext(System.IntPtr contexte);

    public static void Couleur(int k, out byte r, out byte v, out byte b) {
        r = (byte)(64 + 20 * (k & 7));
        v = (byte)(64 + 20 * ((k >> 3) & 7));
        b = (byte)(64 + 20 * ((k >> 6) & 7));
    }
    // Bruit fixe : un bit tire de (y, case).
    public static bool Bruit(int y, int c) {
        uint h = unchecked((uint)y * 2654435761u) ^ unchecked((uint)c * 40503u + 0x9E3779B9u);
        h ^= h >> 15; h = unchecked(h * 2246822519u); h ^= h >> 13;
        return (h & 1u) != 0;
    }
    // Vrai si le pixel (x, y) de la page est noir dans les carres ou les cases ; faux pour la
    // couleur de la ligne (x < 240) ou le blanc (x >= 240).
    static int Genre(int x, int y) {
        int k = y / HauteurLigne, r = y % HauteurLigne;
        if (x < 240) {
            if (r >= 17 && r < 33 && x >= 16 && x < 16 + 12 * 18 && (x - 16) % 18 < 16) {
                int i = (x - 16) / 18;
                return ((k >> (11 - i)) & 1) != 0 ? 1 : 2;   // 1 noir, 2 blanc
            }
            return 0;                                          // couleur de la ligne
        }
        int c = (x - 240) / 8;
        bool noir = c < 16 ? ((y >> (15 - c)) & 1) != 0 : Bruit(y, c);
        return noir ? 1 : 2;
    }
    // Les rangees [y0, y0 + n) de la page, BGRA (Bgra32 de WPF), 600 px de large.
    public static byte[] Bande(int y0, int n) {
        var o = new byte[Largeur * n * 4];
        for (int j = 0; j < n; j++) {
            int y = y0 + j;
            byte r, v, b; Couleur(y / HauteurLigne, out r, out v, out b);
            for (int x = 0; x < Largeur; x++) {
                int p = (j * Largeur + x) * 4;
                int g = Genre(x, y);
                if (g == 0) { o[p] = b; o[p + 1] = v; o[p + 2] = r; }
                else if (g == 1) { o[p] = 0; o[p + 1] = 0; o[p + 2] = 0; }
                else { o[p] = 255; o[p + 1] = 255; o[p + 2] = 255; }
                o[p + 3] = 255;
            }
        }
        return o;
    }
    // L'en-tete : 600 x 40, damier de 8 px (255, 0, 255) et (128, 0, 128), BGRA.
    public static byte[] EnTete() {
        var o = new byte[Largeur * HauteurEnTete * 4];
        for (int y = 0; y < HauteurEnTete; y++)
            for (int x = 0; x < Largeur; x++) {
                int p = (y * Largeur + x) * 4;
                byte c = ((x / 8 + y / 8) % 2 == 0) ? (byte)255 : (byte)128;
                o[p] = c; o[p + 1] = 0; o[p + 2] = c; o[p + 3] = 255;
            }
        return o;
    }
    // BGRA (4 octets par pixel) vers BGR (3 octets, lignes de "pas" octets) pour un BMP 24 bits.
    public static void VersBgr(byte[] bgra, int largeur, int hauteur, System.IntPtr dest, int pas) {
        var ligne = new byte[pas];
        for (int y = 0; y < hauteur; y++) {
            for (int x = 0; x < largeur; x++) {
                int s = (y * largeur + x) * 4;
                ligne[x * 3] = bgra[s]; ligne[x * 3 + 1] = bgra[s + 1]; ligne[x * 3 + 2] = bgra[s + 2];
            }
            System.Runtime.InteropServices.Marshal.Copy(ligne, 0, System.IntPtr.Add(dest, y * pas), pas);
        }
    }
}
}
'@

$P = [BreezyShotEssai.PageDefilante]
$LIGNE = $P::HauteurLigne
if ($Immobile) { $Lignes = [int][math]::Floor($Hauteur / $LIGNE) }
if ($Lignes -lt 1 -or $Lignes -gt 511) { throw "temoin defilant : -Lignes $Lignes hors de 1 a 511" }
$CONTENU = $Lignes * $LIGNE

# --- Mode -Png : la page de reference en BMP 24 bits, sans fenetre ----------------------------
if ($Png -ne '') {
    Add-Type -AssemblyName System.Drawing
    New-Item -ItemType Directory -Force -Path $Png | Out-Null
    function Ecrire-Bmp([byte[]]$Bgra, [int]$Larg, [int]$Haute, [string]$Chemin) {
        $b = [System.Drawing.Bitmap]::new($Larg, $Haute, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
        try {
            $r = [System.Drawing.Rectangle]::new(0, 0, $Larg, $Haute)
            $d = $b.LockBits($r, [System.Drawing.Imaging.ImageLockMode]::WriteOnly, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
            try { $P::VersBgr($Bgra, $Larg, $Haute, $d.Scan0, $d.Stride) } finally { $b.UnlockBits($d) }
            $t = "$Chemin.tmp"
            $b.Save($t, [System.Drawing.Imaging.ImageFormat]::Bmp)
            Move-Item -LiteralPath $t -Destination $Chemin -Force
        }
        finally { $b.Dispose() }
    }
    Ecrire-Bmp ($P::EnTete()) $P::Largeur $P::HauteurEnTete (Join-Path $Png 'entete.bmp')
    Ecrire-Bmp ($P::Bande(0, $CONTENU)) $P::Largeur $CONTENU (Join-Path $Png "page-$Lignes.bmp")
    exit 0
}

if ($Fichier -eq '') { throw 'temoin defilant : -Fichier manque' }
try { [void]$P::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }
Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase

# Toute exception d'un gestionnaire va a la sortie (le journal de l'essai), jamais dans une boite.
function Signaler([string]$Ou, $Ex) {
    try { [Console]::Out.WriteLine("temoin defilant (pid $PID) : exception dans $Ou : $($Ex.GetType().FullName) : $($Ex.Message)") } catch { }
}

$script:positions = [System.Collections.Generic.List[string]]::new()
$script:sale = $true
$script:fait = 0
$script:crans = 0
$script:enAttente = 0
$script:base = [System.Collections.Generic.List[string]]::new()

function Ecrire-Etat {
    $lignesEtat = [System.Collections.Generic.List[string]]::new()
    foreach ($b in $script:base) { $lignesEtat.Add($b) }
    $lignesEtat.Add("fait $($script:fait)")
    $lignesEtat.Add("crans $($script:crans)")
    foreach ($pos in $script:positions) { $lignesEtat.Add($pos) }
    $t = "$Fichier.tmp"
    [System.IO.File]::WriteAllLines($t, $lignesEtat, [System.Text.Encoding]::ASCII)
    for ($i = 0; $i -lt 20; $i++) {
        try { [System.IO.File]::Move($t, $Fichier, $true); $script:sale = $false; return }
        catch { Start-Sleep -Milliseconds 5 }
    }
}
function Heure-Ms { return [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() }
function Rect-Texte([double]$rx, [double]$ry, [double]$rl, [double]$rh) {
    return "$([int][math]::Round($rx)),$([int][math]::Round($ry)) $([int][math]::Round($rl))x$([int][math]::Round($rh))"
}

$f = [System.Windows.Window]::new()
$f.Title = 'Temoin defilant BreezyShot'
$f.WindowStyle = [System.Windows.WindowStyle]::None
$f.ResizeMode = [System.Windows.ResizeMode]::NoResize
$f.ShowInTaskbar = $false
$f.Topmost = $true
$f.ShowActivated = $true
$f.UseLayoutRounding = $true
$f.SnapsToDevicePixels = $true
$f.Background = [System.Windows.Media.Brushes]::White
$f.WindowStartupLocation = [System.Windows.WindowStartupLocation]::Manual
# WPF place en pixels logiques : la machine d'essai est a 96 ppp. L'etat ecrit l'echelle lue ;
# l'essai 14 refuse de continuer si elle n'est pas 1 (les positions seraient fausses).
$k = 1.0
$haut = if ($EnTete) { $P::HauteurEnTete } else { 0 }
$f.Left = $X / $k; $f.Top = $Y / $k
$f.Width = $Largeur / $k; $f.Height = ($Hauteur + $haut) / $k

function Image-De([byte[]]$Bgra, [int]$Larg, [int]$Haute) {
    $wb = [System.Windows.Media.Imaging.WriteableBitmap]::new($Larg, $Haute, 96, 96, [System.Windows.Media.PixelFormats]::Bgra32, $null)
    $wb.WritePixels([System.Windows.Int32Rect]::new(0, 0, $Larg, $Haute), $Bgra, $Larg * 4, 0)
    $wb.Freeze()
    $img = [System.Windows.Controls.Image]::new()
    $img.Source = $wb
    $img.Width = $Larg; $img.Height = $Haute
    $img.Stretch = [System.Windows.Media.Stretch]::None
    $img.HorizontalAlignment = [System.Windows.HorizontalAlignment]::Left
    $img.VerticalAlignment = [System.Windows.VerticalAlignment]::Top
    $img.SnapsToDevicePixels = $true
    [System.Windows.Media.RenderOptions]::SetBitmapScalingMode($img, [System.Windows.Media.BitmapScalingMode]::NearestNeighbor)
    return $img
}

$racine = [System.Windows.Controls.DockPanel]::new()
$racine.LastChildFill = $true
$imgEnTete = $null
if ($EnTete) {
    $imgEnTete = Image-De ($P::EnTete()) $P::Largeur $P::HauteurEnTete
    [System.Windows.Controls.DockPanel]::SetDock($imgEnTete, [System.Windows.Controls.Dock]::Top)
    $racine.Children.Add($imgEnTete) | Out-Null
}
$sv = [System.Windows.Controls.ScrollViewer]::new()
$sv.CanContentScroll = $false
$sv.HorizontalScrollBarVisibility = [System.Windows.Controls.ScrollBarVisibility]::Disabled
$sv.VerticalScrollBarVisibility = if ($Immobile) { [System.Windows.Controls.ScrollBarVisibility]::Disabled } else { [System.Windows.Controls.ScrollBarVisibility]::Hidden }
$sv.PanningMode = [System.Windows.Controls.PanningMode]::None
$sv.Background = [System.Windows.Media.Brushes]::White
$pile = [System.Windows.Controls.StackPanel]::new()
$pile.HorizontalAlignment = [System.Windows.HorizontalAlignment]::Left
for ($i = 0; $i -lt $Lignes; $i++) {
    $pile.Children.Add((Image-De ($P::Bande($i * $LIGNE, $LIGNE)) $P::Largeur $LIGNE)) | Out-Null
}
$sv.Content = $pile
$racine.Children.Add($sv) | Out-Null
$f.Content = $racine

# Position du defilement : une ligne par changement, avec l'heure.
$sv.Add_ScrollChanged({
        param($s, $e)
        try {
            if ($e.VerticalChange -ne 0) {
                $script:positions.Add("position $([int][math]::Round($sv.VerticalOffset)) $(Heure-Ms)")
                $script:sale = $true
            }
        }
        catch { Signaler 'ScrollChanged' $_.Exception }
    })
# La molette : comptee ici, appliquee par la minuterie.
$sv.Add_PreviewMouseWheel({
        param($s, $e)
        try {
            $e.Handled = $true
            $script:enAttente += $e.Delta
            $script:crans++
            $script:sale = $true
        }
        catch { Signaler 'PreviewMouseWheel' $_.Exception }
    })

$molette = [System.Windows.Threading.DispatcherTimer]::new()
$molette.Interval = [TimeSpan]::FromMilliseconds(60)
$molette.Add_Tick({
        try {
            $n = [int][math]::Truncate($script:enAttente / 120)
            if ($n -ne 0) {
                $script:enAttente -= $n * 120
                $parCran = [math]::Max(1, [System.Windows.SystemParameters]::WheelScrollLines) * 16
                $cible = [math]::Min($sv.ScrollableHeight, [math]::Max(0, $sv.VerticalOffset - $n * $parCran))
                $sv.ScrollToVerticalOffset($cible)
            }
        }
        catch { Signaler 'molette' $_.Exception }
    })

$etat = [System.Windows.Threading.DispatcherTimer]::new()
$etat.Interval = [TimeSpan]::FromMilliseconds(40)
$etat.Add_Tick({
        try {
            $cmd = "$Fichier.cmd"
            if ([System.IO.File]::Exists($cmd)) {
                $t = ''
                try { $t = [System.IO.File]::ReadAllText($cmd).Trim() } catch { }
                if ($t -match '^haut (\d+)$') {
                    $sv.ScrollToVerticalOffset(0)
                    $sv.UpdateLayout()
                    $script:enAttente = 0
                    $script:fait = [int]$Matches[1]
                    try { [System.IO.File]::Delete($cmd) } catch { }
                    $script:sale = $true
                }
            }
            if ($script:sale -and $script:base.Count -gt 0) { Ecrire-Etat }
        }
        catch { Signaler 'etat' $_.Exception }
    })

$f.Add_ContentRendered({
        try {
            $hwnd = ([System.Windows.Interop.WindowInteropHelper]::new($f)).Handle
            $src = [System.Windows.PresentationSource]::FromVisual($sv)
            $m = $src.CompositionTarget.TransformToDevice
            $pf = $f.PointToScreen([System.Windows.Point]::new(0, 0))
            $pp = $sv.PointToScreen([System.Windows.Point]::new(0, 0))
            $script:base.Add("hwnd $([long]$hwnd)")
            $script:base.Add("pid $PID")
            $script:base.Add("fenetre $(Rect-Texte $pf.X $pf.Y ($f.ActualWidth * $m.M11) ($f.ActualHeight * $m.M22))")
            $script:base.Add("page $(Rect-Texte $pp.X $pp.Y ($sv.ViewportWidth * $m.M11) ($sv.ViewportHeight * $m.M22))")
            if ($null -ne $imgEnTete) {
                $pe = $imgEnTete.PointToScreen([System.Windows.Point]::new(0, 0))
                $script:base.Add("entete $(Rect-Texte $pe.X $pe.Y ($imgEnTete.ActualWidth * $m.M11) ($imgEnTete.ActualHeight * $m.M22))")
            }
            $script:base.Add("contenu $([int]($CONTENU * $m.M22))")
            $script:base.Add("visible $([int]($sv.ViewportHeight * $m.M22))")
            $script:base.Add("echelle $($m.M22)")
            $script:sale = $true
            $molette.Start()
            $etat.Start()
            Ecrire-Etat
        }
        catch { Signaler 'ContentRendered' $_.Exception }
    })

$app = [System.Windows.Application]::new()
$app.ShutdownMode = [System.Windows.ShutdownMode]::OnMainWindowClose
$app.Add_DispatcherUnhandledException({ param($s, $e) Signaler 'WPF (Dispatcher)' $e.Exception; $e.Handled = $true })
[void]$app.Run($f)
exit 0
