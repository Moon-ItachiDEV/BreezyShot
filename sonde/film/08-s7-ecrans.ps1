# S7 (contrat 0.7, §1.7) et S1 point 6 (§1.1) : échelles, écrans, accès perdu, disque plein.
# 1. L'écran de la machine (taille, modes, DPI, écrans vus) ; l'échelle se change-t-elle sans
#    personne (DisplayConfigSetDeviceInfo, pas relatif à l'échelle recommandée), remise après.
# 2. et S1 point 6 : images --methode perte sur le témoin -Rapide : le mode passe à 800 × 600
#    (ChangeDisplaySettingsEx) à 2 s et revient à 6 s ; la duplication rend-elle
#    DXGI_ERROR_ACCESS_LOST, recréée toutes les 500 ms reprend-elle et en combien de temps ;
#    ce que rend BitBlt pendant ce temps ; ce que rend la liste des écrans (comme
#    gel::ecrans) avant, pendant et après.
# 3. Un disque virtuel de 64 Mo (diskpart create vdisk, attach, partition, format, monté sur
#    un dossier vide) : faisable, en combien de temps ; plein (réservé à 512 Ko près), ce que
#    rend une écriture de fichier et ce que rend l'écrivain MP4 (WriteSample ou Finalize, et
#    le HRESULT) ; démonté et effacé à la fin, même après une erreur.
#   pwsh -NoProfile -File sonde/film/08-s7-ecrans.ps1   (administrateur pour le point 3)

. "$PSScriptRoot/commun.ps1"

