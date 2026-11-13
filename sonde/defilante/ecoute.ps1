# Aide de S6 (Windows seulement) : un autre processus que celui qui envoie les crans.
# Pose le crochet WH_MOUSE_LL et l'entrée brute (ecoute.cs) dans un fil à boucle de
# messages, puis attend les commandes de la sonde dans $Commandes (lu puis supprimé) :
#   detail <fichier>  écrit tout ce qui a été noté (temporaire puis renommage)
#   quitter           retire le crochet et sort
# $Etat est réécrit toutes les 200 ms : pret, crochet, brut, crochet_erreur, brut_erreur,
# crans_crochet, crans_brut, frequence (QueryPerformanceFrequency).
#   pwsh -NoProfile -File sonde/defilante/ecoute.ps1 -Etat e.txt -Commandes c.txt

param([Parameter(Mandatory)][string]$Etat, [Parameter(Mandatory)][string]$Commandes, [int]$DureeMaxS = 600)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'ecoute.cs') -Raw)
$E = [BreezyShotSonde.Ecoute]

# Temporaire puis renommage ; si la sonde tient le fichier, quelques essais puis l'écriture
# est sautée (la suivante suffit) : jamais d'exception qui arrêterait l'écoute.
function Ecrire([string]$Fichier, [string[]]$Lignes) {
    try {
        $t = "$Fichier.tmp"
        [System.IO.File]::WriteAllLines($t, $Lignes, [System.Text.Encoding]::ASCII)
        for ($i = 0; $i -lt 10; $i++) {
            try { [System.IO.File]::Move($t, $Fichier, $true); return } catch { Start-Sleep -Milliseconds 15 }
        }
    }
    catch { [Console]::Out.WriteLine("écoute : état non écrit : $($_.Exception.Message)") }
}

$E::Demarrer()
$fin = [DateTime]::UtcNow.AddSeconds($DureeMaxS)
try {
    while ([DateTime]::UtcNow -lt $fin) {
        Ecrire $Etat @("pret $([int]$E::Pret)", "crochet $([int]$E::CrochetPose())", "crochet_erreur $($E::ErreurCrochet)",
            "brut_erreur $($E::ErreurBrut)", "crans_crochet $($E::NCrochet)", "crans_brut $($E::NBrut)",
            "frequence $([System.Diagnostics.Stopwatch]::Frequency)", "pid $PID")
        if (Test-Path -LiteralPath $Commandes) {
            $c = @(Get-Content -LiteralPath $Commandes)
            Remove-Item -LiteralPath $Commandes -Force
            $quitter = $false
            foreach ($l in $c) {
                if ($l -match '^detail (.+)$') {
                    $f = $Matches[1].Trim('"')
                    $t = "$f.tmp"
                    [System.IO.File]::WriteAllText($t, $E::Detail())
                    [System.IO.File]::Move($t, $f, $true)
                }
                elseif ($l -eq 'quitter') { $quitter = $true }
            }
            if ($quitter) { break }
        }
        Start-Sleep -Milliseconds 200
    }
}
finally { $E::Arreter() }
