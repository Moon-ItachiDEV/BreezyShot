# Inventaire de la machine avant la sonde du film : Windows (édition, « N » ou non),
# fichiers et fonctionnalité Media Foundation, écran, carte graphique, service audio,
# navigateurs et WebView2, session, processeurs.
#   pwsh -NoProfile -File sonde/film/00-inventaire.ps1

. "$PSScriptRoot/commun.ps1"

function Version-Fichier([string]$Chemin) {
    try { return [System.Diagnostics.FileVersionInfo]::GetVersionInfo($Chemin).ProductVersion } catch { return '?' }
}

if (Demarrer-Sonde -Nom '00-inventaire' -Titre 'Inventaire de la machine') {
    try {
        Section 'Système'
        $os = Get-CimInstance Win32_OperatingSystem
        $cv = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -ErrorAction SilentlyContinue
        $ed = if ($cv -and $cv.PSObject.Properties['EditionID']) { $cv.EditionID } else { '?' }
        $af = if ($cv -and $cv.PSObject.Properties['DisplayVersion']) { $cv.DisplayVersion } else { '?' }
        Resultat 'Windows' "$($os.Caption) $($os.Version) (build $($os.BuildNumber), $af, édition $ed, type de produit $($os.ProductType) : 1 poste, 3 serveur)"
        Resultat 'Édition « N » (sans multimédia)' (OuiNon ($ed -match 'N$')) 'à lire'
        Resultat 'Processeurs logiques' ([Environment]::ProcessorCount)
        Resultat 'Processeur' ((Get-CimInstance Win32_Processor | Select-Object -First 1).Name)
        Resultat 'Mémoire' ('{0:0.0} Go' -f ($os.TotalVisibleMemorySize / 1MB))
        $p = Get-Process -Id $PID
        Resultat 'Session du processus' $p.SessionId '0 = service, sans bureau'
        Resultat 'Interactif (.NET)' (OuiNon ([Environment]::UserInteractive))

        Section 'Media Foundation (fichiers et fonctionnalité)'
        $s32 = Join-Path $env:windir 'System32'
        foreach ($dll in 'mfplat.dll', 'mfreadwrite.dll', 'mf.dll', 'mfh264enc.dll', 'msmpeg2vdec.dll', 'mfaacenc.dll', 'msauddecmft.dll', 'mfmp4srcsnk.dll', 'mfsrcsnk.dll') {
            $c = Join-Path $s32 $dll
            Resultat "System32\$dll présent" $(if (Test-Path -LiteralPath $c) { "OUI ($(Version-Fichier $c))" } else { 'NON' }) 'à lire'
        }
        if (Get-Command Get-WindowsFeature -ErrorAction SilentlyContinue) {
            try {
                $f = Get-WindowsFeature -Name Server-Media-Foundation -ErrorAction Stop
                Resultat 'Fonctionnalité Server-Media-Foundation' "$($f.InstallState) (installée : $(OuiNon $f.Installed))" 'à lire'
            }
            catch { NonMesurable 'Fonctionnalité Server-Media-Foundation' "Get-WindowsFeature : $($_.Exception.Message)" }
        }
        else { Resultat 'Get-WindowsFeature' 'absent (poste de travail ou module ServerManager absent)' 'à lire' }
        Note 'Get-WindowsCapability (pack multimédia des éditions N) non lu : trop lent sur la machine, et sans objet sur un Windows Server.'

        Section 'Écran et carte graphique'
        foreach ($e in [System.Windows.Forms.Screen]::AllScreens) {
            Resultat "Écran $($e.DeviceName)" "$($e.Bounds.Width) × $($e.Bounds.Height) en ($($e.Bounds.X), $($e.Bounds.Y)), utile $($e.WorkingArea.Width) × $($e.WorkingArea.Height), principal $($e.Primary)"
        }
        $dpi = $script:Natif::GetDpiForSystem()
        Resultat 'Échelle du système' ("{0} ppp ({1:P0})" -f $dpi, ($dpi / 96))
        Resultat 'Mode d''affichage courant' ($script:Natif::ModeCourant())
        Resultat 'Modes d''affichage proposés' (($script:Natif::Modes()) -join ', ') 'à lire (1920x1080 présent : S7 peut essayer le 1080p)'
        foreach ($v in @(Get-CimInstance Win32_VideoController)) {
            Resultat 'Carte graphique' "$($v.Name), pilote $($v.DriverVersion), $($v.CurrentHorizontalResolution) × $($v.CurrentVerticalResolution) à $($v.CurrentRefreshRate) Hz, état $($v.Status)"
        }

        Section 'Son'
        foreach ($s in 'Audiosrv', 'AudioEndpointBuilder') {
            $x = Get-Service -Name $s -ErrorAction SilentlyContinue
            Resultat "Service $s" $(if ($x) { "$($x.Status), démarrage $($x.StartType)" } else { 'absent' }) 'à lire'
        }
        $snd = @(Get-CimInstance Win32_SoundDevice -ErrorAction SilentlyContinue)
        Resultat 'Périphériques son (Win32_SoundDevice)' $(if ($snd.Count) { ($snd | ForEach-Object { "$($_.Name) ($($_.Status))" }) -join ' ; ' } else { 'aucun' }) 'à lire'

        Section 'Navigateurs et vues Web'
        foreach ($c in "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe", "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe") {
            if (Test-Path -LiteralPath $c) { Resultat 'Edge' "$(Version-Fichier $c) ($c)"; break }
        }
        $wv = $null
        foreach ($k in 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
            'HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}') {
            try { $wv = (Get-ItemProperty $k -ErrorAction Stop).pv; break } catch { }
        }
        Resultat 'WebView2 (runtime)' $(if ($wv) { $wv } else { 'absent du registre' })
        Resultat 'Animations des fenêtres (SPI_GETCLIENTAREAANIMATION)' ($script:Natif::AnimationsClient()) '1 : animations ; 0 : « Réduire les animations » (prefers-reduced-motion)'
        Resultat 'Pointeur montré' (OuiNon ($script:Natif::PointeurMontre())) 'à lire (sans souris, Windows le cache : S1 (b) le dit)'

        Section 'Outils'
        Resultat 'PowerShell' $PSVersionTable.PSVersion.ToString()
        Resultat 'Windows PowerShell 5.1 (Get-Clipboard -Format FileDropList, S6)' $(if (Get-Command powershell.exe -ErrorAction SilentlyContinue) { 'présent' } else { 'absent' })
        try { Resultat 'Rust' (& rustc --version 2>$null) } catch { Resultat 'Rust' 'absent' }
    }
    catch { Erreur-Sonde $_ }
}
Terminer-Sonde
