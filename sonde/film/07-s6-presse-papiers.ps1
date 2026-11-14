# S6 (contrat 0.7, §1.6) : un fichier dans le presse-papiers.
# - L'exemple presse pose la liste [film.mp4] par arboard 3.7 (set().file_list), la relit par
#   arboard, et GARDE le presse-papiers (l'objet Clipboard) jusqu'à « quitter », comme l'app.
# - Pendant ce temps : Get-Clipboard -Format FileDropList (Windows PowerShell 5.1) ;
#   [Windows.Forms.Clipboard]::GetFileDropList() (pwsh -STA) ; coller dans un dossier par
#   Shell.Application (verbe « paste » du dossier) et par Ctrl+V dans une fenêtre de
#   l'Explorateur ; une autre app écrit puis relit du texte (le presse-papiers gardé ne la
#   gêne pas).
# - Après « quitter » : la liste reste-t-elle dans le presse-papiers.
#   pwsh -NoProfile -File sonde/film/07-s6-presse-papiers.ps1

. "$PSScriptRoot/commun.ps1"

# Lance un petit script dans un autre processus (powershell.exe ou pwsh, -STA) ; rend
# { Texte, Code, Fini }.
function Script-A-Part([ValidateSet('powershell', 'pwsh')][string]$Moteur, [string]$Texte, [double]$DelaiS = 30) {
    $f = Fichier-Temp "a-part-$([guid]::NewGuid().ToString('N').Substring(0, 6)).ps1"
    Set-Content -LiteralPath $f -Value $Texte -Encoding utf8BOM
    $exe = if ($Moteur -eq 'powershell') { Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe' } else { $script:Pwsh }
    $out = "$f.out"
    $p = Start-Process -FilePath $exe -ArgumentList @('-NoProfile', '-STA', '-ExecutionPolicy', 'Bypass', '-File', "`"$f`"") -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError "$out.err"
    $null = $p.Handle
    $fini = $p.WaitForExit([int]($DelaiS * 1000))
    if (-not $fini) { try { $p.Kill($true) } catch { } }
    $t = if (Test-Path -LiteralPath $out) { "$(Get-Content -LiteralPath $out -Raw -Encoding utf8)".Trim() } else { '' }
    $e = if (Test-Path -LiteralPath "$out.err") { "$(Get-Content -LiteralPath "$out.err" -Raw)".Trim() } else { '' }
    if ($e) { $t = "$t$(if ($t) { ' ; ' })erreur : $(($e -split "`n")[0])" }
    return [pscustomobject]@{ Texte = $t; Code = $(if ($fini) { Code-Sortie $p.ExitCode } else { "pas fini en $DelaiS s" }); Fini = $fini }
}

function Lancer-Presse([string]$Fichier) {
    $etat = Fichier-Temp "presse-$([guid]::NewGuid().ToString('N').Substring(0, 6)).txt"
    $signal = "$etat.signal"
    $p = Lancer-Exemple 'presse' @('--fichier', "`"$Fichier`"", '--etat', "`"$etat`"", '--signal', "`"$signal`"")
    if ($null -eq $p) { throw 'exemple presse absent (étape de construction échouée ?)' }
    $ok = Attendre -DelaiS 20 -Condition { $e = Lire-Etat $etat; $e.ContainsKey('pose') -or $e.ContainsKey('erreur') -or $p.HasExited }
    return [pscustomobject]@{ P = $p; Etat = $etat; Signal = $signal; Pret = $ok }
}
function Quitter-Presse($X) {
    if ($null -eq $X) { return $null }
    try { Ecrire-Commande $X.Signal 'quitter' } catch { }
    $fini = $X.P.WaitForExit(10000)
    if (-not $fini) { try { $X.P.Kill($true) } catch { } }
    return $fini
}

$lireListe = @'
Add-Type -AssemblyName System.Windows.Forms
$l = [System.Windows.Forms.Clipboard]::GetFileDropList()
"contient $([int][System.Windows.Forms.Clipboard]::ContainsFileDropList())"
"liste $(@($l) -join ';')"
"formats $(@([System.Windows.Forms.Clipboard]::GetDataObject().GetFormats($false)) -join ';')"
'@
$getClipboard = @'
$v = Get-Clipboard -Format FileDropList
"liste $(@($v | ForEach-Object { $_.FullName }) -join ';')"
'@

if (Demarrer-Sonde -Nom '07-s6-presse-papiers' -Titre 'S6 : un fichier dans le presse-papiers (arboard file_list)') {
    $x = $null
    try {
        $film = Fichier-Temp 'film-essai.mp4'
        $o = [byte[]]::new(300KB); [System.Random]::new(7).NextBytes($o); [System.IO.File]::WriteAllBytes($film, $o)
        $film = (Resolve-Path -LiteralPath $film).Path

        Section 'S6 : pose par arboard, presse-papiers gardé ouvert'
        $x = Lancer-Presse $film
        $e = Lire-Etat $x.Etat
        if (-not $x.Pret -or -not $e.ContainsKey('pose')) { NonMesurable 'set().file_list' "$(if ($e.ContainsKey('erreur')) { $e.erreur } elseif ($x.P.HasExited) { "sortie, code $(Code-Sortie $x.P.ExitCode)" } else { 'rien écrit en 20 s' })" }
        else {
            Resultat 'Clipboard::new puis set().file_list([film.mp4])' "pose $($e['pose']) en $(Ms $e['pose_ms'])" 'pose 1' (OuiNon ("$($e['pose'])" -eq '1'))
            Resultat 'Relu par arboard (get().file_list)' "$($e['relu_arboard'])" "le chemin $film" (OuiNon ("$($e['relu_arboard'])" -eq $film))

            $r = Script-A-Part 'powershell' $getClipboard
            $lu = "$($r.Texte)" -replace '^liste ', ''
            Resultat 'Get-Clipboard -Format FileDropList (Windows PowerShell 5.1), presse-papiers gardé par l''exemple' "$($r.Texte) (code $($r.Code))" 'rend le chemin' (OuiNon ($lu -eq $film))
            $r = Script-A-Part 'pwsh' $lireListe
            Resultat '[Windows.Forms.Clipboard]::GetFileDropList() (pwsh -STA)' "$($r.Texte -replace "`r?`n", ' ; ') (code $($r.Code))" 'contient 1, le chemin' (OuiNon ("$($r.Texte)" -match 'contient 1' -and "$($r.Texte)".Contains($film)))

            Section 'S6 : coller dans un dossier de l''Explorateur, sans personne'
            $d1 = Fichier-Temp 'colle-shell'; New-Item -ItemType Directory -Force -Path $d1 | Out-Null
            $d1 = (Resolve-Path -LiteralPath $d1).Path
            $coller = @"
`$sh = New-Object -ComObject Shell.Application
`$d = `$sh.Namespace('$($d1 -replace "'", "''")')
if (`$null -eq `$d) { 'namespace introuvable'; exit 1 }
`$d.Self.InvokeVerb('paste')
'verbe 1'
"@
            $chrono = [System.Diagnostics.Stopwatch]::StartNew()
            $r = Script-A-Part 'powershell' $coller
            $cible1 = Join-Path $d1 'film-essai.mp4'
            $ok = Attendre -DelaiS 15 -PasMs 100 -Condition { (Test-Path -LiteralPath $cible1) -and (Get-Item -LiteralPath $cible1).Length -eq $o.Length }
            Resultat 'Shell.Application : Namespace(dossier).Self.InvokeVerb(''paste'')' "$($r.Texte) (code $($r.Code)) ; fichier créé : $(OuiNon $ok)$(if ($ok) { " en $([int]$chrono.Elapsed.TotalMilliseconds) ms, $((Get-Item -LiteralPath $cible1).Length) octets" })" 'le fichier apparaît, même taille' (OuiNon $ok)

            $d2 = Fichier-Temp 'colle-ctrlv'; New-Item -ItemType Directory -Force -Path $d2 | Out-Null
            $d2 = (Resolve-Path -LiteralPath $d2).Path
            # L'Explorateur peut rendre le chemin long là où la sonde a le nom court (8.3) :
            # la fenêtre est reconnue à la fin de son chemin.
            $script:hwExp = [IntPtr]::Zero
            $exp = Start-Process -FilePath (Join-Path $env:SystemRoot 'explorer.exe') -ArgumentList "`"$d2`"" -PassThru
            $hw = [IntPtr]::Zero
            $vu = Attendre -DelaiS 15 -PasMs 200 -Condition {
                $sh = New-Object -ComObject Shell.Application
                foreach ($w in @($sh.Windows())) {
                    try { if ("$($w.Document.Folder.Self.Path)" -like "*\$(Split-Path -Leaf $script:Sonde.Temp)\colle-ctrlv") { $script:hwExp = [IntPtr]::new([long]$w.HWND); return $true } } catch { }
                }
                return $false
            }
            if (-not $vu) { NonMesurable 'Ctrl+V dans une fenêtre de l''Explorateur' "aucune fenêtre de l'Explorateur sur le dossier en 15 s (explorer.exe lancé, pid $($exp.Id) ; l'Explorateur tourne-t-il comme interface sur cette machine ?)" }
            else {
                $hw = $script:hwExp
                Start-Sleep -Milliseconds 800
                $pp = $script:Natif::PremierPlan($hw)
                $n = $script:Natif::Raccourci(0x56)
                $cible2 = Join-Path $d2 'film-essai.mp4'
                $ok = Attendre -DelaiS 15 -PasMs 100 -Condition { (Test-Path -LiteralPath $cible2) -and (Get-Item -LiteralPath $cible2).Length -eq $o.Length }
                Resultat 'Ctrl+V dans la fenêtre de l''Explorateur ouverte sur le dossier' "fenêtre au premier plan $(OuiNon $pp) ; SendInput $n sur 4 ; fichier créé : $(OuiNon $ok)" 'le fichier apparaît' (OuiNon $ok)
                try {
                    $sh = New-Object -ComObject Shell.Application
                    foreach ($w in @($sh.Windows())) { try { if ([long]$w.HWND -eq [long]$hw) { $w.Quit() } } catch { } }
                }
                catch { }
            }

            Section 'S6 : le presse-papiers gardé par l''app ne gêne pas les autres'
            $ecrire = @'
try { Set-Clipboard -Value 'texte de la sonde S6'; "pose 1" } catch { "pose 0 $($_.Exception.Message)" }
"relu $(Get-Clipboard -Raw)"
'@
            $r = Script-A-Part 'powershell' $ecrire
            Resultat 'Une autre app écrit du texte puis le relit (Set-Clipboard, Get-Clipboard) pendant que l''exemple garde le presse-papiers' "$($r.Texte -replace "`r?`n", ' ; ') (code $($r.Code))" 'pose 1, relu le texte' (OuiNon ("$($r.Texte)" -match 'pose 1' -and "$($r.Texte)" -match 'texte de la sonde S6'))
            $fini = Quitter-Presse $x; $x = $null
            Resultat 'L''exemple rend le presse-papiers et sort sur « quitter »' (OuiNon $fini) 'OUI' (OuiNon $fini)

            Section 'S6 : la liste reste-t-elle après la fin de l''app'
            $x = Lancer-Presse $film
            $fini = Quitter-Presse $x; $x = $null
            $r = Script-A-Part 'powershell' $getClipboard
            $lu = "$($r.Texte)" -replace '^liste ', ''
            Resultat 'Get-Clipboard -Format FileDropList après la sortie de l''exemple' "$($r.Texte) (code $($r.Code))" 'à lire (attendu : le chemin, la liste appartient au presse-papiers)' (OuiNon ($lu -eq $film))
            Note 'Les formats posés par arboard sont lus plus haut (GetFormats) : sans « Preferred DropEffect », l''Explorateur colle une copie.'
        }
    }
    catch { Erreur-Sonde $_ }
    finally { $null = Quitter-Presse $x }
}
Terminer-Sonde