function Diskpart([string[]]$Lignes, [double]$DelaiS = 120) {
    $f = Fichier-Temp "diskpart-$([guid]::NewGuid().ToString('N').Substring(0, 6)).txt"
    [System.IO.File]::WriteAllLines($f, $Lignes, [System.Text.Encoding]::ASCII)
    $out = "$f.out"
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath (Join-Path $env:SystemRoot 'System32/diskpart.exe') -ArgumentList @('/s', "`"$f`"") -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError "$out.err"
    $null = $p.Handle
    $fini = $p.WaitForExit([int]($DelaiS * 1000))
    if (-not $fini) { try { $p.Kill($true) } catch { } }
    $t = if (Test-Path -LiteralPath $out) { "$(Get-Content -LiteralPath $out -Raw)" } else { '' }
    $utile = @($t -split "`r?`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ -and $_ -notmatch '^(Microsoft DiskPart|Copyright|On computer|DiskPart)' }) -join ' / '
    return [pscustomobject]@{ Code = $(if ($fini) { $p.ExitCode } else { -1 }); Fini = $fini; Ms = [int]$chrono.Elapsed.TotalMilliseconds; Texte = $utile }
}

if (Demarrer-Sonde -Nom '08-s7-ecrans' -Titre 'S7 et S1 point 6 : écrans, échelle, accès perdu, disque plein') {
    $t = $null
    try {
        Section 'S7 (1) : l''écran de la machine'
        $script:ModeDepart = $script:N::ModeCourant()
        $script:Sonde.Restaurer.Add({ if ($script:N::ModeCourant() -ne $script:ModeDepart) { [void]$script:N::RemettreMode() } })
        Resultat 'Mode courant (EnumDisplaySettings)' $script:ModeDepart '1024x768 attendu' 'à lire'
        $modes = @($script:N::Modes())
        Resultat 'Modes offerts' "$($modes.Count) : $($modes -join ', ')" 'à lire (800x600 en fait-il partie ?)' (OuiNon ($modes -contains '800x600'))
        $ecrans = @([System.Windows.Forms.Screen]::AllScreens)
        Resultat 'Écrans (Screen.AllScreens)' "$($ecrans.Count) : $(($ecrans | ForEach-Object { "$($_.DeviceName) $($_.Bounds.X),$($_.Bounds.Y),$($_.Bounds.Width),$($_.Bounds.Height)$(if ($_.Primary) { ' principal' })" }) -join ' ; ')" '1 attendu' 'à lire'
        $r = Jouer-Exemple 'images' @('--methode', 'ecrans') 30
        $l = @($r.Keys | Where-Object { $_ -like 'ecran_*' } | Sort-Object | ForEach-Object { $r[$_] })
        Resultat 'Écrans comme gel::ecrans (EnumDisplayMonitors, GetMonitorInfoW, GetDpiForMonitor)' $(if ($l.Count) { $l -join ' ; ' } else { "NON MESURÉ : $($r['erreur'])" }) 'à lire'
        $dpi = $script:N::DpiEcran(1, 1)
        Resultat 'DPI de l''écran principal (GetDpiForMonitor)' "$dpi ($([math]::Round($dpi / 96 * 100)) %)" '96 (100 %) attendu' 'à lire'
        $er = @($script:N::EchelleRelative())
        Resultat 'Échelle relative (DisplayConfigGetDeviceInfo, pas relatifs à la recommandée)' "code $($er[0]) ; min $($er[1]) ; actuelle $($er[2]) ; max $($er[3])" 'à lire' 'à lire'
        if ($er[0] -eq 0 -and $er[3] -gt $er[2]) {
            $script:EchelleDepart = $er[2]
            $script:Sonde.Restaurer.Add({ $x = @($script:N::EchelleRelative()); if ($x[0] -eq 0 -and $x[2] -ne $script:EchelleDepart) { [void]$script:N::FixerEchelleRelative($script:EchelleDepart) } })
            $c = $script:N::FixerEchelleRelative($er[2] + 1)
            Start-Sleep -Seconds 2
            $d2 = $script:N::DpiEcran(1, 1)
            $c2 = $script:N::FixerEchelleRelative($er[2])
            Start-Sleep -Seconds 2
            $d3 = $script:N::DpiEcran(1, 1)
            Resultat 'Échelle changée d''un pas sans personne (DisplayConfigSetDeviceInfo), puis remise' "code $c : DPI $dpi → $d2 ; remise code $c2 : DPI $d3" 'à lire : changée puis revenue' (OuiNon ($c -eq 0 -and $d2 -ne $dpi -and $d3 -eq $dpi))
        }
        else { Resultat 'Échelle changée sans personne' 'non faisable : la machine n''offre aucun pas au-dessus de l''échelle actuelle (ou la lecture a échoué, code ci-dessus)' 'supposition du contrat : non faisable' 'NON' }
        Note 'Ajouter un écran sans personne n''est pas faisable ici (une seule sortie d''affichage virtuelle) : restent les tests purs (§15.1) et la liste de l''utilisateur (maison : un écran 2K ; travail : trois écrans 1080p).'

        Section 'S1 (6) et S7 (2) : mode 800 × 600 puis retour, sur le témoin -Rapide'
        $ok = $script:N::ChangerMode(800, 600, $true)
        Resultat 'ChangeDisplaySettingsEx(800 × 600, CDS_TEST)' "$ok (0 : DISP_CHANGE_SUCCESSFUL)" 'à lire' (OuiNon ($ok -eq 0))
        $t = Ouvrir-Temoin -Genre simple -X 100 -Y 100 -Rapide
        $z = $t.Zone
        $r = Jouer-Exemple 'images' @('--methode', 'perte', '--zone', (Rect-Texte $z), '--duree', '12', '--temoin', "$($z[0]),$($z[1])") 90
        Fermer-Temoin $t; $t = $null
        if ($r['ok'] -ne '1') { NonMesurable 'accès perdu provoqué' "$($r['erreur'])" }
        else {
            Resultat 'Changement de mode' "code $($r['mode_change_code']) à $($r['change_a_ms']) ms ; mode $($r['mode_avant']) → $($r['mode_pendant']) → $($r['mode_apres']) (retour code $($r['mode_remis_code']) à $($r['remis_a_ms']) ms)" 'fait, puis revenu au mode de départ' (OuiNon ("$($r['mode_change_code'])" -like '0*' -and "$($r['mode_apres'])" -eq "$($r['mode_avant'])"))
            Resultat 'DXGI_ERROR_ACCESS_LOST rendu' "pertes $($r['pertes_acces']) ; autres erreurs $($r['dxgi_autres_erreurs']) ; première vue à $($r['perte_vue_a_ms']) ms" 'à lire (attendu : OUI)' (OuiNon ((Nombre $r['pertes_acces']) -ge 1))
            Resultat 'Duplication recréée (au plus toutes les 500 ms)' "reprises en $($r['reprises_ms'] -replace ';', ' ; ') ms ; images neuves pendant le mode 800 × 600 $($r['dxgi_neuves_pendant']) ; après le retour $($r['dxgi_neuves_apres_retour']) (la première à $($r['dxgi_premiere_neuve_apres_retour_ms']) ms)" 'reprend : des images neuves après le retour' (OuiNon ((Nombre $r['dxgi_neuves_apres_retour']) -gt 0))
            if ($r.ContainsKey('dxgi_journal')) { Resultat 'Journal de la duplication' "$($r['dxgi_journal'])" 'à lire' }
            Resultat 'BitBlt pendant ce temps (du changement à 1 s après le retour)' "$($r['bitblt_pendant'])" 'à lire' 'à lire'
            $av = @($r.Keys | Where-Object { $_ -like 'ecrans_avant_*' } | Sort-Object | ForEach-Object { $r[$_] }) -join ' ; '
            $pe = @($r.Keys | Where-Object { $_ -like 'ecrans_pendant_*' } | Sort-Object | ForEach-Object { $r[$_] }) -join ' ; '
            $ap = @($r.Keys | Where-Object { $_ -like 'ecrans_apres_*' } | Sort-Object | ForEach-Object { $r[$_] }) -join ' ; '
            Resultat 'S7 (2) : la liste des écrans (gel::ecrans) change pendant le mode 800 × 600' "avant : $av ; pendant : $pe" 'OUI : l''essai 15 peut jouer la fin « ecrans »' (OuiNon ($pe -and $pe -ne $av))
            Resultat 'S7 (2) : elle revient proprement' "après : $ap" 'égale à celle d''avant' (OuiNon ($ap -eq $av))
        }
        Start-Sleep -Seconds 1
        $m = $script:N::ModeCourant()
        if ($m -ne $script:ModeDepart) { $c = $script:N::RemettreMode(); Start-Sleep -Seconds 1; $m2 = $script:N::ModeCourant() } else { $c = 'inutile'; $m2 = $m }
        Resultat 'Mode à la fin, vu par la sonde' "$m (remise : $c → $m2)" "le mode de départ ($script:ModeDepart)" (OuiNon ($m2 -eq $script:ModeDepart))

        Section 'S7 (3) : disque virtuel de 64 Mo monté sur un dossier, puis plein'
        $admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        Resultat 'Administrateur' (OuiNon $admin) 'OUI (diskpart le demande)' (OuiNon $admin)
        $script:Vhd = [System.IO.Path]::GetFullPath((Fichier-Temp 'films-64.vhdx'))
        $script:Montage = [System.IO.Path]::GetFullPath((Fichier-Temp 'Films'))
        New-Item -ItemType Directory -Force -Path $script:Montage | Out-Null
        $script:Sonde.Restaurer.Add({
                if (Test-Path -LiteralPath $script:Vhd) {
                    $x = Diskpart @("select vdisk file=`"$script:Vhd`"", 'detach vdisk noerr') 60
                    Write-Host "disque virtuel démonté : code $($x.Code)"
                    Remove-Item -LiteralPath $script:Vhd -Force -ErrorAction SilentlyContinue
                }
            })
        $x = Diskpart @(
            "create vdisk file=`"$script:Vhd`" maximum=64 type=fixed",
            "select vdisk file=`"$script:Vhd`"",
            'attach vdisk',
            'convert mbr noerr',
            'create partition primary',
            'format fs=ntfs quick label=FILMS64',
            "assign mount=`"$script:Montage`""
        ) 180
        $libre = @($script:N::Disque("$script:Montage\"))
        $ok = $x.Code -eq 0 -and $libre[0] -gt 0 -and $libre[1] -lt 80MB
        Resultat 'diskpart : create vdisk 64 Mo, attach, partition, format NTFS, assign mount=<dossier>' "code $($x.Code) en $($x.Ms) ms$(if (-not $x.Fini) { ' (pas fini en 180 s)' }) ; volume vu au dossier : libre $(Mo $libre[0]), total $(Mo $libre[1])" 'faisable : monté, total vers 64 Mo' (OuiNon $ok)
        if (-not $ok) { Note "Sortie de diskpart : $($x.Texte)" }
        else {
            Resultat 'Temps total pour poser le disque' "$($x.Ms) ms" 'à lire' 'à lire'
            # Écriture d'un fichier plus grand que la place : l'exception et son HRESULT.
            $f = Join-Path $script:Montage 'trop-grand.bin'
            try {
                $fs = [System.IO.File]::Open($f, 'Create', 'Write', 'None')
                try { $fs.SetLength($libre[0] + 1MB); $e = 'aucune erreur (?)' } catch { $ex = $_.Exception; if ($ex.InnerException) { $ex = $ex.InnerException }; $e = "$($ex.GetType().Name) HRESULT $('0x{0:X8}' -f $ex.HResult) : $($ex.Message)" } finally { $fs.Dispose() }
            }
            catch { $e = "ouverture : $($_.Exception.Message)" }
            Remove-Item -LiteralPath $f -Force -ErrorAction SilentlyContinue
            Resultat 'Fichier plus grand que la place libre (FileStream.SetLength)' $e 'refus net (0x80070070 ERROR_DISK_FULL attendu)' (OuiNon ($e -match '80070070|disk|espace|space'))
            # Disque rempli à 512 Ko près, puis l'écrivain MP4 (60 s à 30 i/s).
            $libre = @($script:N::Disque("$script:Montage\"))
            $rempli = Join-Path $script:Montage 'remplissage.bin'
            try {
                $fs = [System.IO.File]::Open($rempli, 'Create', 'Write', 'None')
                try { $fs.SetLength([math]::Max(0, $libre[0] - 512KB)) } finally { $fs.Dispose() }
                $l2 = @($script:N::Disque("$script:Montage\"))
                Resultat 'Disque rempli' "libre $(Mo $l2[0])" 'environ 0,5 Mo' 'à lire'
                $film = Join-Path $script:Montage 'film.mp4'
                $r = Jouer-Exemple 'mf' @('--etape', 'finaliser', '--secondes', '60', '--sortie', "`"$film`"") 240
                $etat = if ($r['ok'] -eq '1') { "fini sans erreur (?) : $($r['taille_octets']) octets, Finalize $(Ms $r['finalize_ms'])" } else { "$($r['erreur'])" }
                Resultat 'Écrivain MP4 (IMFSinkWriter) sur le disque plein : où il échoue, avec quel HRESULT' $etat 'un échec net, avec son HRESULT (0x80070070 attendu)' (OuiNon ($r['ok'] -ne '1' -and "$($r['erreur'])" -match '0x'))
                Resultat 'Fichier laissé par l''écrivain' "$(if (Test-Path -LiteralPath $film) { "$((Get-Item -LiteralPath $film).Length) octets" } else { 'aucun' })" 'à lire' 'à lire'
                Note 'Si oui, l''essai 15 joue la fin « disque » en posant ce disque sous <local>/Films (contrat §1.7, point 3).'
            }
            catch { NonMesurable 'disque rempli' "$($_.Exception.Message)" }
        }
    }
    catch { Erreur-Sonde $_ }
    finally { Fermer-Temoin $t }
}
Terminer-Sonde
