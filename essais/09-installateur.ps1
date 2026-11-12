# Étape installateur : l'exe téléchargé s'installe lui-même, se met à jour, se lance, se
# désinstalle (ARCHITECTURE §12). Joué seulement sur la machine Windows de GitHub.
# Les boutons sont trouvés par UI Automation, par leur nom exact ET leur type (bouton) : le
# nom prouve aussi le texte. Chaque appui passe par Verifier ; une recherche qui échoue dit si
# la fenêtre est là et ce qu'elle contient. Le fichier « téléchargé » est lancé sans
# ShellExecute : l'essai ne dépend jamais de SmartScreen.
# Le journal installation.log s'accumule d'un point à l'autre : chaque attente part d'une
# marque (nombre de lignes avant l'action) et ne regarde que les lignes NOUVELLES.
# Un installateur se ferme par son interface (« Terminer » ou « Fermer ») puis WaitForExit ;
# le tuer n'arrive qu'en dernier recours, et c'est alors un échec écrit.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
$essai = Initialiser-Essai -Nom '09-installateur' -Exe $Exe -Sortie $Sortie

# --- Journal de l'installateur -------------------------------------------------------------

# Version du programme, lue dans Cargo.toml (jamais écrite en dur).
function Version-Programme {
    $cargo = Join-Path $PSScriptRoot '../app/coeur/Cargo.toml'
    foreach ($ligne in Get-Content -LiteralPath $cargo) {
        if ($ligne -match '^\s*version\s*=\s*"([^"]+)"') { return $Matches[1] }
    }
    throw "version introuvable dans Cargo.toml"
}

# Lignes de installation.log, lues sans gêner le processus qui l'écrit. Appeler avec @(…).
function Lignes-Installation {
    param([string]$Dossier = $script:Essai.Dossier)
    $chemin = Join-Path $Dossier 'local/Journal/installation.log'
    if (-not (Test-Path -LiteralPath $chemin)) { return }
    $texte = ''
    try {
        $flux = [System.IO.FileStream]::new($chemin, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]'ReadWrite, Delete')
        try { $texte = [System.IO.StreamReader]::new($flux, [System.Text.Encoding]::UTF8).ReadToEnd() }
        finally { $flux.Dispose() }
    }
    catch { return }
    foreach ($l in ($texte -split "`n")) {
        $l = $l.TrimEnd("`r")
        if ($l -ne '') { $l }
    }
}

# Marque : nombre de lignes déjà écrites. Les attentes ne regardent que les suivantes.
function Marque {
    return @(Lignes-Installation).Count
}

function Nouvelles-Lignes {
    param([int]$Depuis = 0)
    $toutes = @(Lignes-Installation)
    for ($i = $Depuis; $i -lt $toutes.Count; $i++) { $toutes[$i] }
}

# Première NOUVELLE ligne (après la marque) qui contient $Texte, ou $null.
function Ligne-Installation {
    param([Parameter(Mandatory)][string]$Texte, [int]$Depuis = 0)
    foreach ($l in @(Nouvelles-Lignes -Depuis $Depuis)) { if ($l.Contains($Texte)) { return $l } }
    return $null
}

function Compter-Installation {
    param([Parameter(Mandatory)][string]$Texte, [int]$Depuis = 0)
    return @(@(Nouvelles-Lignes -Depuis $Depuis) | Where-Object { $_.Contains($Texte) }).Count
}

function Attendre-Installation {
    param([Parameter(Mandatory)][string]$Texte, [int]$Depuis = 0, [double]$DelaiS = 20)
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        if ($null -ne (Ligne-Installation -Texte $Texte -Depuis $Depuis)) { return $true }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $limite)
    return $false
}

# Après un lancement : attend que la page de l'installateur soit prête (« installateur
# pret »), jusqu'à 60 s. Sur la machine de GitHub, WebView2 a mis plus de 15 s à charger la
# page, deux fois (fenêtre montrée vide par le délai de 2 s) : un bouton n'est donc cherché
# qu'ensuite, et le lancement le plus lent est noté en mesure. Rend vrai si la page est prête.
$script:DemarrageMaxS = 0.0
function Attendre-Page {
    param([int]$Depuis = 0, [Parameter(Mandatory)][string]$Quoi)
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    $vu = Attendre-Installation -Texte 'installateur pret' -Depuis $Depuis -DelaiS 60
    $s = [math]::Round($chrono.Elapsed.TotalSeconds, 1)
    if ($s -gt $script:DemarrageMaxS) { $script:DemarrageMaxS = $s }
    if (-not $vu) {
        $lignes = @(Nouvelles-Lignes -Depuis $Depuis | Select-Object -Last 8) -join ' | '
        Verifier "$Quoi : page prête en moins de 60 s" $false "journal : $lignes" | Out-Null
    }
    elseif ($s -gt 5) { Write-Host "[lent]  $Quoi : page prête après $s s" }
    return $vu
}

# --- Processus ---------------------------------------------------------------------------

# Lance le fichier « téléchargé » sans ShellExecute, avec le dossier d'essai (et d'autres
# variables au besoin). Rend le processus.
function Lancer-Telecharge {
    param(
        [Parameter(Mandatory)][string]$Fichier,
        [string[]]$Arguments = @(),
        [hashtable]$Variables = @{},
        [string]$Dossier = $script:Essai.Dossier
    )
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $Fichier
    $psi.UseShellExecute = $false
    foreach ($a in $Arguments) { [void]$psi.ArgumentList.Add($a) }
    $psi.EnvironmentVariables['BREEZYSHOT_DOSSIER_ESSAI'] = $Dossier
    foreach ($k in $Variables.Keys) { $psi.EnvironmentVariables[$k] = [string]$Variables[$k] }
    $p = [System.Diagnostics.Process]::Start($psi)
    $null = $p.Handle
    return $p
}

# Une « app qui ne se ferme pas » : un processus à part qui tient le mutex de l'instance
# unique de BreezyShot (`dev.breezyshot.desktop-sim`) et ne répond pas à --quitter.
# L'installateur ne doit jamais le tuer.
function Demarrer-Tetu {
    $script_ = '$m = [System.Threading.Mutex]::new($true, ''dev.breezyshot.desktop-sim''); Start-Sleep -Seconds 120'
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = (Get-Process -Id $PID).Path
    foreach ($a in @('-NoProfile', '-NonInteractive', '-Command', $script_)) { [void]$psi.ArgumentList.Add($a) }
    $psi.UseShellExecute = $false
    $p = [System.Diagnostics.Process]::Start($psi)
    $null = $p.Handle
    $limite = [DateTime]::UtcNow.AddSeconds(15)
    $m = $null
    do {
        if ([System.Threading.Mutex]::TryOpenExisting('dev.breezyshot.desktop-sim', [ref]$m)) { $m.Dispose(); return $p }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $limite)
    return $p
}

