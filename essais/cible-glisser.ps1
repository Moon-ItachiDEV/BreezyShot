# Aide de l'essai 07 (Windows seulement) : une fenêtre qui accepte qu'on y dépose des
# fichiers (AllowDrop, effet « copier »), comme une vraie application. Sans bordure,
# toujours au-dessus, posée en ($X, $Y), $Cote × $Cote pixels réels. Écrit son HWND dans
# $FichierHwnd, puis, à chaque dépôt, ajoute les chemins reçus (un par ligne) à
# $FichierDepots. Tourne jusqu'à être tuée. Sa console est cachée une fois la fenêtre
# montrée (pas de -WindowStyle Hidden : Windows l'appliquerait à la cible elle-même).
#   pwsh -NoProfile -File essais/cible-glisser.ps1 -X 700 -Y 20 -Cote 300 -FichierHwnd h.txt -FichierDepots d.txt

param(
    [Parameter(Mandatory)][int]$X,
    [Parameter(Mandatory)][int]$Y,
    [Parameter(Mandatory)][int]$Cote,
    [Parameter(Mandatory)][string]$FichierHwnd,
    [Parameter(Mandatory)][string]$FichierDepots
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -Namespace BreezyShotEssai -Name Cible -MemberDefinition @'
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SetProcessDpiAwarenessContext(IntPtr contexte);
[DllImport("kernel32.dll")]
public static extern IntPtr GetConsoleWindow();
[DllImport("user32.dll")]
public static extern bool ShowWindow(IntPtr h, int commande);
'@
try { [void][BreezyShotEssai.Cible]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

# Le glisser-déposer d'OLE exige un fil STA (celui de pwsh l'est par défaut sous Windows).
$appartement = [System.Threading.Thread]::CurrentThread.GetApartmentState()
if ($appartement -ne [System.Threading.ApartmentState]::STA) { throw "fil $appartement : il faut STA" }

$fenetre = [System.Windows.Forms.Form]::new()
$fenetre.AutoScaleMode = [System.Windows.Forms.AutoScaleMode]::None
$fenetre.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::None
$fenetre.ShowInTaskbar = $false
$fenetre.TopMost = $true
$fenetre.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
$fenetre.Location = [System.Drawing.Point]::new($X, $Y)
$fenetre.ClientSize = [System.Drawing.Size]::new($Cote, $Cote)
$fenetre.BackColor = [System.Drawing.Color]::FromArgb(255, 90, 90, 90)
$fenetre.AllowDrop = $true

$accepter = {
    param($s, $e)
    if ($e.Data.GetDataPresent([System.Windows.Forms.DataFormats]::FileDrop)) {
        $e.Effect = [System.Windows.Forms.DragDropEffects]::Copy
    }
    else { $e.Effect = [System.Windows.Forms.DragDropEffects]::None }
}
$fenetre.Add_DragEnter($accepter)
$fenetre.Add_DragOver($accepter)
$fenetre.Add_DragDrop({
        param($s, $e)
        $chemins = @($e.Data.GetData([System.Windows.Forms.DataFormats]::FileDrop))
        Add-Content -LiteralPath $FichierDepots -Value ($chemins -join "`n") -Encoding utf8
    })
$fenetre.Add_Shown({
        $fenetre.Location = [System.Drawing.Point]::new($X, $Y)
        $fenetre.ClientSize = [System.Drawing.Size]::new($Cote, $Cote)
        $temporaire = "$FichierHwnd.tmp"
        Set-Content -LiteralPath $temporaire -Value ([long]$fenetre.Handle) -Encoding utf8
        Move-Item -LiteralPath $temporaire -Destination $FichierHwnd -Force
        $console = [BreezyShotEssai.Cible]::GetConsoleWindow()
        if ($console -ne [IntPtr]::Zero) { [void][BreezyShotEssai.Cible]::ShowWindow($console, 0) }
    })
[System.Windows.Forms.Application]::Run($fenetre)
