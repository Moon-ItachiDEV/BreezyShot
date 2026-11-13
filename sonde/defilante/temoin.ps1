# Fenêtres témoins de la sonde défilante (Windows seulement), chacune dans son propre
# processus pwsh (une autre app que la sonde) :
#   wpf        ScrollViewer WPF, 300 lignes de 20 px (6 000 px), UI Automation complète ;
#              -Echelle 1.25 : le contenu sous une LayoutTransform (S8) ; -Courte : 10 lignes.
#   winforms   Panel à AutoScroll (une app Win32 sans UI Automation utile).
#   molette    une image qui ne défile qu'à WM_MOUSEWHEEL (ni barre, ni WM_VSCROLL, ni touche).
#   sentinelle deux fenêtres qui guettent tout défilement reçu par erreur : « cote » (à côté
#              de la cible, -X -Y -L -H) et « dessous » (tout l'écran, sous la cible).
#   fige       imitation de l'écran figé : toujours au-dessus, sans bord, tout l'écran,
#              montrant une copie de l'écran prise au lancement.
# Chaque ligne du contenu porte à gauche (8 px) la marque de natif.cs (CouleurLigne) : la
# sonde lit le décalage vrai au pixel. Écrit $Fichier (temporaire puis renommage) :
#   hwnd <h> / pid <p> / zone x,y,l,h / defile <hwnd qui défile> / contenu <px> / echelle <e>
# et, pour la sentinelle, toutes les 100 ms : molettes <n> / defilements <n> /
# touches <n> / position <px> (les deux fenêtres comptées ensemble).
# Texte ASCII seulement dans ce que les témoins affichent.

param(
    [Parameter(Mandatory)][ValidateSet('wpf', 'winforms', 'molette', 'sentinelle', 'fige')][string]$Genre,
    [int]$X = 0, [int]$Y = 0, [int]$L = 700, [int]$H = 720,
    [Parameter(Mandatory)][string]$Fichier,
    [double]$Echelle = 1.0,
    [switch]$Courte
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'natif.cs') -Raw)
$N = [BreezyShotSonde.Natif]
try { [void]$N::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }

$LIGNE = 20
$NB = if ($Courte) { 10 } else { 300 }
$CONTENU = $LIGNE * $NB

function Ecrire([string[]]$Lignes) {
    $t = "$Fichier.tmp"
    Set-Content -LiteralPath $t -Value $Lignes -Encoding ascii
    Move-Item -LiteralPath $t -Destination $Fichier -Force
}
function Texte-Ligne([int]$i) {
    $mots = 'capture', 'page', 'molette', 'image', 'ligne', 'zone', 'ecran', 'temoin'
    $t = 'Ligne {0:D4} - ' -f ($i + 1)
    for ($k = 0; $k -lt 5; $k++) { $t += $mots[($i * 3 + $k * 5) % $mots.Count] + ' ' }
    return $t + ('- {0}' -f ($i * 37 % 1000))
}

