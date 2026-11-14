# Fenêtres témoins de la sonde du film (contrat 0.7, §1.0), chacune dans son propre
# processus pwsh (une autre app que la sonde) :
#   simple  fenêtre WPF sans bordure, toujours au-dessus, zone cliente de 600 × 450 pixels
#           réels : le compteur k (qui avance toutes les -PeriodeMs, 100 ms par défaut)
#           écrit en 16 carrés noirs ou blancs, une bande de la couleur couleur(k), quatre
#           pastilles de couleurs fixes (natif.cs, Temoin : le même dessin que les exemples
#           Rust) ; -Rapide : k avance à chaque image composée (CompositionTarget.Rendering) ;
#   fixe    la même, rien ne bouge (k reste 0) ;
#   plein   la même, tout le fond change à chaque pas (dégradé de plus de 256 couleurs) ;
#   cible   fenêtre WinForms sur tout l'écran (sous la fenêtre de contrôle de S5) qui
#           compte les clics, les crans de molette et les touches reçus.
# Écrit $Fichier (temporaire puis renommage) : hwnd <h> / pid <p> / zone x,y,l,h / journal <f>,
# puis (cible) toutes les 100 ms : clics / molettes / touches / premier_plan. Le journal
# ($Fichier.k) reçoit une ligne « k <n> <ms> » par pas, <ms> lu sur l'horloge de
# performance (QueryPerformanceCounter, la même que celle des exemples Rust).
# C'est le futur essais/temoin-anime.ps1 de l'essai 15.

param(
    [Parameter(Mandatory)][ValidateSet('simple', 'fixe', 'plein', 'cible')][string]$Genre,
    [int]$X = 100, [int]$Y = 100,
    [int]$PeriodeMs = 100,
    [switch]$Rapide,
    [Parameter(Mandatory)][string]$Fichier
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'natif.cs') -Raw)
$N = [BreezyShotFilm.Natif]
$T = [BreezyShotFilm.Temoin]
try { [void]$N::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }

# Jamais d'exception visible : une boîte d'erreur se mettrait devant les images.
function Signaler([string]$Ou, $Ex) {
    try { [Console]::Out.WriteLine("témoin $Genre (pid $PID) : exception dans $Ou : $($Ex.GetType().FullName) : $($Ex.Message)") } catch { }
}
function Ecrire([string[]]$Lignes) {
    try {
        $t = "$Fichier.tmp"
        [System.IO.File]::WriteAllLines($t, $Lignes, [System.Text.Encoding]::ASCII)
        for ($i = 0; $i -lt 10; $i++) {
            try { [System.IO.File]::Move($t, $Fichier, $true); return }
            catch { Start-Sleep -Milliseconds 15 }
        }
    }
    catch { Signaler "écriture de l'état" $_.Exception }
}

