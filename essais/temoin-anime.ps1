# Aide de l'essai 15 (Windows seulement) : le temoin anime, cible des films. PowerShell 7,
# texte ASCII seulement. Repris du temoin de la sonde du film (contrat 0.7, partie I), sans
# la fenetre "cible" de S5 ; le dessin est celui de essais/film-natif.cs (classe Temoin).
#
# Fenetre WPF sans bord, toujours au-dessus, posee en ($X, $Y) en pixels reels, zone de
# 600 x 450 pixels reels :
#   simple  le compteur k (qui avance toutes les -PeriodeMs, 100 ms par defaut) ecrit en 16
#           carres noirs ou blancs, une bande de la couleur Couleur(k), quatre pastilles de
#           couleurs fixes ; -Rapide : k avance a chaque image composee (CompositionTarget
#           .Rendering), pour mesurer la fraicheur des images du film ;
#   fixe    la meme, rien ne bouge (k reste 0) : un GIF qui ne change pas ;
#   plein   la meme, tout le fond change a chaque pas (plus de 256 couleurs) : un film qui
#           pese lourd (la fin "disque").
# Ecrit $Fichier (temporaire puis renommage) : hwnd <h> / pid <p> / zone x,y,l,h /
# echelle <e> / journal <f> / rapide <0|1>. Le journal ($Fichier.k) recoit une ligne
# "k <n> <ms>" par pas, <ms> lu sur l'horloge de performance (QueryPerformanceCounter, la
# meme que celle de l'exemple lire_film et de l'essai).
#   pwsh -NoProfile -File essais/temoin-anime.ps1 -Genre simple -X 40 -Y 40 -Fichier t.txt [-Rapide]

param(
    [ValidateSet('simple', 'fixe', 'plein')][string]$Genre = 'simple',
    [int]$X = 40, [int]$Y = 40,
    [int]$PeriodeMs = 100,
    [switch]$Rapide,
    [Parameter(Mandatory)][string]$Fichier
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'film-natif.cs') -Raw)
# Noms longs : PowerShell confond $N et $n, $T et $t.
$script:Natif = [BreezyShotFilm.Natif]
$script:Temoin = [BreezyShotFilm.Temoin]
try { [void]$script:Natif::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }

# Jamais d'exception visible : une boite d'erreur se mettrait devant les images.
function Signaler([string]$Ou, $Ex) {
    try { [Console]::Out.WriteLine("temoin $Genre (pid $PID) : exception dans $Ou : $($Ex.GetType().FullName) : $($Ex.Message)") } catch { }
}
function Ecrire-Etat([string[]]$Lignes) {
    try {
        $tmp = "$Fichier.tmp"
        [System.IO.File]::WriteAllLines($tmp, $Lignes, [System.Text.Encoding]::ASCII)
        for ($i = 0; $i -lt 10; $i++) {
            try { [System.IO.File]::Move($tmp, $Fichier, $true); return }
            catch { Start-Sleep -Milliseconds 15 }
        }
    }
    catch { Signaler 'ecriture de l etat' $_.Exception }
}

Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase
$script:GenreN = switch ($Genre) { 'simple' { 0 } 'fixe' { 1 } 'plein' { 2 } }
$script:Larg = $script:Temoin::L
$script:Haut = $script:Temoin::H
$fenetre = [System.Windows.Window]::new()
$fenetre.Title = "Temoin film BreezyShot $Genre"
$fenetre.WindowStyle = [System.Windows.WindowStyle]::None
$fenetre.ResizeMode = [System.Windows.ResizeMode]::NoResize
$fenetre.Topmost = $true
$fenetre.ShowInTaskbar = $false
$fenetre.UseLayoutRounding = $true
$fenetre.WindowStartupLocation = [System.Windows.WindowStartupLocation]::Manual
# WPF place en pixels logiques : ramenes aux pixels reels demandes (ecran a 125 % compris).
$script:Echelle = [double]$script:Natif::DpiEcran($X + 10, $Y + 10) / 96
if ($script:Echelle -le 0) { $script:Echelle = 1.0 }
$fenetre.Left = $X / $script:Echelle
$fenetre.Top = $Y / $script:Echelle
$fenetre.Width = $script:Larg / $script:Echelle
$fenetre.Height = $script:Haut / $script:Echelle
$script:Image = [System.Windows.Media.Imaging.WriteableBitmap]::new($script:Larg, $script:Haut, 96 * $script:Echelle, 96 * $script:Echelle, [System.Windows.Media.PixelFormats]::Bgra32, $null)
$script:Tampon = [byte[]]::new($script:Larg * $script:Haut * 4)
$script:Temoin::Peindre($script:Tampon, 0, $script:GenreN)
$script:Image.WritePixels([System.Windows.Int32Rect]::new(0, 0, $script:Larg, $script:Haut), $script:Tampon, $script:Larg * 4, 0)
$vue = [System.Windows.Controls.Image]::new()
$vue.Source = $script:Image
$vue.Stretch = [System.Windows.Media.Stretch]::None
[System.Windows.Media.RenderOptions]::SetBitmapScalingMode($vue, [System.Windows.Media.BitmapScalingMode]::NearestNeighbor)
$fenetre.Content = $vue
$script:Chrono = [System.Diagnostics.Stopwatch]::StartNew()
$script:KCourant = 0
$script:JournalK = "$Fichier.k"
$script:Ecrivain = $null
try {
    $flux = [System.IO.FileStream]::new($script:JournalK, [System.IO.FileMode]::Create, [System.IO.FileAccess]::Write, [System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete)
    $script:Ecrivain = [System.IO.StreamWriter]::new($flux, [System.Text.Encoding]::ASCII)
    $script:Ecrivain.AutoFlush = $true
}
catch { Signaler 'journal' $_.Exception }
$script:TicsParMs = [double][System.Diagnostics.Stopwatch]::Frequency / 1000.0
function Noter-K([int]$Valeur) {
    try {
        if ($script:Ecrivain) {
            $ms = [math]::Round([System.Diagnostics.Stopwatch]::GetTimestamp() / $script:TicsParMs, 2).ToString([System.Globalization.CultureInfo]::InvariantCulture)
            $script:Ecrivain.WriteLine("k $Valeur $ms")
        }
    }
    catch { }
}
# Un pas : k peint (le haut seulement pour le temoin simple), puis note.
function Faire-Pas([int]$Valeur) {
    $rangees = $script:Temoin::PeindrePas($script:Tampon, $Valeur, $script:GenreN)
    $script:Image.WritePixels([System.Windows.Int32Rect]::new(0, 0, $script:Larg, $rangees), $script:Tampon, $script:Larg * 4, 0)
    $script:KCourant = $Valeur
    Noter-K $Valeur
}
$script:Minuterie = [System.Windows.Threading.DispatcherTimer]::new([System.Windows.Threading.DispatcherPriority]::Render)
$script:Minuterie.Interval = [TimeSpan]::FromMilliseconds(5)
$script:Minuterie.Add_Tick({
        try {
            $voulu = [int][math]::Floor($script:Chrono.Elapsed.TotalMilliseconds / $PeriodeMs) % 65536
            if ($voulu -ne $script:KCourant) { Faire-Pas $voulu }
        }
        catch { Signaler 'minuterie' $_.Exception }
    })
$script:ARendu = [System.EventHandler]{
    try { Faire-Pas (($script:KCourant + 1) % 65536) } catch { Signaler 'Rendering' $_.Exception }
}
$fenetre.Add_ContentRendered({
        try {
            $hwnd = ([System.Windows.Interop.WindowInteropHelper]::new($fenetre)).Handle
            $coin = $vue.PointToScreen([System.Windows.Point]::new(0, 0))
            $ech = $script:Echelle.ToString([System.Globalization.CultureInfo]::InvariantCulture)
            Ecrire-Etat @("hwnd $([long]$hwnd)", "pid $PID", "zone $([int]$coin.X),$([int]$coin.Y),$($script:Larg),$($script:Haut)", "echelle $ech", "journal $($script:JournalK)", "rapide $([int][bool]$Rapide)")
            Noter-K 0
            $script:Chrono.Restart()
            if ($script:GenreN -eq 1) { return }
            if ($Rapide) { [System.Windows.Media.CompositionTarget]::add_Rendering($script:ARendu) } else { $script:Minuterie.Start() }
        }
        catch { Signaler 'ContentRendered' $_.Exception }
    })
$appli = [System.Windows.Application]::new()
$appli.Add_DispatcherUnhandledException({ param($s, $e) Signaler 'WPF' $e.Exception; $e.Handled = $true })
[void]$appli.Run($fenetre)
try { if ($script:Ecrivain) { $script:Ecrivain.Dispose() } } catch { }
exit 0