# Le contenu en image (WinForms) : bande, lignes alternées, texte.
function Image-Contenu([int]$Largeur, [int]$Hauteur) {
    Add-Type -AssemblyName System.Drawing
    $b = [System.Drawing.Bitmap]::new($Largeur, $Hauteur, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($b)
    $g.Clear([System.Drawing.Color]::White)
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit
    $police = [System.Drawing.Font]::new('Segoe UI', [single]13, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
    $gris = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(255, 241, 244, 249))
    for ($i = 0; $i -lt [int]($Hauteur / $LIGNE); $i++) {
        if ($i % 5 -eq 4) { $g.FillRectangle($gris, 8, $i * $LIGNE, $Largeur - 8, $LIGNE) }
        $g.DrawString((Texte-Ligne $i), $police, [System.Drawing.Brushes]::Black, [single]14, [single]($i * $LIGNE + 1))
    }
    $g.Dispose(); $police.Dispose(); $gris.Dispose()
    $r = [System.Drawing.Rectangle]::new(0, 0, $Largeur, $Hauteur)
    $d = $b.LockBits($r, [System.Drawing.Imaging.ImageLockMode]::WriteOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    try { $N::PeindreBande($d.Scan0, $d.Stride, 8, $Hauteur, 0) } finally { $b.UnlockBits($d) }
    return $b
}

function Rect-Texte($x, $y, $l, $h) { return "$([int]$x),$([int]$y),$([int]$l),$([int]$h)" }

if ($Genre -eq 'wpf') {
    Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase
    $f = [System.Windows.Window]::new()
    $f.Title = 'Temoin WPF BreezyShot'
    $f.WindowStartupLocation = [System.Windows.WindowStartupLocation]::Manual
    # WPF place en pixels logiques : ramenés aux pixels réels demandés.
    $k = [double]$N::DpiEcran($X + 10, $Y + 10) / 96
    if ($k -le 0) { $k = 1.0 }
    $f.Left = $X / $k; $f.Top = $Y / $k; $f.Width = $L / $k; $f.Height = $H / $k
    $f.ShowInTaskbar = $true
    $f.UseLayoutRounding = $true
    [System.Windows.Media.TextOptions]::SetTextFormattingMode($f, [System.Windows.Media.TextFormattingMode]::Display)
    $sv = [System.Windows.Controls.ScrollViewer]::new()
    $sv.VerticalScrollBarVisibility = [System.Windows.Controls.ScrollBarVisibility]::Visible
    $sv.HorizontalScrollBarVisibility = [System.Windows.Controls.ScrollBarVisibility]::Disabled
    $sv.Background = [System.Windows.Media.Brushes]::White
    $grille = [System.Windows.Controls.Grid]::new()
    $c0 = [System.Windows.Controls.ColumnDefinition]::new(); $c0.Width = [System.Windows.GridLength]::new(8)
    $c1 = [System.Windows.Controls.ColumnDefinition]::new()
    $grille.ColumnDefinitions.Add($c0); $grille.ColumnDefinitions.Add($c1)
    $wb = [System.Windows.Media.Imaging.WriteableBitmap]::new(8, $CONTENU, 96, 96, [System.Windows.Media.PixelFormats]::Bgra32, $null)
    $wb.WritePixels([System.Windows.Int32Rect]::new(0, 0, 8, $CONTENU), $N::Bande(8, $CONTENU), 32, 0)
    $img = [System.Windows.Controls.Image]::new()
    $img.Source = $wb; $img.Width = 8; $img.Height = $CONTENU
    $img.Stretch = [System.Windows.Media.Stretch]::Fill
    $img.VerticalAlignment = [System.Windows.VerticalAlignment]::Top
    [System.Windows.Media.RenderOptions]::SetBitmapScalingMode($img, [System.Windows.Media.BitmapScalingMode]::NearestNeighbor)
    [System.Windows.Controls.Grid]::SetColumn($img, 0)
    $pile = [System.Windows.Controls.StackPanel]::new()
    $gris = [System.Windows.Media.SolidColorBrush]::new([System.Windows.Media.Color]::FromRgb(241, 244, 249))
    for ($i = 0; $i -lt $NB; $i++) {
        $t = [System.Windows.Controls.TextBlock]::new()
        $t.Text = Texte-Ligne $i
        $t.Height = $LIGNE
        $t.FontFamily = [System.Windows.Media.FontFamily]::new('Segoe UI')
        $t.FontSize = 13
        $t.Padding = [System.Windows.Thickness]::new(6, 1, 0, 0)
        if ($i % 5 -eq 4) { $t.Background = $gris }
        $pile.Children.Add($t) | Out-Null
    }
    [System.Windows.Controls.Grid]::SetColumn($pile, 1)
    $grille.Children.Add($img) | Out-Null
    $grille.Children.Add($pile) | Out-Null
    if ($Echelle -ne 1.0) { $grille.LayoutTransform = [System.Windows.Media.ScaleTransform]::new($Echelle, $Echelle) }
    $sv.Content = $grille
    $f.Content = $sv
    $f.Add_ContentRendered({
            $p = $sv.PointToScreen([System.Windows.Point]::new(0, 0))
            $src = [System.Windows.PresentationSource]::FromVisual($sv)
            $m = $src.CompositionTarget.TransformToDevice
            $hwnd = ([System.Windows.Interop.WindowInteropHelper]::new($f)).Handle
            Ecrire @("hwnd $([long]$hwnd)", "pid $PID",
                "zone $(Rect-Texte $p.X $p.Y ($sv.ViewportWidth * $m.M11) ($sv.ViewportHeight * $m.M22))",
                "defile $([long]$hwnd)", "contenu $([int]($CONTENU * $Echelle * $m.M22))", "echelle $($Echelle * $m.M22)")
        })
    $app = [System.Windows.Application]::new()
    [void]$app.Run($f)
    exit 0
}

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
try { [void][System.Windows.Forms.Application]::SetHighDpiMode([System.Windows.Forms.HighDpiMode]::PerMonitorV2) } catch { }

function Nouvelle-Forme([string]$Titre, [int]$fx, [int]$fy, [int]$fl, [int]$fh) {
    $f = [System.Windows.Forms.Form]::new()
    $f.AutoScaleMode = [System.Windows.Forms.AutoScaleMode]::None
    $f.Text = $Titre
    $f.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
    $f.Bounds = [System.Drawing.Rectangle]::new($fx, $fy, $fl, $fh)
    $f.BackColor = [System.Drawing.Color]::White
    return $f
}

if ($Genre -eq 'fige') {
    $ecran = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $photo = [System.Drawing.Bitmap]::new($ecran.Width, $ecran.Height)
    $g = [System.Drawing.Graphics]::FromImage($photo)
    $g.CopyFromScreen($ecran.X, $ecran.Y, 0, 0, $ecran.Size)
    $g.Dispose()
    $f = Nouvelle-Forme 'Ecran fige BreezyShot sonde' $ecran.X $ecran.Y $ecran.Width $ecran.Height
    $f.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::None
    $f.TopMost = $true
    $f.ShowInTaskbar = $false
    $f.BackgroundImage = $photo
    $f.BackgroundImageLayout = [System.Windows.Forms.ImageLayout]::None
    $f.Add_Shown({
            $f.Bounds = [System.Drawing.Rectangle]::new($ecran.X, $ecran.Y, $ecran.Width, $ecran.Height)
            Ecrire @("hwnd $([long]$f.Handle)", "pid $PID", "zone $(Rect-Texte $ecran.X $ecran.Y $ecran.Width $ecran.Height)")
        })
    [System.Windows.Forms.Application]::Run($f)
    exit 0
}

if ($Genre -eq 'winforms') {
    $f = Nouvelle-Forme 'Temoin WinForms BreezyShot' $X $Y $L $H
    $p = [System.Windows.Forms.Panel]::new()
    $p.Dock = [System.Windows.Forms.DockStyle]::Fill
    $p.AutoScroll = $true
    $pb = [System.Windows.Forms.PictureBox]::new()
    $pb.Location = [System.Drawing.Point]::new(0, 0)
    $pb.Size = [System.Drawing.Size]::new($L - 40, $CONTENU)
    $pb.Image = Image-Contenu ($L - 40) $CONTENU
    $p.Controls.Add($pb)
    $f.Controls.Add($p)
    $f.Add_Shown({
            $r = $p.RectangleToScreen($p.ClientRectangle)
            Ecrire @("hwnd $([long]$f.Handle)", "pid $PID", "zone $(Rect-Texte $r.X $r.Y $r.Width $r.Height)",
                "defile $([long]$p.Handle)", "contenu $CONTENU", 'echelle 1')
        })
    [System.Windows.Forms.Application]::Run($f)
    exit 0
}

if ($Genre -eq 'molette') {
    $f = Nouvelle-Forme 'Temoin molette BreezyShot' $X $Y $L $H
    $image = Image-Contenu ($L - 20) $CONTENU
    $script:decalage = 0
    $pb = [System.Windows.Forms.PictureBox]::new()
    $pb.Dock = [System.Windows.Forms.DockStyle]::Fill
    $pb.Add_Paint({
            param($s, $e)
            $e.Graphics.Clear([System.Drawing.Color]::White)
            $e.Graphics.DrawImageUnscaled($image, 0, - $script:decalage)
        })
    $pb.Add_MouseWheel({
            param($s, $e)
            $max = [math]::Max(0, $CONTENU - $pb.ClientSize.Height)
            $script:decalage = [math]::Min($max, [math]::Max(0, $script:decalage - [int]($e.Delta / 120 * 48)))
            if ($e -is [System.Windows.Forms.HandledMouseEventArgs]) { $e.Handled = $true }
            $pb.Invalidate(); $pb.Update()
        })
    $f.Controls.Add($pb)
    $f.Add_Shown({
            $r = $pb.RectangleToScreen($pb.ClientRectangle)
            Ecrire @("hwnd $([long]$f.Handle)", "pid $PID", "zone $(Rect-Texte $r.X $r.Y $r.Width $r.Height)",
                "defile $([long]$pb.Handle)", "contenu $CONTENU", 'echelle 1')
        })
    [System.Windows.Forms.Application]::Run($f)
    exit 0
}

# Sentinelle : deux fenêtres, chacune avec un Panel à AutoScroll ; tout ce qui les fait
# défiler, et toute molette ou touche qu'elles reçoivent, est compté.
$script:molettes = 0
$script:defilements = 0
$script:touches = 0
$panneaux = [System.Collections.Generic.List[object]]::new()
function Sentinelle([string]$Titre, [int]$fx, [int]$fy, [int]$fl, [int]$fh) {
    $f = Nouvelle-Forme $Titre $fx $fy $fl $fh
    $f.KeyPreview = $true
    $f.Add_KeyDown({ $script:touches++ })
    $p = [System.Windows.Forms.Panel]::new()
    $p.Dock = [System.Windows.Forms.DockStyle]::Fill
    $p.AutoScroll = $true
    $pb = [System.Windows.Forms.PictureBox]::new()
    $pb.Size = [System.Drawing.Size]::new([math]::Max(100, $fl - 40), $CONTENU)
    $pb.Image = Image-Contenu ([math]::Max(100, $fl - 40)) $CONTENU
    $p.Controls.Add($pb)
    $p.Add_MouseWheel({ $script:molettes++ })
    $p.Add_Scroll({ $script:defilements++ })
    $f.Controls.Add($p)
    $panneaux.Add($p)
    return $f
}
$ecran = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
$dessous = Sentinelle 'Sentinelle dessous BreezyShot' $ecran.X $ecran.Y $ecran.Width $ecran.Height
$cote = Sentinelle 'Sentinelle cote BreezyShot' $X $Y $L $H
$minuterie = [System.Windows.Forms.Timer]::new()
$minuterie.Interval = 100
$minuterie.Add_Tick({
        $pos = 0
        foreach ($p in $panneaux) { $pos += [math]::Abs($p.AutoScrollPosition.Y) }
        $r = $panneaux[1].RectangleToScreen($panneaux[1].ClientRectangle)
        Ecrire @("hwnd $([long]$cote.Handle)", "dessous $([long]$dessous.Handle)", "pid $PID",
            "zone $(Rect-Texte $r.X $r.Y $r.Width $r.Height)",
            "molettes $script:molettes", "defilements $script:defilements", "touches $script:touches", "position $pos")
    })
$dessous.Add_Shown({ $cote.Show(); $minuterie.Start() })
[System.Windows.Forms.Application]::Run($dessous)
