# Étape installateur : l'exe téléchargé s'installe lui-même, se met à jour, se lance, se
# désinstalle (contrat-installateur.md, §8). Joué seulement sur la machine Windows de GitHub.
# Les boutons sont pressés par UI Automation (leur nom exact prouve aussi le texte). Le
# fichier « téléchargé » est lancé sans ShellExecute : l'essai ne dépend jamais de SmartScreen.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
$essai = Initialiser-Essai -Nom '09-installateur' -Exe $Exe -Sortie $Sortie

# --- Aides propres à cet essai -------------------------------------------------------------

# Version du programme, lue dans Cargo.toml (jamais écrite en dur).
function Version-Programme {
    $cargo = Join-Path $PSScriptRoot '../app/coeur/Cargo.toml'
    foreach ($ligne in Get-Content -LiteralPath $cargo) {
        if ($ligne -match '^\s*version\s*=\s*"([^"]+)"') { return $Matches[1] }
    }
    throw "version introuvable dans Cargo.toml"
}

# Journal de l'installateur (installation.log), lu sans gêner le processus qui l'écrit.
function Lire-Installation {
    param([string]$Dossier = $script:Essai.Dossier)
    $chemin = Join-Path $Dossier 'local/Journal/installation.log'
    if (-not (Test-Path -LiteralPath $chemin)) { return '' }
    try {
        $flux = [System.IO.FileStream]::new($chemin, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]'ReadWrite, Delete')
        try { return [System.IO.StreamReader]::new($flux, [System.Text.Encoding]::UTF8).ReadToEnd() }
        finally { $flux.Dispose() }
    }
    catch { return '' }
}

function Compter-Installation {
    param([Parameter(Mandatory)][string]$Texte, [switch]$FinDeLigne)
    $n = 0
    foreach ($ligne in ((Lire-Installation) -split "`n")) {
        $ligne = $ligne.TrimEnd("`r")
        if ($FinDeLigne) { if ($ligne.EndsWith($Texte)) { $n++ } }
        elseif ($ligne.Contains($Texte)) { $n++ }
    }
    return $n
}

function Attendre-Installation {
    param([Parameter(Mandatory)][string]$Texte, [double]$DelaiS = 20, [switch]$FinDeLigne)
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        if ((Compter-Installation -Texte $Texte -FinDeLigne:$FinDeLigne) -ge 1) { return $true }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $limite)
    return $false
}

# La ligne qui contient $Texte (la première), ou $null.
function Ligne-Installation {
    param([Parameter(Mandatory)][string]$Texte)
    foreach ($ligne in ((Lire-Installation) -split "`n")) {
        $ligne = $ligne.TrimEnd("`r")
        if ($ligne.Contains($Texte)) { return $ligne }
    }
    return $null
}

# Lance le fichier « téléchargé » sans ShellExecute, avec le dossier d'essai. Rend le processus.
function Lancer-Telecharge {
    param([Parameter(Mandatory)][string]$Fichier, [string[]]$Arguments = @(), [string]$Dossier = $script:Essai.Dossier)
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $Fichier
    $psi.UseShellExecute = $false
    foreach ($a in $Arguments) { [void]$psi.ArgumentList.Add($a) }
    $psi.EnvironmentVariables['BREEZYSHOT_DOSSIER_ESSAI'] = $Dossier
    $p = [System.Diagnostics.Process]::Start($psi)
    $null = $p.Handle
    return $p
}

# UI Automation : trouve la fenêtre de l'installateur par son titre.
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

function Attendre-Fenetre {
    param([string]$Titre = 'Installer BreezyShot', [double]$DelaiS = 20)
    $racine = [System.Windows.Automation.AutomationElement]::RootElement
    $cond = [System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::NameProperty, $Titre)
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        $f = $racine.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
        if ($null -ne $f) { return $f }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $limite)
    return $null
}

# Presse un bouton par son nom exact (prouve le texte). Rend $true si trouvé et pressé.
function Presser-Bouton {
    param([Parameter(Mandatory)][string]$Nom, [double]$DelaiS = 10)
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        $fenetre = Attendre-Fenetre -DelaiS 2
        if ($null -ne $fenetre) {
            $cond = [System.Windows.Automation.PropertyCondition]::new(
                [System.Windows.Automation.AutomationElement]::NameProperty, $Nom)
            $bouton = $fenetre.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
            if ($null -ne $bouton) {
                $motif = $bouton.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
                $motif.Invoke()
                return $true
            }
        }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $limite)
    return $false
}

