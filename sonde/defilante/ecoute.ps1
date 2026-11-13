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

function Ecrire([string]$Fichier, [string[]]$Lignes) {
    $t = "$Fichier.tmp"
    Set-Content -LiteralPath $t -Value $Lignes -Encoding ascii
    Move-Item -LiteralPath $t -Destination $Fichier -Force
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
                    Move-Item -LiteralPath $t -Destination $f -Force
                }
                elseif ($l -eq 'quitter') { $quitter = $true }
            }
            if ($quitter) { break }
        }
        Start-Sleep -Milliseconds 200
    }
}
finally { $E::Arreter() }
