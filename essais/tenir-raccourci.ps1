# Aide de l'essai 03 (Windows seulement) : tient un raccourci global avec RegisterHotKey,
# comme le ferait une autre application. Écrit « tenu » ou « refuse » dans $Fichier, puis,
# si tenu, attend $DureeS secondes (l'essai le ferme avant).
#   pwsh -NoProfile -File essais/tenir-raccourci.ps1 -Vk 0x2C -Mods 0 -Fichier x.txt

param(
    [Parameter(Mandatory)][int]$Vk,
    [int]$Mods = 0,
    [Parameter(Mandatory)][string]$Fichier,
    [int]$DureeS = 120
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -Namespace BreezyShotEssai -Name Raccourci -MemberDefinition @'
[DllImport("user32.dll", SetLastError = true)]
public static extern bool RegisterHotKey(IntPtr hWnd, int id, uint fsModifiers, uint vk);
[DllImport("user32.dll")]
public static extern bool UnregisterHotKey(IntPtr hWnd, int id);
'@

# MOD_NOREPEAT (0x4000) : comme global-hotkey.
$tenu = [BreezyShotEssai.Raccourci]::RegisterHotKey([IntPtr]::Zero, 1, [uint32]($Mods -bor 0x4000), [uint32]$Vk)
# Écrit dans un fichier temporaire puis renomme : l'essai ne lit jamais un fichier à moitié écrit.
$temporaire = "$Fichier.tmp"
Set-Content -LiteralPath $temporaire -Value $(if ($tenu) { 'tenu' } else { 'refuse' }) -Encoding utf8
Move-Item -LiteralPath $temporaire -Destination $Fichier -Force
if ($tenu) {
    try { Start-Sleep -Seconds $DureeS }
    finally { [void][BreezyShotEssai.Raccourci]::UnregisterHotKey([IntPtr]::Zero, 1) }
}