# Un texte (nom d'élément) est-il présent dans la fenêtre de l'installateur ?
function Texte-Present {
    param([Parameter(Mandatory)][string]$Texte, [double]$DelaiS = 10)
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        $fenetre = Attendre-Fenetre -DelaiS 2
        if ($null -ne $fenetre) {
            $cond = [System.Windows.Automation.PropertyCondition]::new(
                [System.Windows.Automation.AutomationElement]::NameProperty, $Texte)
            if ($null -ne $fenetre.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)) { return $true }
        }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $limite)
    return $false
}

# Flux nommés (ADS) d'un fichier ; '' si aucun.
function Flux-Nommes {
    param([Parameter(Mandatory)][string]$Fichier)
    try { return @(Get-Item -LiteralPath $Fichier -Stream * -ErrorAction Stop | Where-Object { $_.Stream -ne ':$DATA' } | ForEach-Object { $_.Stream }) }
    catch { return @() }
}

# Clé de désinstallation d'essai, sous HKCU\Software\BreezyShot-essais\<nom du dossier>.
function Cle-Essai {
    $nom = Split-Path -Leaf $script:Essai.Dossier
    return "HKCU:\Software\BreezyShot-essais\$nom\Uninstall\BreezyShot"
}
function Cle-Demarrage-Essai {
    $nom = Split-Path -Leaf $script:Essai.Dossier
    return "HKCU:\Software\BreezyShot-essais\$nom\Run"
}

# Chemins d'installation d'essai.
$version = Version-Programme
$racineEssai = $script:Essai.Dossier
$dossierInstalle = Join-Path $racineEssai 'programmes/BreezyShot'
$exeInstalle = Join-Path $dossierInstalle 'breezyshot.exe'
$raccourci = Join-Path $racineEssai 'menu/BreezyShot.lnk'
$telecharge = Join-Path $racineEssai "telechargement/BreezyShot-$version-essai.exe"