# Arrête le processus têtu de l'essai (le nôtre, pas une app BreezyShot).
function Arreter-Tetu {
    param([Parameter(Mandatory)][System.Diagnostics.Process]$Processus)
    if (-not $Processus.HasExited) { try { $Processus.Kill() } catch { } }
    $null = $Processus.WaitForExit(5000)
}

# --- UI Automation -------------------------------------------------------------------------

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$AE = [System.Windows.Automation.AutomationElement]
$Portee = [System.Windows.Automation.TreeScope]
$TitreInstallateur = 'Installer BreezyShot'

function Fenetre-Installateur {
    param([string]$Titre = $TitreInstallateur)
    $cond = [System.Windows.Automation.PropertyCondition]::new($AE::NameProperty, $Titre)
    return $AE::RootElement.FindFirst($Portee::Children, $cond)
}

function Attendre-Fenetre {
    param([string]$Titre = $TitreInstallateur, [double]$DelaiS = 20)
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        $f = Fenetre-Installateur -Titre $Titre
        if ($null -ne $f) { return $f }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $limite)
    return $null
}

function Attendre-Sans-Fenetre {
    param([string]$Titre = $TitreInstallateur, [double]$DelaiS = 5)
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        $f = Fenetre-Installateur -Titre $Titre
        if ($null -eq $f) { return $true }
        try { if ($f.Current.IsOffscreen) { return $true } } catch { return $true }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $limite)
    return $false
}

# Ce que contient la fenêtre, pour un échec lisible : trouvée ou non, puis le type et le nom
# des 40 premiers descendants.
function Decrire-Fenetre {
    param([string]$Titre = $TitreInstallateur)
    $f = Fenetre-Installateur -Titre $Titre
    if ($null -eq $f) { return "fenêtre « $Titre » introuvable" }
    $tous = $f.FindAll($Portee::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    $n = [Math]::Min(40, $tous.Count)
    $parts = @(for ($i = 0; $i -lt $n; $i++) {
            $c = $tous[$i].Current
            "$($c.ControlType.ProgrammaticName -replace '^ControlType\.', '')='$($c.Name)'"
        })
    return "fenêtre trouvée, $($tous.Count) descendants : " + ($parts -join ' ; ')
}

# Un bouton par son nom exact ET son type ; $null au-delà du délai.
function Trouver-Bouton {
    param([Parameter(Mandatory)][string]$Nom, [double]$DelaiS = 10, [string]$Titre = $TitreInstallateur)
    $cond = [System.Windows.Automation.AndCondition]::new(
        [System.Windows.Automation.PropertyCondition]::new($AE::NameProperty, $Nom),
        [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, [System.Windows.Automation.ControlType]::Button))
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        $f = Fenetre-Installateur -Titre $Titre
        if ($null -ne $f) {
            $b = $f.FindFirst($Portee::Descendants, $cond)
            if ($null -ne $b) { return $b }
        }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $limite)
    return $null
}

# Presse un bouton ; toujours une vérification écrite. Rend la condition.
function Presser {
    param([Parameter(Mandatory)][string]$Nom, [Parameter(Mandatory)][string]$Quoi, [double]$DelaiS = 10, [string]$Titre = $TitreInstallateur)
    $b = Trouver-Bouton -Nom $Nom -DelaiS $DelaiS -Titre $Titre
    if ($null -eq $b) { return Verifier $Quoi $false "bouton « $Nom » introuvable ; $(Decrire-Fenetre -Titre $Titre)" }
    try {
        $b.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        return Verifier $Quoi $true
    }
    catch { return Verifier $Quoi $false "Invoke : $($_.Exception.Message)" }
}

# Un bouton présent, visible et actif (un texte à opacité 0 reste lu par UIA : le bouton
# prouve l'écran).
function Verifier-Bouton-Utilisable {
    param([Parameter(Mandatory)][string]$Nom, [Parameter(Mandatory)][string]$Quoi, [double]$DelaiS = 10)
    $b = Trouver-Bouton -Nom $Nom -DelaiS $DelaiS
    if ($null -eq $b) { return Verifier $Quoi $false "bouton « $Nom » introuvable ; $(Decrire-Fenetre)" }
    $c = $b.Current
    return Verifier $Quoi ((-not $c.IsOffscreen) -and $c.IsEnabled) "visible=$(-not $c.IsOffscreen) actif=$($c.IsEnabled)"
}

# Un texte présent dans la fenêtre (nom exact, ou début du nom avec -Debut).
function Verifier-Texte {
    param([Parameter(Mandatory)][string]$Texte, [Parameter(Mandatory)][string]$Quoi, [double]$DelaiS = 10, [switch]$Debut, [string]$Titre = $TitreInstallateur)
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        $f = Fenetre-Installateur -Titre $Titre
        if ($null -ne $f) {
            if ($Debut) {
                foreach ($e in $f.FindAll($Portee::Descendants, [System.Windows.Automation.Condition]::TrueCondition)) {
                    if ($e.Current.Name.StartsWith($Texte)) { return Verifier $Quoi $true $e.Current.Name }
                }
            }
            else {
                $cond = [System.Windows.Automation.PropertyCondition]::new($AE::NameProperty, $Texte)
                if ($null -ne $f.FindFirst($Portee::Descendants, $cond)) { return Verifier $Quoi $true }
            }
        }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $limite)
    return Verifier $Quoi $false "texte « $Texte » absent ; $(Decrire-Fenetre -Titre $Titre)"
}

# Ferme un installateur par son interface, puis attend sa sortie. Le tuer n'arrive qu'en
# dernier recours (avec ses processus WebView2), et c'est un échec écrit.
function Fermer-Installateur {
    param(
        [Parameter(Mandatory)][System.Diagnostics.Process]$Processus,
        [Parameter(Mandatory)][string]$Bouton,
        [Parameter(Mandatory)][string]$Quoi,
        [double]$DelaiS = 10,
        [int]$Code = 0
    )
    if (-not $Processus.HasExited) {
        $null = Presser -Nom $Bouton -Quoi "$Quoi : « $Bouton » pressé"
    }
    $sorti = $Processus.WaitForExit([int]($DelaiS * 1000))
    if (-not $sorti) {
        try { $Processus.Kill($true) } catch { }
        $null = $Processus.WaitForExit(5000)
        return Verifier "$Quoi : installateur sorti par son interface" $false "toujours là après $DelaiS s : tué en dernier recours"
    }
    $Processus.WaitForExit()
    return Verifier "$Quoi : installateur sorti par son interface, code $Code" ($Processus.ExitCode -eq $Code) "code $($Processus.ExitCode)"
}