if ($Genre -eq 'cible') {
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    [System.Windows.Forms.Application]::SetUnhandledExceptionMode([System.Windows.Forms.UnhandledExceptionMode]::CatchException)
    [System.Windows.Forms.Application]::add_ThreadException({ param($s, $e) Signaler 'WinForms' $e.Exception })
    $ecran = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
    $f = [System.Windows.Forms.Form]::new()
    $f.AutoScaleMode = [System.Windows.Forms.AutoScaleMode]::None
    $f.Text = 'Cible film BreezyShot'
    $f.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
    $f.Bounds = $ecran
    $f.BackColor = [System.Drawing.Color]::FromArgb(255, 236, 240, 244)
    $f.KeyPreview = $true
    $script:clics = 0; $script:molettes = 0; $script:touches = 0
    $f.Add_MouseDown({ try { $script:clics++ } catch { } })
    $f.Add_MouseWheel({ try { $script:molettes++ } catch { } })
    $f.Add_KeyDown({ try { $script:touches++ } catch { } })
    $m = [System.Windows.Forms.Timer]::new()
    $m.Interval = 100
    $m.Add_Tick({
            try {
                $pp = [int]($N::GetForegroundWindow() -eq $f.Handle)
                Ecrire @("hwnd $([long]$f.Handle)", "pid $PID", "zone $($ecran.X),$($ecran.Y),$($ecran.Width),$($ecran.Height)",
                    "clics $script:clics", "molettes $script:molettes", "touches $script:touches", "premier_plan $pp")
            }
            catch { Signaler 'minuterie' $_.Exception }
        })
    $f.Add_Shown({ try { $m.Start() } catch { Signaler 'Shown' $_.Exception } })
    [System.Windows.Forms.Application]::Run($f)
    exit 0
}

Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase
$genreN = switch ($Genre) { 'simple' { 0 } 'fixe' { 1 } 'plein' { 2 } }
$L = $T::L; $H = $T::H
$f = [System.Windows.Window]::new()
$f.Title = "Temoin film BreezyShot $Genre"
$f.WindowStyle = [System.Windows.WindowStyle]::None
$f.ResizeMode = [System.Windows.ResizeMode]::NoResize
$f.Topmost = $true
$f.ShowInTaskbar = $true
$f.UseLayoutRounding = $true
$f.WindowStartupLocation = [System.Windows.WindowStartupLocation]::Manual
# WPF place en pixels logiques : ramenés aux pixels réels demandés.
$k0 = [double]$N::DpiEcran($X + 10, $Y + 10) / 96
if ($k0 -le 0) { $k0 = 1.0 }
$f.Left = $X / $k0; $f.Top = $Y / $k0; $f.Width = $L / $k0; $f.Height = $H / $k0
$wb = [System.Windows.Media.Imaging.WriteableBitmap]::new($L, $H, 96 * $k0, 96 * $k0, [System.Windows.Media.PixelFormats]::Bgra32, $null)
$tampon = [byte[]]::new($L * $H * 4)
$T::Peindre($tampon, 0, $genreN)
$wb.WritePixels([System.Windows.Int32Rect]::new(0, 0, $L, $H), $tampon, $L * 4, 0)
$img = [System.Windows.Controls.Image]::new()
$img.Source = $wb
$img.Stretch = [System.Windows.Media.Stretch]::None
[System.Windows.Media.RenderOptions]::SetBitmapScalingMode($img, [System.Windows.Media.BitmapScalingMode]::NearestNeighbor)
$f.Content = $img
$chrono = [System.Diagnostics.Stopwatch]::StartNew()
$script:k = 0
$script:pas = 0
$journal = "$Fichier.k"
$script:jw = $null
try {
    $fs = [System.IO.FileStream]::new($journal, [System.IO.FileMode]::Create, [System.IO.FileAccess]::Write, [System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete)
    $script:jw = [System.IO.StreamWriter]::new($fs, [System.Text.Encoding]::ASCII)
    $script:jw.AutoFlush = $true
}
catch { Signaler 'journal' $_.Exception }
$qpc = [double][System.Diagnostics.Stopwatch]::Frequency / 1000.0
function Noter([int]$K) {
    try { if ($script:jw) { $script:jw.WriteLine("k $K $([math]::Round([System.Diagnostics.Stopwatch]::GetTimestamp() / $qpc, 2).ToString([System.Globalization.CultureInfo]::InvariantCulture))") } } catch { }
}
# Un pas : k peint (le haut seulement pour le témoin simple), puis noté.
function Pas([int]$K) {
    $n = $T::PeindrePas($tampon, $K, $genreN)
    $wb.WritePixels([System.Windows.Int32Rect]::new(0, 0, $L, $n), $tampon, $L * 4, 0)
    $script:k = $K
    $script:pas++
    Noter $K
}
$minuterie = [System.Windows.Threading.DispatcherTimer]::new([System.Windows.Threading.DispatcherPriority]::Render)
$minuterie.Interval = [TimeSpan]::FromMilliseconds(5)
$minuterie.Add_Tick({
        try {
            $k = [int][math]::Floor($chrono.Elapsed.TotalMilliseconds / $PeriodeMs) % 65536
            if ($k -ne $script:k) { Pas $k }
        }
        catch { Signaler 'minuterie' $_.Exception }
    })
$rendu = [System.EventHandler]{
    try { Pas (($script:k + 1) % 65536) } catch { Signaler 'Rendering' $_.Exception }
}
$f.Add_ContentRendered({
        try {
            $hwnd = ([System.Windows.Interop.WindowInteropHelper]::new($f)).Handle
            $p = $img.PointToScreen([System.Windows.Point]::new(0, 0))
            Ecrire @("hwnd $([long]$hwnd)", "pid $PID", "zone $([int]$p.X),$([int]$p.Y),$L,$H", "echelle $k0", "journal $journal", "rapide $([int][bool]$Rapide)")
            Noter 0
            $chrono.Restart()
            if ($genreN -eq 1) { return }
            if ($Rapide) { [System.Windows.Media.CompositionTarget]::add_Rendering($rendu) } else { $minuterie.Start() }
        }
        catch { Signaler 'ContentRendered' $_.Exception }
    })
$app = [System.Windows.Application]::new()
$app.Add_DispatcherUnhandledException({ param($s, $e) Signaler 'WPF' $e.Exception; $e.Handled = $true })
[void]$app.Run($f)
try { if ($script:jw) { $script:jw.Dispose() } } catch { }
exit 0