try {
    if (-not $IsWindows) {
        Write-Host "[note]  essai 09 : Windows seulement, sauté sur $([System.Environment]::OSVersion.Platform)."
        Verifier 'essai 09 joué sur Windows' $false 'non Windows' | Out-Null
        Terminer-Essai
    }

    # Prépare le fichier « téléchargé » avec une marque du Web (Zone.Identifier).
    New-Item -ItemType Directory -Force -Path (Split-Path $telecharge) | Out-Null
    Copy-Item -LiteralPath $script:Essai.Exe -Destination $telecharge -Force
    Set-Content -LiteralPath $telecharge -Stream 'Zone.Identifier' -Value "[ZoneTransfer]`r`nZoneId=3"

    # 1. État réel noté avant : il ne doit pas bouger.
    $cleReelleAvant = Test-Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\BreezyShot'
    $dossierReelProgrammes = [Environment]::GetFolderPath('LocalApplicationData')
    $dossierReelAvant = Test-Path (Join-Path $dossierReelProgrammes 'Programs/BreezyShot')

    # 2. Lancement : l'accueil.
    $inst = Lancer-Telecharge -Fichier $telecharge
    Verifier 'plan: installer' (Attendre-Installation -Texte 'plan: installer' -DelaiS 25) | Out-Null
    Verifier 'installateur fenetre (hwnd)' (Attendre-Installation -Texte 'installateur fenetre ') | Out-Null
    Verifier 'fenêtre « Installer BreezyShot » au premier plan' ($null -ne (Attendre-Fenetre)) | Out-Null
    Verifier 'aucune app BreezyShot en cours' ((Processus-App).Count -eq 0) | Out-Null

    # 3. « Installer » : étapes dans l'ordre, installation finie.
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    Verifier 'bouton « Installer » pressé' (Presser-Bouton -Nom 'Installer') | Out-Null
    $finie = Attendre-Installation -Texte "installation finie $version" -DelaiS 20
    $chrono.Stop()
    Verifier "installation finie en moins de 20 s" $finie "$([math]::Round($chrono.Elapsed.TotalSeconds,1)) s" | Out-Null
    Noter-Mesure -Nom "durée d'installation" -Valeur ([math]::Round($chrono.Elapsed.TotalSeconds, 1)) -Unite 's'
    foreach ($etape in @('place', 'arret', 'copie', 'verification', 'raccourci', 'inscription', 'lancement')) {
        Verifier "étape $etape journalisée" ((Compter-Installation -Texte "etape $etape") -ge 1) | Out-Null
    }

    # 4. Exe installé identique octet à octet, sans marque du Web.
    $memes = $false
    if ((Test-Path -LiteralPath $exeInstalle) -and (Test-Path -LiteralPath $telecharge)) {
        $a = [System.IO.File]::ReadAllBytes($exeInstalle)
        $b = [System.IO.File]::ReadAllBytes($telecharge)
        $memes = ($a.Length -eq $b.Length)
        if ($memes) { for ($i = 0; $i -lt $a.Length; $i++) { if ($a[$i] -ne $b[$i]) { $memes = $false; break } } }
    }
    Verifier 'exe installé identique au téléchargé' $memes | Out-Null
    Verifier 'exe installé sans marque du Web (Zone.Identifier)' ((Flux-Nommes -Fichier $exeInstalle).Count -eq 0) | Out-Null

    # 5. Raccourci : cible = exe installé.
    $cibleRaccourci = $null
    if (Test-Path -LiteralPath $raccourci) {
        $shell = New-Object -ComObject WScript.Shell
        $cibleRaccourci = $shell.CreateShortcut($raccourci).TargetPath
    }
    Verifier 'raccourci vers l''exe installé' ($cibleRaccourci -eq $exeInstalle) "$cibleRaccourci" | Out-Null

    # 6. Clé d'essai : les onze valeurs attendues.
    $cle = Cle-Essai
    $valeurs = $null
    if (Test-Path -LiteralPath $cle) { $valeurs = Get-ItemProperty -LiteralPath $cle }
    $commande = "`"$exeInstalle`" --desinstaller"
    $attendues = @{
        DisplayName          = 'BreezyShot'
        DisplayVersion       = $version
        Publisher            = 'BreezyShot'
        DisplayIcon          = "`"$exeInstalle`",0"
        InstallLocation      = $dossierInstalle
        UninstallString      = $commande
        QuietUninstallString = $commande
        NoModify             = 1
        NoRepair             = 1
        InstallDate          = (Get-Date -Format 'yyyyMMdd')
    }
    $cleOk = $null -ne $valeurs
    if ($cleOk) {
        foreach ($nom in $attendues.Keys) {
            $lu = $valeurs.$nom
            if ("$lu" -ne "$($attendues[$nom])") { $cleOk = $false; Write-Host "[note]  $nom = $lu (attendu $($attendues[$nom]))" }
        }
        if ($null -eq $valeurs.EstimatedSize -or $valeurs.EstimatedSize -le 0) { $cleOk = $false }
    }
    Verifier 'clé de désinstallation : onze valeurs exactes' $cleOk | Out-Null

    # 7. L'app installée tourne, depuis l'exe installé, et est prête.
    $procApp = @(Processus-App)
    $bonChemin = $false
    foreach ($q in $procApp) { try { if ($q.Path -eq $exeInstalle) { $bonChemin = $true } } catch { } }
    Verifier 'app installée en cours, depuis l''exe installé' ($procApp.Count -ge 1 -and $bonChemin) | Out-Null
    Verifier 'app installée : journal « pret »' (Attendre-Ligne -Texte ' pret' -FinDeLigne -DelaiS 15) | Out-Null

    # 8. « BreezyShot est prêt » visible.
    Verifier 'écran « BreezyShot est prêt »' (Texte-Present -Texte 'BreezyShot est prêt') | Out-Null

    # 9. « Essayer une capture » : l'app lance une capture de zone ; Échap annule.
    Verifier 'bouton « Essayer une capture » pressé' (Presser-Bouton -Nom 'Essayer une capture') | Out-Null
    Verifier 'app : « --capturer recu: zone »' (Attendre-Ligne -Texte '--capturer recu: zone' -DelaiS 10) | Out-Null
    Verifier 'app : capture de zone lancée' (Attendre-Ligne -Texte 'DemandeCapture zone' -DelaiS 10) | Out-Null
    Verifier 'app : sélection montrée' (Attendre-Ligne -Texte 'selection montree' -DelaiS 10) | Out-Null
    Verifier 'app : jamais « selection sans premier plan »' ((Compter-Lignes -Texte 'selection sans premier plan') -eq 0) | Out-Null
    # Échap : la sélection s'annule.
    Add-Type -Namespace BreezyShotEssai09 -Name Entree -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern void keybd_event(byte vk, byte scan, uint flags, System.UIntPtr extra);