# Fenêtres : placer, lire la place réelle (pixels réels), clavier.
Add-Type -Namespace BreezyShotEssai09 -Name Natif -MemberDefinition @'
[System.Runtime.InteropServices.StructLayout(System.Runtime.InteropServices.LayoutKind.Sequential)]
public struct RECT { public int Left, Top, Right, Bottom; }
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern bool GetWindowRect(System.IntPtr h, out RECT r);
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern bool SetWindowPos(System.IntPtr h, System.IntPtr apres, int x, int y, int l, int ht, uint options);
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern void keybd_event(byte vk, byte scan, uint flags, System.UIntPtr extra);
[System.Runtime.InteropServices.DllImport("user32.dll", CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
public static extern System.IntPtr FindWindowW(string classe, string titre);
[System.Runtime.InteropServices.DllImport("user32.dll", CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
public static extern System.IntPtr FindWindowExW(System.IntPtr parent, System.IntPtr apres, string classe, string titre);
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern bool PostMessageW(System.IntPtr h, uint message, System.IntPtr w, System.IntPtr l);
'@

function Place-Fenetre {
    param([Parameter(Mandatory)][System.IntPtr]$Poignee)
    $r = [BreezyShotEssai09.Natif+RECT]::new()
    if (-not [BreezyShotEssai09.Natif]::GetWindowRect($Poignee, [ref]$r)) { return $null }
    return [pscustomobject]@{ X = $r.Left; Y = $r.Top }
}

# « Terminer » sur l'écran prêt : l'essai déplace d'abord la fenêtre, presse Terminer, relève
# sa place pendant la fermeture, puis lit au journal LEQUEL des deux chemins a joué et le
# vérifie : `envol: anime` (la fenêtre part de sa place réelle et se déplace, `envol cible`)
# ou `envol: fondu` (animations coupées par Windows : fenêtre immobile). -Force : l'essai a
# lancé l'installateur avec BREEZYSHOT_ESSAI_ANIMATIONS=1, l'envol doit jouer.
function Verifier-Envol {
    param(
        [Parameter(Mandatory)][System.Diagnostics.Process]$Processus,
        [Parameter(Mandatory)][string]$Quoi,
        [switch]$Force
    )
    $fen = Attendre-Fenetre -DelaiS 3
    $poignee = if ($null -ne $fen) { [System.IntPtr]$fen.Current.NativeWindowHandle } else { [System.IntPtr]::Zero }
    $depart = $null
    if ($poignee -ne [System.IntPtr]::Zero) {
        $avant = Place-Fenetre -Poignee $poignee
        # SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE
        [void][BreezyShotEssai09.Natif]::SetWindowPos($poignee, [System.IntPtr]::Zero, $avant.X - 200, $avant.Y - 150, 0, 0, 0x0001 -bor 0x0004 -bor 0x0010)
        Start-Sleep -Milliseconds 200
        $depart = Place-Fenetre -Poignee $poignee
    }
    Verifier "$Quoi : fenêtre déplacée par l'essai avant Terminer" ($null -ne $depart) (Decrire-Fenetre) | Out-Null
    $m = Marque
    $null = Presser -Nom 'Terminer' -Quoi "$Quoi : « Terminer » pressé"
    $places = [System.Collections.Generic.List[object]]::new()
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    while ($chrono.ElapsedMilliseconds -lt 1500 -and -not $Processus.HasExited -and $null -ne $depart) {
        $q = Place-Fenetre -Poignee $poignee
        if ($null -ne $q) { $places.Add($q) }
        Start-Sleep -Milliseconds 15
    }
    $sorti = $Processus.WaitForExit(5000)
    $null = Attendre-Installation -Texte 'installateur ferme en' -Depuis $m -DelaiS 2
    $vues = @(@(Nouvelles-Lignes -Depuis $m) | Where-Object { $_ -match 'envol|fermeture|installateur ferme' })
    $journal = 'journal : ' + $(if ($vues.Count -gt 0) { $vues -join ' | ' } else { 'aucune ligne envol ni fermeture' })
    $anime = @($vues | Where-Object { $_.Contains('envol: anime') }).Count -gt 0
    $fondu = @($vues | Where-Object { $_.Contains('envol: fondu') }).Count -gt 0
    Verifier "$Quoi : chemin journalisé (envol: anime ou envol: fondu)" ($anime -xor $fondu) $journal | Out-Null
    if ($Force) { Verifier "$Quoi : envol forcé par l'essai (envol: anime)" $anime $journal | Out-Null }
    if ($null -ne $depart) {
        $bouge = @($places | Where-Object { [Math]::Abs($_.X - $depart.X) + [Math]::Abs($_.Y - $depart.Y) -gt 4 })
        if ($anime) {
            Verifier "$Quoi : « envol cible »" (@($vues | Where-Object { $_.Contains('envol cible') }).Count -gt 0) $journal | Out-Null
            Verifier "$Quoi : la fenêtre se déplace" ($bouge.Count -gt 0) "$($places.Count) relevés ; $journal" | Out-Null
            $saut = if ($bouge.Count -gt 0) { [Math]::Abs($bouge[0].X - $depart.X) + [Math]::Abs($bouge[0].Y - $depart.Y) } else { -1 }
            Verifier "$Quoi : part de la place réelle (pas de saut au centre)" ($saut -ge 0 -and $saut -lt 80) "premier écart $saut px ; $journal" | Out-Null
        }
        elseif ($fondu) {
            Verifier "$Quoi : fondu, fenêtre immobile" ($bouge.Count -eq 0) "$($places.Count) relevés, $($bouge.Count) déplacés ; $journal" | Out-Null
        }
    }
    Verifier "$Quoi : installateur sorti en moins de 5 s après Terminer" $sorti $journal | Out-Null
    if ($sorti) { Verifier "$Quoi : code de sortie 0" ($Processus.ExitCode -eq 0) "code $($Processus.ExitCode)" | Out-Null }
    else { try { $Processus.Kill($true) } catch { }; Verifier "$Quoi : installateur jamais tué par l'essai" $false 'tué en dernier recours' | Out-Null }
}

# « OK » d'une boîte de message native, par Win32 (UI Automation peut rendre ses boutons en
# Pane) : BM_CLICK au bouton, sinon WM_COMMAND IDOK à la boîte. Rend la condition.
function Presser-OK-Natif {
    param([Parameter(Mandatory)][System.Diagnostics.Process]$Processus, [Parameter(Mandatory)][string]$Quoi, [string]$Titre = $TitreInstallateur)
    $boite = [System.IntPtr]::Zero
    $limite = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $boite = [BreezyShotEssai09.Natif]::FindWindowW('#32770', $Titre)
        if ($boite -ne [System.IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $limite)
    if ($boite -eq [System.IntPtr]::Zero) { return Verifier $Quoi $false "boîte #32770 « $Titre » introuvable ; $(Decrire-Fenetre -Titre $Titre)" }
    $bouton = [BreezyShotEssai09.Natif]::FindWindowExW($boite, [System.IntPtr]::Zero, 'Button', 'OK')
    if ($bouton -eq [System.IntPtr]::Zero) { $bouton = [BreezyShotEssai09.Natif]::FindWindowExW($boite, [System.IntPtr]::Zero, 'Button', $null) }
    $essais = @()
    if ($bouton -ne [System.IntPtr]::Zero) {
        # BM_CLICK
        [void][BreezyShotEssai09.Natif]::PostMessageW($bouton, 0x00F5, [System.IntPtr]::Zero, [System.IntPtr]::Zero)
        $essais += 'BM_CLICK'
        if ($Processus.WaitForExit(3000)) { return Verifier $Quoi $true 'BM_CLICK au bouton' }
    }
    # WM_COMMAND, IDOK
    [void][BreezyShotEssai09.Natif]::PostMessageW($boite, 0x0111, [System.IntPtr]1, [System.IntPtr]::Zero)
    $essais += 'WM_COMMAND IDOK'
    if ($Processus.WaitForExit(3000)) { return Verifier $Quoi $true ($essais -join ' puis ') }
    return Verifier $Quoi $false "bouton trouvé : $($bouton -ne [System.IntPtr]::Zero) ; essayé : $($essais -join ', ') ; $(Decrire-Fenetre -Titre $Titre)"
}

# --- Registre ------------------------------------------------------------------------------

# Valeur d'une clé de HKCU, $null si la clé ou la valeur manque (jamais d'exception).
function Valeur-Registre {
    param([Parameter(Mandatory)][string]$Cle, [Parameter(Mandatory)][string]$Nom)
    $p = Get-ItemProperty -LiteralPath $Cle -ErrorAction SilentlyContinue
    if ($null -eq $p) { return $null }
    $prop = $p.PSObject.Properties[$Nom]
    if ($null -eq $prop) { return $null }
    return $prop.Value
}

# Clé de désinstallation d'essai, sous HKCU\Software\BreezyShot-essais\<nom du dossier>.
function Cle-Essai {
    $nom = Split-Path -Leaf $script:Essai.Dossier
    return "HKCU:\Software\BreezyShot-essais\$nom\Uninstall\BreezyShot"
}
# Démarrage avec Windows des Réglages, en essai (BREEZYSHOT_DOSSIER_ESSAI défini) : jamais
# la vraie clé Run.
$cleRunEssai = 'HKCU:\Software\BreezyShot\Essais\Run'
$cleRunReelle = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'

# Flux nommés (ADS) d'un fichier. Appeler avec @(…).
function Flux-Nommes {
    param([Parameter(Mandatory)][string]$Fichier)
    try { Get-Item -LiteralPath $Fichier -Stream * -ErrorAction Stop | Where-Object { $_.Stream -ne ':$DATA' } | ForEach-Object { $_.Stream } }
    catch { }
}

# --- Chemins d'installation d'essai --------------------------------------------------------
$version = Version-Programme
$racineEssai = $script:Essai.Dossier
$dossierInstalle = Join-Path $racineEssai 'programmes/BreezyShot'
$exeInstalle = Join-Path $dossierInstalle 'breezyshot.exe'
$raccourci = Join-Path $racineEssai 'menu/BreezyShot.lnk'
$telecharge = Join-Path $racineEssai "telechargement/BreezyShot-$version-essai.exe"
$tetu = $null

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
    $runReelAvant = Valeur-Registre -Cle $cleRunReelle -Nom 'BreezyShot'

    # 2. Lancement : l'accueil.
    $m = Marque
    $inst = Lancer-Telecharge -Fichier $telecharge
    Verifier 'plan: installer' (Attendre-Installation -Texte 'plan: installer' -Depuis $m -DelaiS 25) | Out-Null
    Verifier 'installateur fenetre (hwnd)' (Attendre-Installation -Texte 'installateur fenetre ' -Depuis $m) | Out-Null
    # La fenêtre est montrée une seule fois : quand la page est prête, ou vide au bout du
    # délai de 2 s si la page tarde (machine lente). La raison lue est dite.
    $null = Attendre-Page -Depuis $m -Quoi 'premier lancement'
    $montrees = @(Nouvelles-Lignes -Depuis $m | Where-Object { $_.Contains('installateur montre: ') })
    $raison = if ($montrees.Count -ge 1) { ($montrees[0] -replace '^.*installateur montre: ', '') } else { '' }
    Verifier 'installateur montré une seule fois (page prête, ou délai de 2 s si elle tarde)' ($montrees.Count -eq 1 -and $raison -in @('page prete', 'delai')) "$($montrees.Count) fois ; raison : $raison" | Out-Null
    Verifier 'fenêtre « Installer BreezyShot » présente' ($null -ne (Attendre-Fenetre)) (Decrire-Fenetre) | Out-Null
    Verifier 'aucune app BreezyShot en cours' (@(Processus-App).Count -eq 0) | Out-Null

    # 3. « Installer » : étapes dans l'ordre, installation finie.
    $m = Marque
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    $null = Presser -Nom 'Installer' -Quoi 'bouton « Installer » pressé'
    $finie = Attendre-Installation -Texte "installation finie $version" -Depuis $m -DelaiS 20
    $chrono.Stop()
    Verifier "installation finie en moins de 20 s" $finie "$([math]::Round($chrono.Elapsed.TotalSeconds,1)) s" | Out-Null
    Noter-Mesure -Nom "durée d'installation" -Valeur ([math]::Round($chrono.Elapsed.TotalSeconds, 1)) -Unite 's'
    $nouvelles = @(Nouvelles-Lignes -Depuis $m)
    $rangs = foreach ($etape in @('place', 'arret', 'copie', 'verification', 'raccourci', 'inscription', 'lancement')) {
        $r = -1
        for ($i = 0; $i -lt $nouvelles.Count; $i++) { if ($nouvelles[$i].Contains("etape ${etape}:")) { $r = $i; break } }
        Verifier "étape $etape journalisée" ($r -ge 0) | Out-Null
        $r
    }
    $rangs = @($rangs)
    $ordre = $true
    for ($i = 1; $i -lt $rangs.Count; $i++) { if ($rangs[$i] -le $rangs[$i - 1]) { $ordre = $false } }
    Verifier 'étapes dans l''ordre' $ordre ($rangs -join ',') | Out-Null

    # 4. Exe installé identique au téléchargé, sans marque du Web.
    $memes = (Test-Path -LiteralPath $exeInstalle) -and (Test-Path -LiteralPath $telecharge) -and
        ((Get-FileHash -LiteralPath $exeInstalle -Algorithm SHA256).Hash -eq (Get-FileHash -LiteralPath $telecharge -Algorithm SHA256).Hash)
    Verifier 'exe installé identique au téléchargé (SHA-256)' $memes | Out-Null
    $flux = @(Flux-Nommes -Fichier $exeInstalle)
    Verifier 'exe installé sans marque du Web (Zone.Identifier)' ($flux.Count -eq 0) ($flux -join ',') | Out-Null

    # 5. Raccourci : cible = exe installé.
    $cibleRaccourci = $null
    if (Test-Path -LiteralPath $raccourci) {
        $shell = New-Object -ComObject WScript.Shell
        $cibleRaccourci = $shell.CreateShortcut($raccourci).TargetPath
    }
    Verifier 'raccourci vers l''exe installé' ($cibleRaccourci -eq $exeInstalle) "$cibleRaccourci" | Out-Null

    # 6. Clé d'essai : les onze valeurs attendues.
    $cle = Cle-Essai
    $commande = "`"$exeInstalle`" --desinstaller"
    $attendues = [ordered]@{
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
    $ecarts = @()
    if (-not (Test-Path -LiteralPath $cle)) { $ecarts += 'clé absente' }
    else {
        foreach ($nom in $attendues.Keys) {
            $lu = Valeur-Registre -Cle $cle -Nom $nom
            if ($null -eq $lu) { $ecarts += "$nom absente" }
            elseif ("$lu" -ne "$($attendues[$nom])") { $ecarts += "$nom = $lu (attendu $($attendues[$nom]))" }
        }
        $taille = Valeur-Registre -Cle $cle -Nom 'EstimatedSize'
        if ($null -eq $taille) { $ecarts += 'EstimatedSize absente' }
        elseif ([int64]$taille -le 0) { $ecarts += "EstimatedSize = $taille" }
    }
    Verifier 'clé de désinstallation : onze valeurs exactes' ($ecarts.Count -eq 0) ($ecarts -join ' ; ') | Out-Null

    # 7. L'app installée tourne, depuis l'exe installé, et est prête.
    $procApp = @(Processus-App)
    $bonChemin = $false
    foreach ($q in $procApp) { try { if ($q.Path -eq $exeInstalle) { $bonChemin = $true } } catch { } }
    Verifier 'app installée en cours, depuis l''exe installé' ($procApp.Count -ge 1 -and $bonChemin) | Out-Null
    Verifier 'app installée : journal « pret »' (Attendre-Ligne -Texte ' pret' -FinDeLigne -DelaiS 15) | Out-Null

    # 8. « BreezyShot est prêt » visible, et ses deux boutons.
    Verifier-Texte -Texte 'BreezyShot est prêt' -Quoi 'écran « BreezyShot est prêt »' | Out-Null
    Verifier-Bouton-Utilisable -Nom 'Terminer' -Quoi 'bouton « Terminer » visible et actif' | Out-Null

    # 9. « Essayer une capture » : l'app lance une capture de zone ; Échap annule.
    $null = Presser -Nom 'Essayer une capture' -Quoi 'bouton « Essayer une capture » pressé'
    Verifier 'app : « --capturer recu: zone »' (Attendre-Ligne -Texte '--capturer recu: zone' -DelaiS 10) | Out-Null
    Verifier 'app : capture de zone lancée' (Attendre-Ligne -Texte 'DemandeCapture zone' -DelaiS 10) | Out-Null
    Verifier 'app : sélection montrée' (Attendre-Ligne -Texte 'selection montree' -DelaiS 10) | Out-Null
    Verifier 'app : jamais « selection sans premier plan »' ((Compter-Lignes -Texte 'selection sans premier plan') -eq 0) | Out-Null
    [BreezyShotEssai09.Natif]::keybd_event(0x1B, 0, 0, [UIntPtr]::Zero)
    [BreezyShotEssai09.Natif]::keybd_event(0x1B, 0, 2, [UIntPtr]::Zero)
    Verifier 'app : sélection annulée (Échap)' (Attendre-Ligne -Texte 'SelectionAnnulee' -DelaiS 10) | Out-Null

    # 10. « Terminer » : envol depuis la place réelle, ou fondu si Windows coupe les
    # animations (le journal dit lequel) ; l'installateur sort de lui-même.
    Verifier-Envol -Processus $inst -Quoi 'Terminer'
    Verifier 'app toujours en cours après Terminer' (@(Processus-App).Count -ge 1) | Out-Null

    Quitter-App | Out-Null
    Start-Sleep -Milliseconds 500

    # 11. Relance du téléchargé, app ouverte : réinstallation, arrêt propre de l'instance.
    $n = Compter-Lignes -Texte ' pret' -FinDeLigne
    Demarrer-App -Exe $exeInstalle | Out-Null
    Verifier 'app relancée (pret)' (Attendre-Ligne -Texte ' pret' -FinDeLigne -DelaiS 15 -Fois ($n + 1)) | Out-Null
    $m = Marque
    # Sous BREEZYSHOT_ESSAI_ANIMATIONS=1 : l'envol joue même si Windows coupe les animations.
    $inst2 = Lancer-Telecharge -Fichier $telecharge -Variables @{ BREEZYSHOT_ESSAI_ANIMATIONS = '1' }
    Verifier 'plan: reinstaller' (Attendre-Installation -Texte 'plan: reinstaller' -Depuis $m -DelaiS 20) | Out-Null
    $null = Attendre-Page -Depuis $m -Quoi 'réinstallation'
    $null = Presser -Nom 'Installer' -Quoi 'bouton « Installer » (réinstallation) pressé'
    Verifier 'réinstallation : « etape arret: instance arretee »' (Attendre-Installation -Texte 'etape arret: instance arretee' -Depuis $m -DelaiS 20) | Out-Null
    Verifier 'réinstallation finie' (Attendre-Installation -Texte "installation finie $version" -Depuis $m -DelaiS 20) | Out-Null
    Verifier-Envol -Processus $inst2 -Quoi 'réinstallation, envol forcé' -Force
    Quitter-App | Out-Null
    Start-Sleep -Milliseconds 500

    # 12. Mise à jour, puis version plus récente installée (exe non réécrit).
    Set-ItemProperty -LiteralPath $cle -Name 'DisplayVersion' -Value '0.0.1'
    $m = Marque
    $instMaj = Lancer-Telecharge -Fichier $telecharge
    Verifier 'plan: mettre a jour depuis 0.0.1' (Attendre-Installation -Texte 'plan: mettre a jour depuis 0.0.1' -Depuis $m -DelaiS 20) | Out-Null
    $null = Attendre-Page -Depuis $m -Quoi 'mise à jour non lancée'
    Fermer-Installateur -Processus $instMaj -Bouton 'Fermer' -Quoi 'mise à jour non lancée' | Out-Null
    Set-ItemProperty -LiteralPath $cle -Name 'DisplayVersion' -Value '99.0.0'
    $ecritAvant = (Get-Item -LiteralPath $exeInstalle).LastWriteTimeUtc
    $m = Marque
    $instRecent = Lancer-Telecharge -Fichier $telecharge
    Verifier 'plan: plus recente installee 99.0.0' (Attendre-Installation -Texte 'plan: plus recente installee 99.0.0' -Depuis $m -DelaiS 20) | Out-Null
    $null = Attendre-Page -Depuis $m -Quoi 'version plus récente'
    Verifier-Texte -Texte 'BreezyShot est prêt' -Quoi 'version plus récente : « prêt » direct' | Out-Null
    Start-Sleep -Milliseconds 500
    Verifier 'version plus récente : exe non réécrit' ((Get-Item -LiteralPath $exeInstalle).LastWriteTimeUtc -eq $ecritAvant) | Out-Null
    Fermer-Installateur -Processus $instRecent -Bouton 'Terminer' -Quoi 'version plus récente' | Out-Null
    Quitter-App | Out-Null
    Set-ItemProperty -LiteralPath $cle -Name 'DisplayVersion' -Value $version
    Start-Sleep -Milliseconds 500

    # 13. Erreur d'écriture puis Réessayer : l'essai tient l'exe installé ouvert sans partage.
    Verifier 'aucune app avant l''erreur d''écriture' (@(Processus-App).Count -eq 0) | Out-Null
    $tenu = [System.IO.FileStream]::new($exeInstalle, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::None)
    $m = Marque
    $instErr = Lancer-Telecharge -Fichier $telecharge
    $null = Attendre-Page -Depuis $m -Quoi 'erreur d''écriture'
    $null = Presser -Nom 'Installer' -Quoi 'erreur d''écriture : « Installer » pressé' -DelaiS 15
    Verifier 'erreur : « installation erreur: ecriture »' (Attendre-Installation -Texte 'installation erreur: ecriture' -Depuis $m -DelaiS 25) | Out-Null
    Verifier 'erreur : diagnostic de fichier au journal' ($null -ne (Ligne-Installation -Texte 'erreur fichier: ' -Depuis $m)) | Out-Null
    Verifier-Texte -Texte "Impossible d'écrire dans le dossier de BreezyShot. Réessayez dans un instant." -Quoi 'erreur d''écriture : cause affichée' | Out-Null
    Verifier-Bouton-Utilisable -Nom 'Réessayer' -Quoi 'erreur d''écriture : « Réessayer » visible et actif' | Out-Null
    $tenu.Dispose()
    $m = Marque
    $null = Presser -Nom 'Réessayer' -Quoi 'erreur d''écriture : « Réessayer » pressé'
    Verifier 'Réessayer : installation finie' (Attendre-Installation -Texte "installation finie $version" -Depuis $m -DelaiS 20) | Out-Null
    Fermer-Installateur -Processus $instErr -Bouton 'Terminer' -Quoi 'après Réessayer' | Out-Null
    Quitter-App | Out-Null
    Start-Sleep -Milliseconds 500

    # 14. « Ouvert » : une app qui ne se ferme pas n'est jamais tuée ; l'erreur le dit, et
    # Réessayer marche une fois qu'elle est partie.
    Verifier 'aucune app avant « ouvert »' (@(Processus-App).Count -eq 0) | Out-Null
    $tetu = Demarrer-Tetu
    $m = Marque
    $instOuvert = Lancer-Telecharge -Fichier $telecharge
    $null = Attendre-Page -Depuis $m -Quoi '« ouvert »'
    $null = Presser -Nom 'Installer' -Quoi '« ouvert » : « Installer » pressé' -DelaiS 15
    Verifier '« ouvert » : « etape arret: instance toujours la »' (Attendre-Installation -Texte 'etape arret: instance toujours la' -Depuis $m -DelaiS 25) | Out-Null
    Verifier '« ouvert » : « installation erreur: ouvert »' (Attendre-Installation -Texte 'installation erreur: ouvert' -Depuis $m -DelaiS 5) | Out-Null
    Verifier-Texte -Texte 'BreezyShot est encore ouvert. Quittez-le, puis réessayez.' -Quoi '« ouvert » : cause affichée' | Out-Null
    Verifier-Bouton-Utilisable -Nom 'Réessayer' -Quoi '« ouvert » : « Réessayer » visible et actif' | Out-Null
    Verifier '« ouvert » : l''app têtue n''a pas été tuée' (-not $tetu.HasExited) | Out-Null
    Arreter-Tetu -Processus $tetu
    $m = Marque
    $null = Presser -Nom 'Réessayer' -Quoi '« ouvert » : « Réessayer » pressé'
    Verifier '« ouvert » : installation finie après Réessayer' (Attendre-Installation -Texte "installation finie $version" -Depuis $m -DelaiS 25) | Out-Null
    Fermer-Installateur -Processus $instOuvert -Bouton 'Terminer' -Quoi '« ouvert »' | Out-Null
    Quitter-App | Out-Null
    Start-Sleep -Milliseconds 500

    # 15. Erreur de place : place libre simulée à 1 octet.
    $m = Marque
    $instPlace = Lancer-Telecharge -Fichier $telecharge -Variables @{ BREEZYSHOT_ESSAI_LIBRE_OCTETS = '1' }
    $null = Attendre-Page -Depuis $m -Quoi 'place'
    $null = Presser -Nom 'Installer' -Quoi 'place : « Installer » pressé' -DelaiS 15
    Verifier 'erreur de place journalisée' (Attendre-Installation -Texte 'installation erreur: place' -Depuis $m -DelaiS 15) | Out-Null
    $ligPlace = Ligne-Installation -Texte 'etape place: libre' -Depuis $m
    Verifier 'place : « libre 0 Mo besoin N Mo »' ($null -ne $ligPlace -and $ligPlace -match 'etape place: libre 0 Mo besoin [1-9]\d* Mo$') "$ligPlace" | Out-Null
    Verifier-Texte -Texte 'Il manque de la place sur le disque' -Debut -Quoi 'place : « Il manque de la place… » affiché' | Out-Null
    Verifier-Bouton-Utilisable -Nom 'Réessayer' -Quoi 'place : « Réessayer » visible et actif' | Out-Null
    Fermer-Installateur -Processus $instPlace -Bouton 'Fermer' -Quoi 'place' | Out-Null

    # 16. Second installateur : refusé, le premier est ramené au premier plan et reste.
    $m = Marque
    $premier = Lancer-Telecharge -Fichier $telecharge
    $null = Attendre-Fenetre
    $null = Attendre-Page -Depuis $m -Quoi 'premier installateur'
    $m = Marque
    $second = Lancer-Telecharge -Fichier $telecharge
    $secondSorti = $second.WaitForExit(8000)
    Verifier 'second installateur sorti de lui-même' $secondSorti | Out-Null
    if (-not $secondSorti) { try { $second.Kill($true) } catch { } }
    Verifier 'second installateur : code 0' ($secondSorti -and $second.ExitCode -eq 0) | Out-Null
    Verifier 'second installateur : premier ramené au premier plan' (Attendre-Installation -Texte 'seconde installation refusee: premier plan rendu' -Depuis $m -DelaiS 3) | Out-Null
    Verifier 'premier installateur toujours là' (-not $premier.HasExited) | Out-Null
    Fermer-Installateur -Processus $premier -Bouton 'Fermer' -Quoi 'premier installateur' | Out-Null

    # 17. « Fermer » pendant l'installation. a) Réussite : la fenêtre disparaît, l'installation
    # finit, l'app démarre, l'installateur sort avec 0. b) Erreur : il sort avec 1, sans
    # jamais rester caché (le verrou de l'installateur est rendu).
    $tetu = Demarrer-Tetu
    $m = Marque
    $instA = Lancer-Telecharge -Fichier $telecharge
    $null = Attendre-Page -Depuis $m -Quoi 'fermer pendant (réussite)'
    $null = Presser -Nom 'Installer' -Quoi 'fermer pendant (réussite) : « Installer » pressé' -DelaiS 15
    Verifier 'fermer pendant (réussite) : étape arrêt en cours' (Attendre-Installation -Texte 'etape place: libre' -Depuis $m -DelaiS 10) | Out-Null
    $null = Presser -Nom 'Fermer' -Quoi 'fermer pendant (réussite) : « Fermer » pressé'
    Verifier 'fermer pendant (réussite) : fenêtre cachée' (Attendre-Sans-Fenetre -DelaiS 3) | Out-Null
    Verifier 'fermer pendant (réussite) : installateur toujours là' (-not $instA.HasExited) | Out-Null
    Arreter-Tetu -Processus $tetu
    $sortiA = $instA.WaitForExit(30000)
    if (-not $sortiA) { try { $instA.Kill($true) } catch { } }
    Verifier 'fermer pendant (réussite) : sorti de lui-même, code 0' ($sortiA -and $instA.ExitCode -eq 0) $(if ($sortiA) { "code $($instA.ExitCode)" } else { 'tué en dernier recours' }) | Out-Null
    Verifier 'fermer pendant (réussite) : « sortie 0 » au journal' ($null -ne (Ligne-Installation -Texte 'installation fermee pendant les etapes: sortie 0' -Depuis $m)) | Out-Null
    Verifier 'fermer pendant (réussite) : app démarrée' (@(Processus-App).Count -ge 1) | Out-Null
    Quitter-App | Out-Null
    Start-Sleep -Milliseconds 500

    $tetu = Demarrer-Tetu
    $m = Marque
    $instB = Lancer-Telecharge -Fichier $telecharge
    $null = Attendre-Page -Depuis $m -Quoi 'fermer pendant (erreur)'
    $null = Presser -Nom 'Installer' -Quoi 'fermer pendant (erreur) : « Installer » pressé' -DelaiS 15
    Verifier 'fermer pendant (erreur) : étape arrêt en cours' (Attendre-Installation -Texte 'etape place: libre' -Depuis $m -DelaiS 10) | Out-Null
    $null = Presser -Nom 'Fermer' -Quoi 'fermer pendant (erreur) : « Fermer » pressé'
    Verifier 'fermer pendant (erreur) : fenêtre cachée' (Attendre-Sans-Fenetre -DelaiS 3) | Out-Null
    $sortiB = $instB.WaitForExit(25000)
    if (-not $sortiB) { try { $instB.Kill($true) } catch { } }
    Verifier 'fermer pendant (erreur) : sorti de lui-même, code 1' ($sortiB -and $instB.ExitCode -eq 1) $(if ($sortiB) { "code $($instB.ExitCode)" } else { 'resté caché : tué en dernier recours' }) | Out-Null
    Verifier 'fermer pendant (erreur) : « installation erreur: ouvert »' ($null -ne (Ligne-Installation -Texte 'installation erreur: ouvert' -Depuis $m)) | Out-Null
    Verifier 'fermer pendant (erreur) : « sortie 1 » au journal' ($null -ne (Ligne-Installation -Texte 'installation fermee pendant les etapes: sortie 1' -Depuis $m)) | Out-Null
    Verifier 'fermer pendant (erreur) : l''app têtue n''a pas été tuée' (-not $tetu.HasExited) | Out-Null
    Arreter-Tetu -Processus $tetu
    Verifier 'fermer pendant : aucune app restante' (@(Processus-App).Count -eq 0) | Out-Null

    # 18. Désinstallation, app ouverte, données posées avant : retire ce qui est à nous,
    # garde le reste (réglages, Récents, captures).
    $n = Compter-Lignes -Texte ' pret' -FinDeLigne
    Demarrer-App -Exe $exeInstalle | Out-Null
    Verifier 'app ouverte avant la désinstallation' (Attendre-Ligne -Texte ' pret' -FinDeLigne -DelaiS 15 -Fois ($n + 1)) | Out-Null
    # Démarrage avec Windows (Réglages, clé d'essai) : la désinstallation doit le retirer.
    New-Item -Path $cleRunEssai -Force | Out-Null
    New-ItemProperty -LiteralPath $cleRunEssai -Name 'BreezyShot' -Value "`"$exeInstalle`"" -PropertyType String -Force | Out-Null
    $reglages = Join-Path $racineEssai 'reglages/reglages.json'
    $recent = Join-Path $racineEssai 'local/Recents/temoin.png'
    $capture = Join-Path $racineEssai 'images/BreezyShot temoin.png'
    foreach ($f in @($reglages, $recent, $capture)) { New-Item -ItemType Directory -Force -Path (Split-Path $f) | Out-Null }
    Set-Content -LiteralPath $reglages -Value '{"version":1}' -Encoding utf8
    Set-Content -LiteralPath $recent -Value 'temoin' -Encoding utf8
    Set-Content -LiteralPath $capture -Value 'capture' -Encoding utf8
    $empreintes = @{}
    foreach ($f in @($reglages, $recent, $capture)) { $empreintes[$f] = (Get-FileHash -LiteralPath $f).Hash }
    $m = Marque
    $desinst = Lancer-Telecharge -Fichier $exeInstalle -Arguments @('--desinstaller')
    $desinstSorti = $desinst.WaitForExit(20000)
    if (-not $desinstSorti) { try { $desinst.Kill($true) } catch { } }
    Verifier 'désinstallation sortie d''elle-même en moins de 20 s' $desinstSorti | Out-Null
    Verifier 'désinstallation : code 0' ($desinstSorti -and $desinst.ExitCode -eq 0) | Out-Null
    foreach ($ligne in @('desinstallation: instance arretee', 'desinstallation exe: deplace', 'desinstallation raccourci: retire',
            'desinstallation inscription: retiree', 'desinstallation demarrage: retire', 'desinstallation finie: complete')) {
        Verifier "désinstallation : « $ligne »" ($null -ne (Ligne-Installation -Texte $ligne -Depuis $m)) | Out-Null
    }
    Verifier 'désinstallation : app arrêtée' (@(Processus-App).Count -eq 0) | Out-Null
    Verifier 'clé de désinstallation retirée' (-not (Test-Path -LiteralPath $cle)) | Out-Null
    Verifier 'raccourci retiré' (-not (Test-Path -LiteralPath $raccourci)) | Out-Null
    Verifier 'exe installé retiré (déplacé)' (-not (Test-Path -LiteralPath $exeInstalle)) | Out-Null
    $deplace = Join-Path $racineEssai "temp/BreezyShot-desinstalle-$($desinst.Id).exe"
    Verifier 'exe déplacé dans le dossier temporaire' (Test-Path -LiteralPath $deplace) | Out-Null
    Verifier 'dossier installé retiré' (-not (Test-Path -LiteralPath $dossierInstalle)) | Out-Null
    Verifier 'démarrage avec Windows (essai) retiré' ($null -eq (Valeur-Registre -Cle $cleRunEssai -Nom 'BreezyShot')) | Out-Null
    foreach ($f in @($reglages, $recent, $capture)) {
        $intact = (Test-Path -LiteralPath $f) -and ((Get-FileHash -LiteralPath $f).Hash -eq $empreintes[$f])
        Verifier "gardé intact : $(Split-Path -Leaf $f)" $intact | Out-Null
    }

    # 19. WebView2 absent simulé : message, code 1, rien d'installé.
    $m = Marque
    $instSans = Lancer-Telecharge -Fichier $telecharge -Variables @{ BREEZYSHOT_ESSAI_SANS_WEBVIEW2 = '1' }
    Verifier 'webview2 absent journalisé' (Attendre-Installation -Texte 'installateur: webview2 absent' -Depuis $m -DelaiS 15) | Out-Null
    Verifier-Texte -Texte 'BreezyShot a besoin de Microsoft Edge WebView2, absent de ce PC.' -Quoi 'webview2 absent : message affiché' | Out-Null
    $null = Presser-OK-Natif -Processus $instSans -Quoi 'webview2 absent : « OK » pressé'
    $sansSorti = $instSans.WaitForExit(8000)
    if (-not $sansSorti) { try { $instSans.Kill($true) } catch { } }
    Verifier 'webview2 absent : sorti de lui-même, code 1' ($sansSorti -and $instSans.ExitCode -eq 1) | Out-Null
    Verifier 'webview2 absent : rien réinstallé' (-not (Test-Path -LiteralPath $exeInstalle)) | Out-Null

    # 20. Fin : l'état réel n'a pas bougé, aucun processus restant.
    $cleReelleApres = Test-Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\BreezyShot'
    $dossierReelApres = Test-Path (Join-Path $dossierReelProgrammes 'Programs/BreezyShot')
    $runReelApres = Valeur-Registre -Cle $cleRunReelle -Nom 'BreezyShot'
    Verifier 'clé de désinstallation réelle inchangée' ($cleReelleAvant -eq $cleReelleApres) | Out-Null
    Verifier 'dossier installé réel inchangé' ($dossierReelAvant -eq $dossierReelApres) | Out-Null
    Verifier 'valeur Run réelle inchangée' ("$runReelAvant" -eq "$runReelApres") | Out-Null
    Verifier 'aucun processus BreezyShot restant' (@(Processus-App).Count -eq 0) | Out-Null
    Noter-Mesure -Nom 'lancement le plus lent de l''installateur' -Valeur $script:DemarrageMaxS -Unite 's'
    Noter-Mesure -Nom 'taille du téléchargé' -Valeur ([math]::Round((Get-Item -LiteralPath $telecharge).Length / 1MB, 1)) -Unite 'Mo'
}
catch {
    Write-Host "ERREUR : $($_.Exception.Message)"
    Write-Host $_.ScriptStackTrace
    Verifier 'essai 09 sans exception' $false $_.Exception.Message | Out-Null
}
finally {
    if ($null -ne $tetu) { Arreter-Tetu -Processus $tetu }
    # Un installateur resté (après une exception) est fermé aussi, et c'est un échec.
    $restants = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -like 'BreezyShot-*-essai' })
    foreach ($p in $restants) { try { $p.Kill($true); $null = $p.WaitForExit(5000) } catch { } }
    Verifier 'aucun installateur resté à la fin' ($restants.Count -eq 0) "$($restants.Count) fermé(s) de force par le nettoyage" | Out-Null
    # Nettoyage des clés d'essai (jamais les vraies) : la clé de désinstallation d'essai et
    # la valeur de démarrage posée dans la clé d'essai des Réglages.
    $nom = Split-Path -Leaf $script:Essai.Dossier
    Remove-Item -LiteralPath "HKCU:\Software\BreezyShot-essais\$nom" -Recurse -Force -ErrorAction SilentlyContinue
    Remove-ItemProperty -LiteralPath $cleRunEssai -Name 'BreezyShot' -ErrorAction SilentlyContinue
    # La clé Run d'essai, laissée vide, est retirée (jamais si elle garde autre chose).
    $k = Get-Item -LiteralPath $cleRunEssai -ErrorAction SilentlyContinue
    if ($null -ne $k -and $k.ValueCount -eq 0 -and $k.SubKeyCount -eq 0) {
        Remove-Item -LiteralPath $cleRunEssai -Force -ErrorAction SilentlyContinue
    }
    Terminer-Essai
}
