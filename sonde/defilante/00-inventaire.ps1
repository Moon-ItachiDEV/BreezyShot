# Inventaire de la machine avant la sonde : Windows, écran (résolution, échelle, nombre),
# carte graphique, navigateurs et leurs versions, WebView2, .NET, PowerShell, Node,
# session interactive, réglages de la molette, délai des crochets bas niveau.
#   pwsh -NoProfile -File sonde/defilante/00-inventaire.ps1

. "$PSScriptRoot/commun.ps1"

function Version-Fichier([string]$Chemin) {
    try { return [System.Diagnostics.FileVersionInfo]::GetVersionInfo($Chemin).ProductVersion } catch { return '?' }
}

if (Demarrer-Sonde -Nom 'inventaire' -Titre 'Inventaire de la machine') {
    try {
        Section 'Système'
        $os = Get-CimInstance Win32_OperatingSystem
        $affichee = ''
        try { $affichee = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion').DisplayVersion } catch { }
        Resultat 'Windows' "$($os.Caption) $($os.Version) (build $($os.BuildNumber), $affichee)"
        Resultat 'Processeurs logiques' ([Environment]::ProcessorCount)
        Resultat 'Mémoire' ('{0:0.0} Go' -f ($os.TotalVisibleMemorySize / 1MB))

        Section 'Session'
        $p = Get-Process -Id $PID
        Resultat 'Session du processus' $p.SessionId '0 = service, sans bureau'
        Resultat 'Interactif (.NET)' ([Environment]::UserInteractive)
        $exp = @(Get-Process -Name explorer -ErrorAction SilentlyContinue)
        Resultat 'Explorateur (shell) en cours' $(if ($exp.Count -gt 0) { "oui ($($exp.Count), session $($exp[0].SessionId))" } else { 'non' })
        $avant = $script:N::GetForegroundWindow()
        Resultat 'Fenêtre au premier plan' ($script:N::Decrire($avant))

        Section 'Écran'
        $ecrans = [System.Windows.Forms.Screen]::AllScreens
        Resultat 'Écrans' $ecrans.Count 'un seul attendu (essais : 1024 × 768 à 100 %)'
        foreach ($e in $ecrans) {
            Resultat "Écran $($e.DeviceName)" "$($e.Bounds.Width) × $($e.Bounds.Height) en ($($e.Bounds.X), $($e.Bounds.Y)), utile $($e.WorkingArea.Width) × $($e.WorkingArea.Height), principal $($e.Primary)"
        }
        $dpi = $script:N::GetDpiForSystem()
        Resultat 'Échelle du système' ("{0} ppp ({1:P0})" -f $dpi, ($dpi / 96))
        try {
            $lp = (Get-ItemProperty 'HKCU:\Control Panel\Desktop' -ErrorAction Stop).LogPixels
            Resultat 'LogPixels (HKCU)' $(if ($null -ne $lp) { $lp } else { 'absent' })
        }
        catch { Resultat 'LogPixels (HKCU)' 'absent' }
        foreach ($v in @(Get-CimInstance Win32_VideoController)) {
            Resultat "Carte graphique" "$($v.Name), pilote $($v.DriverVersion), $($v.CurrentHorizontalResolution) × $($v.CurrentVerticalResolution) à $($v.CurrentRefreshRate) Hz"
        }

        Section 'Navigateurs et vues Web'
        foreach ($g in 'edge', 'chrome', 'firefox') {
            $c = Chemin-Navigateur $g
            Resultat $g $(if ($c) { "$(Version-Fichier $c) ($c)" } else { 'absent' })
        }
        $e = Chemin-Navigateur 'electron'
        Resultat 'electron' $(if ($e) { "$(Version-Fichier $e) ($e)" } else { 'absent (voir l''étape Electron)' })
        $wv = $null
        foreach ($k in 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
            'HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}') {
            try { $wv = (Get-ItemProperty $k -ErrorAction Stop).pv; break } catch { }
        }
        Resultat 'WebView2 (runtime)' $(if ($wv) { $wv } else { 'absent du registre' })
        $parametres = Join-Path $env:windir 'ImmersiveControlPanel\SystemSettings.exe'
        Resultat 'App Paramètres (WinUI, cible du contrat)' $(if (Test-Path $parametres) { "présente ($parametres)" } else { 'absente' })

        Section 'Outils'
        Resultat 'PowerShell' $PSVersionTable.PSVersion.ToString()
        Resultat '.NET (de pwsh)' ([System.Runtime.InteropServices.RuntimeInformation]::FrameworkDescription)
        try { Resultat '.NET installés' ((& dotnet --list-runtimes 2>$null) -join ' ; ') } catch { Resultat '.NET installés' 'dotnet absent' }
        try { Resultat 'Node' (& node --version 2>$null) } catch { Resultat 'Node' 'absent' }
        try { Resultat 'Rust' (& rustc --version 2>$null) } catch { Resultat 'Rust' 'absent' }

        Section 'Souris et crochets'
        Resultat 'Lignes par cran (SPI_GETWHEELSCROLLLINES)' ($script:N::LignesParCran())
        Resultat 'Routage de la molette (SPI_GETMOUSEWHEELROUTING)' ($script:N::LireRoutageMolette()) '0 premier plan, 1 hybride, 2 sous le pointeur (« faire défiler les fenêtres inactives »)'
        try {
            $d = Get-ItemProperty 'HKCU:\Control Panel\Desktop' -ErrorAction Stop
            Resultat 'MouseWheelRouting (HKCU)' $(if ($d.PSObject.Properties['MouseWheelRouting']) { $d.MouseWheelRouting } else { 'absent' })
            Resultat 'LowLevelHooksTimeout (HKCU)' $(if ($d.PSObject.Properties['LowLevelHooksTimeout']) { "$($d.LowLevelHooksTimeout) ms" } else { 'absent (défaut du système)' })
            Resultat 'SmoothScroll (HKCU)' $(if ($d.PSObject.Properties['SmoothScroll']) { $d.SmoothScroll } else { 'absent' })
        }
        catch { NonMesurable 'Réglages HKCU\Control Panel\Desktop' $_.Exception.Message }
        Resultat 'Pointeur montré' ($script:N::PointeurMontre())
    }
    catch { Erreur-Sonde $_ }
}
Terminer-Sonde
