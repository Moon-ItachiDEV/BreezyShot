# Aide de l'essai 04 (Windows seulement) : la fenêtre témoin, cible des captures.
# Sans bordure, toujours au-dessus, absente de la barre des tâches, posée en ($X, $Y) en
# pixels réels, $Cote × $Cote pixels : quatre carrés de couleurs connues (haut gauche,
# haut droite, bas gauche, bas droite, dans l'ordre de -Couleurs, « r,v,b » chacune).
# Écrit son HWND dans $Fichier (temporaire puis renommage), puis tourne jusqu'à être tuée.
# Avec -Cadre (essai 06) : une vraie fenêtre à cadre (FormBorderStyle Sizable, titre,
# toujours au-dessus), posée en ($X, $Y) par son coin extérieur ; les carrés remplissent
# sa zone cliente de $Cote × $Cote pixels.
# Avec -CacherConsole (essai 07) : la console de l'aide est cachée une fois la fenêtre
# montrée (pas de -WindowStyle Hidden au lancement : Windows l'appliquerait au premier
# ShowWindow du processus, c'est-à-dire au témoin lui-même).
#   pwsh -NoProfile -File essais/temoin.ps1 -X 400 -Y 40 -Cote 200 -Couleurs '220,40,40;...' -Fichier t.txt

param(
    [Parameter(Mandatory)][int]$X,
    [Parameter(Mandatory)][int]$Y,
    [Parameter(Mandatory)][int]$Cote,
    [Parameter(Mandatory)][string]$Couleurs,
    [Parameter(Mandatory)][string]$Fichier,
    [switch]$Cadre,
    [switch]$CacherConsole
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -Namespace BreezyShotEssai -Name Temoin -MemberDefinition @'
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SetProcessDpiAwarenessContext(IntPtr contexte);
[DllImport("kernel32.dll")]
public static extern IntPtr GetConsoleWindow();
[DllImport("user32.dll")]
public static extern bool ShowWindow(IntPtr h, int commande);
'@
# Pixels réels, avant toute fenêtre (comme l'essai et l'app).
try { [void][BreezyShotEssai.Temoin]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
try { [void][System.Windows.Forms.Application]::SetHighDpiMode([System.Windows.Forms.HighDpiMode]::PerMonitorV2) } catch { }

$teintes = @($Couleurs -split ';' | ForEach-Object {
        $c = @($_ -split ',' | ForEach-Object { [int]$_ })
        [System.Drawing.Color]::FromArgb(255, $c[0], $c[1], $c[2])
    })
if ($teintes.Count -ne 4) { throw "il faut quatre couleurs, reçu : $Couleurs" }

$moitie = [int]($Cote / 2)
$fenetre = [System.Windows.Forms.Form]::new()
$fenetre.AutoScaleMode = [System.Windows.Forms.AutoScaleMode]::None
if ($Cadre) {
    $fenetre.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::Sizable
    $fenetre.Text = 'Témoin BreezyShot'
    $fenetre.MaximizeBox = $false
}
else { $fenetre.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::None }
$fenetre.ShowInTaskbar = $false
$fenetre.TopMost = $true
$fenetre.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
$fenetre.Location = [System.Drawing.Point]::new($X, $Y)
$fenetre.ClientSize = [System.Drawing.Size]::new($Cote, $Cote)
$fenetre.BackColor = [System.Drawing.Color]::Black
$places = @(@(0, 0), @($moitie, 0), @(0, $moitie), @($moitie, $moitie))
for ($i = 0; $i -lt 4; $i++) {
    $carre = [System.Windows.Forms.Panel]::new()
    $carre.Location = [System.Drawing.Point]::new($places[$i][0], $places[$i][1])
    $carre.Size = [System.Drawing.Size]::new($Cote - $moitie, $Cote - $moitie)
    if ($places[$i][0] -eq 0) { $carre.Width = $moitie }
    if ($places[$i][1] -eq 0) { $carre.Height = $moitie }
    $carre.BackColor = $teintes[$i]
    $fenetre.Controls.Add($carre)
}
$fenetre.Add_Shown({
        # Place exacte redonnée une fois la fenêtre créée (aucune mise à l'échelle).
        $fenetre.Location = [System.Drawing.Point]::new($X, $Y)
        $fenetre.ClientSize = [System.Drawing.Size]::new($Cote, $Cote)
        $temporaire = "$Fichier.tmp"
        Set-Content -LiteralPath $temporaire -Value ([long]$fenetre.Handle) -Encoding utf8
        Move-Item -LiteralPath $temporaire -Destination $Fichier -Force
        if ($CacherConsole) {
            $console = [BreezyShotEssai.Temoin]::GetConsoleWindow()
            if ($console -ne [IntPtr]::Zero) { [void][BreezyShotEssai.Temoin]::ShowWindow($console, 0) }
        }
    })
[System.Windows.Forms.Application]::Run($fenetre)
