# Aide de l'essai 13 (Windows seulement) : le temoin de texte, cible de Copier le texte.
# Fenetre WinForms sans bordure, fond blanc, toujours au-dessus, absente de la barre des
# taches, posee en ($X, $Y) en pixels reels. Trois lignes noires en Segoe UI 20 px :
#   BreezyShot reads this text / Version 0.5 of 12 November 2026 / 0123456789
# (3 lignes, 11 mots), une ligne de 12 px (resultat seulement note par l'essai), puis une
# partie blanche. Ecrit dans $Fichier (temporaire puis renommage) le rectangle de chaque
# ligne et de la partie blanche, en pixels reels de l'ecran, une ligne par rectangle :
#   ligne1 x,y lxh / ligne2 ... / ligne3 ... / petite ... / blanc ... / hwnd <h>
# puis tourne jusqu'a etre tuee. Console cachee une fois la fenetre montree (comme
# temoin.ps1 -CacherConsole).
# Texte ASCII seulement : la machine d'essai lit l'anglais (en-US), et ce fichier ne doit
# dependre d'aucun encodage.
#   pwsh -NoProfile -File essais/temoin-texte.ps1 -X 40 -Y 120 -Fichier t.txt

param(
    [Parameter(Mandatory)][int]$X,
    [Parameter(Mandatory)][int]$Y,
    [Parameter(Mandatory)][string]$Fichier
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -Namespace BreezyShotEssai -Name TemoinTexte -MemberDefinition @'
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SetProcessDpiAwarenessContext(IntPtr contexte);
[DllImport("kernel32.dll")]
public static extern IntPtr GetConsoleWindow();
[DllImport("user32.dll")]
public static extern bool ShowWindow(IntPtr h, int commande);
'@
# Pixels reels, avant toute fenetre (comme l'essai et l'app).
try { [void][BreezyShotEssai.TemoinTexte]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
try { [void][System.Windows.Forms.Application]::SetHighDpiMode([System.Windows.Forms.HighDpiMode]::PerMonitorV2) } catch { }

$LARGEUR = 520
$HAUTEUR = 360
$fenetre = [System.Windows.Forms.Form]::new()
$fenetre.AutoScaleMode = [System.Windows.Forms.AutoScaleMode]::None
$fenetre.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::None
$fenetre.ShowInTaskbar = $false
$fenetre.TopMost = $true
$fenetre.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
$fenetre.Location = [System.Drawing.Point]::new($X, $Y)
$fenetre.ClientSize = [System.Drawing.Size]::new($LARGEUR, $HAUTEUR)
$fenetre.BackColor = [System.Drawing.Color]::White

# Nom, texte, taille de police (px), haut de la ligne dans la fenetre.
$lignes = @(
    @('ligne1', 'BreezyShot reads this text', 20, 16),
    @('ligne2', 'Version 0.5 of 12 November 2026', 20, 56),
    @('ligne3', '0123456789', 20, 96),
    @('petite', 'Hello BreezyShot 2026, the quick brown fox: 42', 12, 150)
)
$etiquettes = [System.Collections.Generic.List[object]]::new()
foreach ($l in $lignes) {
    $e = [System.Windows.Forms.Label]::new()
    $e.AutoSize = $true
    $e.Font = [System.Drawing.Font]::new('Segoe UI', [single]$l[2], [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
    $e.ForeColor = [System.Drawing.Color]::Black
    $e.BackColor = [System.Drawing.Color]::White
    $e.Padding = [System.Windows.Forms.Padding]::new(0)
    $e.Margin = [System.Windows.Forms.Padding]::new(0)
    $e.Text = $l[1]
    $e.Location = [System.Drawing.Point]::new(16, $l[3])
    $fenetre.Controls.Add($e)
    $etiquettes.Add([pscustomobject]@{ Nom = $l[0]; Etiquette = $e })
}

function Texte-Rect([System.Drawing.Rectangle]$r) { return "$($r.X),$($r.Y) $($r.Width)x$($r.Height)" }

$fenetre.Add_Shown({
        # Place exacte redonnee une fois la fenetre creee (aucune mise a l'echelle).
        $fenetre.Location = [System.Drawing.Point]::new($X, $Y)
        $fenetre.ClientSize = [System.Drawing.Size]::new($LARGEUR, $HAUTEUR)
        $fenetre.Refresh()
        $sortie = [System.Collections.Generic.List[string]]::new()
        foreach ($et in $etiquettes) {
            $r = $et.Etiquette.RectangleToScreen($et.Etiquette.ClientRectangle)
            $sortie.Add("$($et.Nom) $(Texte-Rect $r)")
        }
        # La partie blanche : sous la petite ligne, rien n'y est dessine.
        $blanc = $fenetre.RectangleToScreen([System.Drawing.Rectangle]::new(40, 210, 300, 110))
        $sortie.Add("blanc $(Texte-Rect $blanc)")
        $sortie.Add("hwnd $([long]$fenetre.Handle)")
        $temporaire = "$Fichier.tmp"
        Set-Content -LiteralPath $temporaire -Value $sortie -Encoding ascii
        Move-Item -LiteralPath $temporaire -Destination $Fichier -Force
        $console = [BreezyShotEssai.TemoinTexte]::GetConsoleWindow()
        if ($console -ne [IntPtr]::Zero) { [void][BreezyShotEssai.TemoinTexte]::ShowWindow($console, 0) }
    })
[System.Windows.Forms.Application]::Run($fenetre)