'@
    [BreezyShotEssai09.Entree]::keybd_event(0x1B, 0, 0, [UIntPtr]::Zero)
    [BreezyShotEssai09.Entree]::keybd_event(0x1B, 0, 2, [UIntPtr]::Zero)
    Verifier 'app : sélection annulée (Échap)' (Attendre-Ligne -Texte 'SelectionAnnulee' -DelaiS 10) | Out-Null

    # 10. « Terminer » : la fenêtre s'envole (se déplace vers le bas droite), puis l'installateur sort.
    $fenetreAvant = Attendre-Fenetre -DelaiS 3
    $yAvant = if ($null -ne $fenetreAvant) { $fenetreAvant.Current.BoundingRectangle.Y } else { 0 }
    Verifier 'bouton « Terminer » pressé' (Presser-Bouton -Nom 'Terminer') | Out-Null
    Verifier 'installateur : « envol »' (Attendre-Installation -Texte 'envol cible' -DelaiS 5) | Out-Null
    $sorti = $inst.WaitForExit(5000)
    Verifier 'installateur sorti en moins de 5 s après Terminer' $sorti | Out-Null
    if ($sorti) { Verifier 'installateur : code de sortie 0' ($inst.ExitCode -eq 0) "code $($inst.ExitCode)" | Out-Null }
    Verifier 'app toujours en cours après Terminer' ((Processus-App).Count -ge 1) | Out-Null

    # Ferme l'app installée avant la suite.
    Quitter-App | Out-Null
    Start-Sleep -Milliseconds 500

    # 11. Relance du téléchargé, app ouverte : réinstallation, arrêt de l'instance.
    Demarrer-App -Exe $exeInstalle | Out-Null
    Verifier 'app relancée (pret)' (Attendre-Ligne -Texte ' pret' -FinDeLigne -DelaiS 15 -Fois 2) | Out-Null
    $inst2 = Lancer-Telecharge -Fichier $telecharge
    Verifier 'plan: reinstaller' (Attendre-Installation -Texte 'plan: reinstaller' -DelaiS 20) | Out-Null
    Verifier 'bouton « Installer » (réinstallation) pressé' (Presser-Bouton -Nom 'Installer') | Out-Null
    Verifier 'réinstallation : « etape arret: instance arretee »' (Attendre-Installation -Texte 'etape arret: instance arretee' -DelaiS 20) | Out-Null
    Verifier 'réinstallation finie' (Attendre-Installation -Texte 'installation finie' -DelaiS 20) | Out-Null
    if (-not $inst2.WaitForExit(8000)) { try { $inst2.Kill() } catch { } }
    Presser-Bouton -Nom 'Terminer' -DelaiS 3 | Out-Null
    Quitter-App | Out-Null
    Start-Sleep -Milliseconds 500

    # 12. Mise à jour et version plus récente (clé réécrite, puis exe non réécrit).
    Set-ItemProperty -LiteralPath $cle -Name 'DisplayVersion' -Value '0.0.1'
    $instMaj = Lancer-Telecharge -Fichier $telecharge
    Verifier 'plan: mettre a jour depuis 0.0.1' (Attendre-Installation -Texte 'plan: mettre a jour depuis 0.0.1' -DelaiS 20) | Out-Null
    if (-not $instMaj.WaitForExit(2000)) { try { $instMaj.Kill() } catch { } }
    Set-ItemProperty -LiteralPath $cle -Name 'DisplayVersion' -Value '99.0.0'
    $ecritAvant = (Get-Item -LiteralPath $exeInstalle).LastWriteTimeUtc
    $instRecent = Lancer-Telecharge -Fichier $telecharge
    Verifier 'plan: plus recente installee 99.0.0' (Attendre-Installation -Texte 'plan: plus recente installee 99.0.0' -DelaiS 20) | Out-Null
    Verifier 'version plus récente : « prêt » direct' (Texte-Present -Texte 'BreezyShot est prêt' -DelaiS 10) | Out-Null
    Start-Sleep -Milliseconds 500
    Verifier 'version plus récente : exe non réécrit' ((Get-Item -LiteralPath $exeInstalle).LastWriteTimeUtc -eq $ecritAvant) | Out-Null
    Presser-Bouton -Nom 'Terminer' -DelaiS 3 | Out-Null
    if (-not $instRecent.WaitForExit(5000)) { try { $instRecent.Kill() } catch { } }
    Quitter-App | Out-Null
    Set-ItemProperty -LiteralPath $cle -Name 'DisplayVersion' -Value $version
    Start-Sleep -Milliseconds 500

    # 13. Erreur d'écriture puis Réessayer : l'essai tient l'exe installé ouvert sans partage.
    Arreter-Tout
    $flux = [System.IO.FileStream]::new($exeInstalle, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::None)
    $instErr = Lancer-Telecharge -Fichier $telecharge
    Presser-Bouton -Nom 'Installer' -DelaiS 15 | Out-Null
    Verifier 'erreur : « installation erreur: ecriture »' (Attendre-Installation -Texte 'installation erreur: ecriture' -DelaiS 25) | Out-Null
    Verifier 'erreur : texte affiché' (Texte-Present -Texte "Impossible d'écrire dans le dossier de BreezyShot. Réessayez dans un instant." -DelaiS 8) | Out-Null
    $flux.Dispose()
    Presser-Bouton -Nom 'Réessayer' -DelaiS 10 | Out-Null
    Verifier 'Réessayer : installation finie' (Attendre-Installation -Texte 'installation finie' -DelaiS 20) | Out-Null
    Presser-Bouton -Nom 'Terminer' -DelaiS 3 | Out-Null
    if (-not $instErr.WaitForExit(5000)) { try { $instErr.Kill() } catch { } }
    Quitter-App | Out-Null
    Start-Sleep -Milliseconds 500

    # 15. Erreur de place : place libre simulée à 1 octet.
    $instPlace = Lancer-Telecharge -Fichier $telecharge -Arguments @()
    $env:BREEZYSHOT_ESSAI_LIBRE_OCTETS = '1'
    # (la variable est héritée par le prochain lancement ; on relance avec elle)
    $instPlace.Kill()
    $psiPlace = [System.Diagnostics.ProcessStartInfo]::new()
    $psiPlace.FileName = $telecharge
    $psiPlace.UseShellExecute = $false
    $psiPlace.EnvironmentVariables['BREEZYSHOT_DOSSIER_ESSAI'] = $racineEssai
    $psiPlace.EnvironmentVariables['BREEZYSHOT_ESSAI_LIBRE_OCTETS'] = '1'
    $instPlace2 = [System.Diagnostics.Process]::Start($psiPlace)
    Presser-Bouton -Nom 'Installer' -DelaiS 15 | Out-Null
    Verifier 'erreur de place journalisée' (Attendre-Installation -Texte 'installation erreur: place' -DelaiS 15) | Out-Null
    $ligPlace = Ligne-Installation -Texte 'etape place: libre'
    Verifier 'texte « il manque de la place » affiché' (Texte-Present -Texte 'Il manque de la place' -DelaiS 8) | Out-Null
    Remove-Item Env:\BREEZYSHOT_ESSAI_LIBRE_OCTETS -ErrorAction SilentlyContinue
    if (-not $instPlace2.WaitForExit(3000)) { Presser-Bouton -Nom 'Fermer' -DelaiS 3 | Out-Null; if (-not $instPlace2.WaitForExit(3000)) { try { $instPlace2.Kill() } catch { } } }

    # 16. Second installateur : refusé, le premier reste.
    $premier = Lancer-Telecharge -Fichier $telecharge
    $null = Attendre-Fenetre
    $second = Lancer-Telecharge -Fichier $telecharge
    $secondSorti = $second.WaitForExit(8000)
    Verifier 'second installateur sorti' $secondSorti | Out-Null
    Verifier 'second installateur : code 0' ($secondSorti -and $second.ExitCode -eq 0) | Out-Null
    Verifier 'premier installateur toujours là' (-not $premier.HasExited) | Out-Null
    Presser-Bouton -Nom 'Fermer' -DelaiS 3 | Out-Null
    if (-not $premier.WaitForExit(4000)) { try { $premier.Kill() } catch { } }

    # 18. Désinstallation, app ouverte, données posées avant : retire la nôtre, garde l'autre.
    Demarrer-App -Exe $exeInstalle | Out-Null
    Start-Sleep -Seconds 1
    # Démarrage d'essai pointant notre exe : il doit être retiré.
    New-Item -Path (Cle-Demarrage-Essai) -Force | Out-Null
    # Réglages et un fichier de Récents, posés avant (jamais touchés).
    $reglages = Join-Path $racineEssai 'reglages/reglages.json'
    $recent = Join-Path $racineEssai 'local/Recents/temoin.png'
    New-Item -ItemType Directory -Force -Path (Split-Path $reglages) | Out-Null
    New-Item -ItemType Directory -Force -Path (Split-Path $recent) | Out-Null
    Set-Content -LiteralPath $reglages -Value '{"version":1}' -Encoding utf8
    Set-Content -LiteralPath $recent -Value 'temoin' -Encoding utf8
    $empreinteReglages = (Get-FileHash -LiteralPath $reglages).Hash
    $desinst = Lancer-Telecharge -Fichier $exeInstalle -Arguments @('--desinstaller')
    $desinstSorti = $desinst.WaitForExit(20000)
    Verifier 'désinstallation sortie en moins de 20 s' $desinstSorti | Out-Null
    Verifier 'désinstallation : code 0' ($desinstSorti -and $desinst.ExitCode -eq 0) | Out-Null
    Verifier 'désinstallation : « desinstallation finie »' (Attendre-Installation -Texte 'desinstallation finie' -DelaiS 5) | Out-Null
    Verifier 'désinstallation : app arrêtée' ((Processus-App).Count -eq 0) | Out-Null
    Verifier 'clé de désinstallation retirée' (-not (Test-Path -LiteralPath $cle)) | Out-Null
    Verifier 'raccourci retiré' (-not (Test-Path -LiteralPath $raccourci)) | Out-Null
    Verifier 'exe installé retiré (déplacé)' (-not (Test-Path -LiteralPath $exeInstalle)) | Out-Null
    Verifier 'réglages intacts' ((Test-Path -LiteralPath $reglages) -and ((Get-FileHash -LiteralPath $reglages).Hash -eq $empreinteReglages)) | Out-Null
    Verifier 'Récents intacts' (Test-Path -LiteralPath $recent) | Out-Null

    # 19. WebView2 absent simulé : message, code 1, rien d'installé.
    $psiSans = [System.Diagnostics.ProcessStartInfo]::new()
    $psiSans.FileName = $telecharge
    $psiSans.UseShellExecute = $false
    $psiSans.EnvironmentVariables['BREEZYSHOT_DOSSIER_ESSAI'] = $racineEssai
    $psiSans.EnvironmentVariables['BREEZYSHOT_ESSAI_SANS_WEBVIEW2'] = '1'
    $instSans = [System.Diagnostics.Process]::Start($psiSans)
    Verifier 'webview2 absent journalisé' (Attendre-Installation -Texte 'installateur: webview2 absent' -DelaiS 15) | Out-Null
    # La boîte système (#32770) : on la trouve et on la ferme.
    $boite = Attendre-Fenetre -Titre 'Installer BreezyShot' -DelaiS 8
    $texteBoite = Texte-Present -Texte 'BreezyShot a besoin de Microsoft Edge WebView2, absent de ce PC.' -DelaiS 8
    Verifier 'webview2 absent : message affiché' $texteBoite | Out-Null
    Presser-Bouton -Nom 'OK' -DelaiS 5 | Out-Null
    $sansSorti = $instSans.WaitForExit(8000)
    if (-not $sansSorti) { try { $instSans.Kill() } catch { } }
    Verifier 'webview2 absent : code 1' ($sansSorti -and $instSans.ExitCode -eq 1) | Out-Null
    Verifier 'webview2 absent : rien réinstallé' (-not (Test-Path -LiteralPath $exeInstalle)) | Out-Null

    # 20. Fin : l'état réel n'a pas bougé, aucun processus restant.
    Arreter-Tout
    $cleReelleApres = Test-Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\BreezyShot'
    $dossierReelApres = Test-Path (Join-Path $dossierReelProgrammes 'Programs/BreezyShot')
    Verifier 'clé de désinstallation réelle inchangée' ($cleReelleAvant -eq $cleReelleApres) | Out-Null
    Verifier 'dossier installé réel inchangé' ($dossierReelAvant -eq $dossierReelApres) | Out-Null
    Verifier 'aucun processus BreezyShot restant' ((Processus-App).Count -eq 0) | Out-Null
    Noter-Mesure -Nom 'taille du téléchargé' -Valeur ([math]::Round((Get-Item -LiteralPath $telecharge).Length / 1MB, 1)) -Unite 'Mo'
}
catch {
    Write-Host "ERREUR : $($_.Exception.Message)"
    Write-Host $_.ScriptStackTrace
    Verifier 'essai 09 sans exception' $false $_.Exception.Message | Out-Null
}
finally {
    # Nettoyage des clés d'essai (jamais les vraies).
    $nom = Split-Path -Leaf $script:Essai.Dossier
    Remove-Item -LiteralPath "HKCU:\Software\BreezyShot-essais\$nom" -Recurse -Force -ErrorAction SilentlyContinue
    Terminer-Essai
}
