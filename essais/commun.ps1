# Aides communes aux essais BreezyShot. À charger au début de chaque essai :
#   . "$PSScriptRoot/commun.ps1"
#   Initialiser-Essai -Nom '01-socle' -Exe $Exe -Sortie $Sortie
#   try { ... Verifier ... } catch { ... } finally { Terminer-Essai }
#
# Chaque essai tourne dans son propre dossier temporaire, désigné à l'app par la variable
# BREEZYSHOT_DOSSIER_ESSAI : il ne touche jamais aux vrais dossiers de l'utilisateur.
# PowerShell 7 (pwsh) ; marche aussi sous Linux avec le binaire de développement.

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:Essai = $null
$script:Verifications = [System.Collections.Generic.List[object]]::new()
$script:Mesures = [System.Collections.Generic.List[object]]::new()

# Nom du processus de l'app (breezyshot.exe sous Windows).
$script:NomProcessus = 'breezyshot'

function Initialiser-Essai {
    param(
        [Parameter(Mandatory)][string]$Nom,
        [Parameter(Mandatory)][string]$Exe,
        [Parameter(Mandatory)][string]$Sortie
    )
    $exeComplet = (Resolve-Path -LiteralPath $Exe).Path
    New-Item -ItemType Directory -Force -Path $Sortie | Out-Null
    $script:Essai = [pscustomobject]@{
        Nom     = $Nom
        Exe     = $exeComplet
        Sortie  = (Resolve-Path -LiteralPath $Sortie).Path
        Dossier = Nouveau-DossierEssai -Nom $Nom
    }
    Write-Host "=== Essai $Nom ==="
    Write-Host "app : $exeComplet"
    Write-Host "dossier d'essai : $($script:Essai.Dossier)"
    # Un essai part toujours d'une machine sans BreezyShot en cours.
    Arreter-Tout
    return $script:Essai
}

function Nouveau-DossierEssai {
    param([string]$Nom = 'essai')
    $chemin = Join-Path ([System.IO.Path]::GetTempPath()) ("breezyshot-$Nom-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
    New-Item -ItemType Directory -Force -Path $chemin | Out-Null
    return $chemin
}

# Lance l'app avec BREEZYSHOT_DOSSIER_ESSAI pointé sur $Dossier. Rend le processus.
function Demarrer-App {
    param(
        [string]$Exe = $script:Essai.Exe,
        [string]$Dossier = $script:Essai.Dossier,
        [string[]]$Arguments = @()
    )
    $ancien = $env:BREEZYSHOT_DOSSIER_ESSAI
    $env:BREEZYSHOT_DOSSIER_ESSAI = $Dossier
    try {
        $options = @{ FilePath = $Exe; PassThru = $true }
        if ($Arguments.Count -gt 0) { $options.ArgumentList = $Arguments }
        $p = Start-Process @options
        # Lire Handle tout de suite garde le code de sortie disponible plus tard.
        $null = $p.Handle
        return $p
    }
    finally {
        $env:BREEZYSHOT_DOSSIER_ESSAI = $ancien
    }
}

# Lance l'app et attend qu'elle sorte. Rend Fini (sortie avant le délai), Code, DureeS,
# Processus. Au-delà du délai, le processus est tué, sauf avec -LaisserVivant (l'essai
# compte alors les processus avant de nettoyer lui-même).
function Lancer-Et-Attendre {
    param(
        [string[]]$Arguments = @(),
        [double]$DelaiS = 10,
        [switch]$LaisserVivant,
        [string]$Exe = $script:Essai.Exe,
        [string]$Dossier = $script:Essai.Dossier
    )
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    $p = Demarrer-App -Exe $Exe -Dossier $Dossier -Arguments $Arguments
    $fini = $p.WaitForExit([int]($DelaiS * 1000))
    $chrono.Stop()
    $code = $null
    if ($fini) {
        $p.WaitForExit()
        $code = $p.ExitCode
    }
    elseif (-not $LaisserVivant) {
        try { $p.Kill() } catch { }
    }
    return [pscustomobject]@{ Fini = $fini; Code = $code; DureeS = [math]::Round($chrono.Elapsed.TotalSeconds, 2); Processus = $p }
}

# Demande à l'instance en cours de se fermer (--quitter).
function Quitter-App {
    param([double]$DelaiS = 10, [string]$Dossier = $script:Essai.Dossier)
    return Lancer-Et-Attendre -Arguments @('--quitter') -DelaiS $DelaiS -Dossier $Dossier
}

function Chemin-Journal {
    param([string]$Dossier = $script:Essai.Dossier)
    return Join-Path $Dossier 'local/Journal/breezyshot.log'
}

# Lit le journal sans gêner l'app qui l'a ouvert en écriture.
function Lire-Journal {
    param([string]$Dossier = $script:Essai.Dossier)
    $chemin = Chemin-Journal -Dossier $Dossier
    if (-not (Test-Path -LiteralPath $chemin)) { return '' }
    try {
        $flux = [System.IO.FileStream]::new($chemin, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]'ReadWrite, Delete')
        try {
            $lecteur = [System.IO.StreamReader]::new($flux, [System.Text.Encoding]::UTF8)
            return $lecteur.ReadToEnd()
        }
        finally { $flux.Dispose() }
    }
    catch { return '' }
}

# Nombre de lignes du journal qui contiennent $Texte (texte exact, pas une expression).
# Avec -FinDeLigne, la ligne doit se terminer par $Texte.
function Compter-Lignes {
    param(
        [Parameter(Mandatory)][string]$Texte,
        [switch]$FinDeLigne,
        [string]$Dossier = $script:Essai.Dossier
    )
    $n = 0
    foreach ($ligne in ((Lire-Journal -Dossier $Dossier) -split "`n")) {
        $ligne = $ligne.TrimEnd("`r")
        $trouve = if ($FinDeLigne) { $ligne.EndsWith($Texte) } else { $ligne.Contains($Texte) }
        if ($trouve) { $n++ }
    }
    return $n
}

# Attend qu'au moins $Fois lignes contiennent $Texte. Rend $true si c'est arrivé à temps.
function Attendre-Ligne {
    param(
        [Parameter(Mandatory)][string]$Texte,
        [double]$DelaiS = 10,
        [int]$Fois = 1,
        [switch]$FinDeLigne,
        [string]$Dossier = $script:Essai.Dossier
    )
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        if ((Compter-Lignes -Texte $Texte -FinDeLigne:$FinDeLigne -Dossier $Dossier) -ge $Fois) { return $true }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $limite)
    return $false
}

# Les processus BreezyShot en cours.
function Processus-App {
    return @(Get-Process -Name $script:NomProcessus -ErrorAction SilentlyContinue)
}

# Mémoire privée (Mo) du processus et de tous ses descendants (processus du moteur web).
function Memoire-Privee-Mo {
    param([Parameter(Mandatory)][System.Diagnostics.Process]$Processus)
    $tous = @(Get-Process -ErrorAction SilentlyContinue)
    $enfants = @{}
    foreach ($q in $tous) {
        try { $parent = $q.Parent } catch { $parent = $null }
        if ($null -ne $parent) {
            if (-not $enfants.ContainsKey($parent.Id)) { $enfants[$parent.Id] = [System.Collections.Generic.List[object]]::new() }
            $enfants[$parent.Id].Add($q)
        }
    }
    $Processus.Refresh()
    $total = [long]$Processus.PrivateMemorySize64
    $a_voir = [System.Collections.Generic.Queue[int]]::new()
    $a_voir.Enqueue($Processus.Id)
    $vus = @{ $Processus.Id = $true }
    while ($a_voir.Count -gt 0) {
        $id = $a_voir.Dequeue()
        if (-not $enfants.ContainsKey($id)) { continue }
        foreach ($e in $enfants[$id]) {
            if ($vus.ContainsKey($e.Id)) { continue }
            $vus[$e.Id] = $true
            $total += [long]$e.PrivateMemorySize64
            $a_voir.Enqueue($e.Id)
        }
    }
    return [math]::Round($total / 1MB, 1)
}

# Ferme de force tout BreezyShot restant (nettoyage, jamais une vérification).
function Arreter-Tout {
    foreach ($p in (Processus-App)) {
        try { $p.Kill(); $null = $p.WaitForExit(5000) } catch { }
    }
}

# Note une vérification. Rend la condition, pour enchaîner si besoin.
function Verifier {
    param(
        [Parameter(Mandatory)][string]$Nom,
        [Parameter(Mandatory)][bool]$Condition,
        [string]$Detail = ''
    )
    $script:Verifications.Add([pscustomobject]@{ Nom = $Nom; Reussi = $Condition; Detail = $Detail })
    $etiquette = if ($Condition) { '[ok]    ' } else { '[ECHEC] ' }
    $suite = if ($Detail) { " ($Detail)" } else { '' }
    Write-Host "$etiquette$Nom$suite"
    return $Condition
}

# Note une mesure pour le résumé (mémoire, chrono…).
function Noter-Mesure {
    param([Parameter(Mandatory)][string]$Nom, [Parameter(Mandatory)]$Valeur, [string]$Unite = '')
    $script:Mesures.Add([pscustomobject]@{ Nom = $Nom; Valeur = $Valeur; Unite = $Unite })
    Write-Host "[mesure] $Nom : $Valeur $Unite"
}

# Copie dans $Cible les journaux de l'app trouvés sous $Dossier : celui de l'essai
# (`local/Journal`), et ceux de ses cas (`<cas>/local/Journal`, rangés sous `<cas>`).
function Ranger-Journaux([string]$Dossier, [string]$Cible) {
    $journaux = @(Get-ChildItem -LiteralPath $Dossier -Directory -Recurse -Depth 3 -Filter 'Journal' -ErrorAction SilentlyContinue |
        Where-Object { $_.Parent.Name -eq 'local' })
    foreach ($j in $journaux) {
        $cas = [System.IO.Path]::GetRelativePath($Dossier, $j.Parent.Parent.FullName)
        $ici = if ($cas -eq '.') { $Cible } else { Join-Path $Cible $cas }
        New-Item -ItemType Directory -Force -Path $ici | Out-Null
        Copy-Item -Path (Join-Path $j.FullName '*') -Destination $ici -Force -ErrorAction SilentlyContinue
    }
}

# Range le journal et le bilan dans $Sortie, ferme ce qui reste, sort avec 0 ou 1.
function Terminer-Essai {
    Arreter-Tout
    $e = $script:Essai
    $code = 1
    if ($null -ne $e) {
        $dossierJournaux = Join-Path $e.Sortie "journaux/$($e.Nom)"
        New-Item -ItemType Directory -Force -Path $dossierJournaux | Out-Null
        Ranger-Journaux -Dossier $e.Dossier -Cible $dossierJournaux
        $echecs = @($script:Verifications | Where-Object { -not $_.Reussi }).Count
        $reussi = ($script:Verifications.Count -gt 0) -and ($echecs -eq 0)
        $bilan = [pscustomobject]@{
            Nom           = $e.Nom
            Reussi        = $reussi
            Verifications = @($script:Verifications)
            Mesures       = @($script:Mesures)
        }
        $bilan | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $e.Sortie "$($e.Nom).json") -Encoding utf8
        Remove-Item -LiteralPath $e.Dossier -Recurse -Force -ErrorAction SilentlyContinue
        if ($reussi) { $code = 0 }
        Write-Host "=== $($e.Nom) : $(if ($reussi) { 'réussi' } else { "échoué ($echecs vérification(s))" }) ==="
    }
    exit $code
}

# --- Menu de l'icône (menu web, piste A « Liste ») -------------------------------------
# Le vrai geste : le message que Windows envoie à la fenêtre cachée de l'icône au clic droit
# (WM_USER_TRAYICON 6002, WM_RBUTTONUP 0x205) ; le menu est une page lue par UI Automation.
# `Ouvrir-Menu-Icone` rend { Fenetre, Textes, Lus, Rect, Lignes } : `Textes` donne chaque
# position dans l'ordre, « libellé<tab>raccourci » (ou le libellé seul) et '' pour un
# séparateur ; `Rect` est la carte, pixels réels { X, Y, L, H } ; `Lignes`, les éléments par
# libellé.
function Initialiser-Menu-Icone {
    if ('BreezyShotEssai.MenuIcone' -as [type]) { return }
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace BreezyShotEssai {
public static class MenuIcone {
    delegate bool Rappel(IntPtr h, IntPtr p);
    [DllImport("user32.dll")] static extern bool EnumWindows(Rappel f, IntPtr p);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr c);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr h, uint f);
    // La fenêtre de premier niveau sous le point (GA_ROOT).
    public static IntPtr RacineSous(int x, int y) {
        var p = new POINT(); p.X = x; p.Y = y;
        IntPtr h = WindowFromPoint(p);
        return h == IntPtr.Zero ? IntPtr.Zero : GetAncestor(h, 2);
    }
    [DllImport("user32.dll")] static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
    [DllImport("user32.dll")] static extern void keybd_event(byte vk, byte scan, uint f, UIntPtr e);
    public static IntPtr[] Fenetres(int pid, string classe) {
        var liste = new System.Collections.Generic.List<IntPtr>();
        EnumWindows((h, p) => {
            uint proprio; GetWindowThreadProcessId(h, out proprio);
            if ((int)proprio != pid) return true;
            var nom = new System.Text.StringBuilder(256);
            GetClassName(h, nom, nom.Capacity);
            if (nom.ToString() == classe) liste.Add(h);
            return true;
        }, IntPtr.Zero);
        return liste.ToArray();
    }
    public static void Cliquer(int x, int y) {
        SetCursorPos(x, y);
        mouse_event(0x1, 0, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(30);
        mouse_event(0x2, 0, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(30);
        mouse_event(0x4, 0, 0, 0, UIntPtr.Zero);
    }
    public static void Touche(byte vk) {
        keybd_event(vk, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(30);
        keybd_event(vk, 0, 2, UIntPtr.Zero);
    }
}
}
'@
    try { $null = [BreezyShotEssai.MenuIcone]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }
}

# Le clic droit sur l'icône, tel que Windows l'envoie à sa fenêtre cachée.
function Clic-Icone([int]$IdProc, [string]$Quoi) {
    $M = [BreezyShotEssai.MenuIcone]
    $icones = $M::Fenetres($IdProc, 'tray_icon_app')
    if ($icones.Length -lt 1) { throw "$Quoi : aucune fenêtre tray_icon_app (menu de l'icône impossible à ouvrir)" }
    foreach ($h in $icones) { [void]$M::PostMessage($h, 6002, [IntPtr]::Zero, [IntPtr]0x205) }
}

function Ouvrir-Menu-Icone([int]$IdProc, [string]$Dossier, [string]$Quoi, $Souris = $null) {
    Initialiser-Menu-Icone
    $M = [BreezyShotEssai.MenuIcone]
    $AE = [System.Windows.Automation.AutomationElement]
    $CT = [System.Windows.Automation.ControlType]
    if ($null -ne $Souris) { [void]$M::SetCursorPos($Souris[0], $Souris[1]) }
    # Un clic qui suit de moins de 300 ms une fermeture ne rouvre pas (anti-rebond).
    Start-Sleep -Milliseconds 400
    $avant = Compter-Lignes -Texte 'menu ouvert' -FinDeLigne -Dossier $Dossier
    Clic-Icone $IdProc $Quoi
    if (-not (Attendre-Ligne -Texte 'menu ouvert' -FinDeLigne -Fois ($avant + 1) -DelaiS 5 -Dossier $Dossier)) {
        throw "$Quoi : le menu de l'icône ne s'ouvre pas en 5 s"
    }
    $fen = [IntPtr]::Zero
    foreach ($l in ((Lire-Journal -Dossier $Dossier) -split "`n")) {
        if ($l.TrimEnd("`r") -match 'menu fenetre (\d+)$') { $fen = [IntPtr][long]$Matches[1] }
    }
    if ($fen -eq [IntPtr]::Zero) { throw "$Quoi : fenêtre du menu absente du journal" }
    # La carte et ses lignes (la page reçoit ses lignes à l'ouverture), dans l'ordre.
    $condMenu = [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, $CT::Menu)
    $marcheur = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    # La page se vide à chaque fermeture : les lignes lues sont celles de cette ouverture.
    $textes = @(); $lignes = @{}; $carte = $null; $vu = 'carte introuvable'
    $limite = [DateTime]::UtcNow.AddSeconds(5)
    do {
        try {
            $trouvee = $AE::FromHandle($fen).FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condMenu)
            if ($null -ne $trouvee) {
                $lusIci = @(); $lignesIci = @{}
                $e = $marcheur.GetFirstChild($trouvee)
                while ($null -ne $e) {
                    $c = $e.Current
                    if ($c.ControlType -eq $CT::Separator) { $lusIci += '' }
                    elseif ($c.ControlType -eq $CT::MenuItem) {
                        # Le raccourci : le texte de la ligne qui n'est pas son libellé.
                        $racc = $null
                        $tous = $e.FindAll([System.Windows.Automation.TreeScope]::Descendants,
                            [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, $CT::Text))
                        foreach ($t in $tous) { if ($t.Current.Name -ne '' -and $t.Current.Name -ne $c.Name) { $racc = $t.Current.Name } }
                        # À défaut de texte lisible : aria-keyshortcuts (AcceleratorKey).
                        if (-not $racc -and $c.AcceleratorKey) {
                            $racc = ($c.AcceleratorKey -replace '^Control', 'Ctrl' -replace 'Shift', 'Maj') -replace '\+', ' + '
                        }
                        $lusIci += $(if ($racc) { "$($c.Name)`t$racc" } else { $c.Name })
                        $lignesIci[$c.Name] = $e
                    }
                    $e = $marcheur.GetNextSibling($e)
                }
                # Parcours complet et sans erreur : il fait foi.
                $vu = "$($lusIci.Count) positions"
                if ($lignesIci.Count -gt 0) { $textes = $lusIci; $lignes = $lignesIci; $carte = $trouvee; break }
            }
        } catch { $vu = "UI Automation : $($_.Exception.Message)" }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $limite)
    if ($lignes.Count -eq 0) { throw "$Quoi : lignes du menu illisibles ($vu)" }
    # Fondu d'ouverture fini.
    Start-Sleep -Milliseconds 250
    $r = $carte.Current.BoundingRectangle
    $lus = ($textes | ForEach-Object { "« $($_ -replace "`t", '<tab>') »" }) -join ', '
    return [pscustomobject]@{
        Fenetre = $fen; Textes = $textes; Lus = $lus; Lignes = $lignes; Carte = $carte; IdProc = $IdProc
        Rect = [pscustomobject]@{ X = [int]$r.X; Y = [int]$r.Y; L = [int]$r.Width; H = [int]$r.Height }
    }
}

# Clic réel en (x, y), seulement si le menu est montré et sous ce point (jamais de clic
# hors des fenêtres de l'app).
function Cliquer-Dans-Menu($Menu, [int]$X, [int]$Y, [string]$Quoi) {
    $M = [BreezyShotEssai.MenuIcone]
    if (-not $M::IsWindowVisible($Menu.Fenetre)) { throw "$Quoi : le menu s'est fermé avant le clic" }
    $sous = $M::RacineSous($X, $Y)
    if ($sous -ne $Menu.Fenetre) { throw "$Quoi : le menu n'est pas sous le point $X,$Y (fenêtre $sous)" }
    $M::Cliquer($X, $Y)
}

# Clic réel au centre de la ligne dont le libellé vaut $Libelle.
function Cliquer-Ligne-Menu($Menu, [string]$Libelle, [string]$Quoi) {
    $e = $Menu.Lignes[$Libelle]
    if ($null -eq $e) { throw "$Quoi : pas de ligne « $Libelle » au menu ($($Menu.Lus))" }
    $r = $e.Current.BoundingRectangle
    if ($r.IsEmpty -or $r.Width -le 0) { throw "$Quoi : rectangle de « $Libelle » illisible" }
    Cliquer-Dans-Menu $Menu ([int]($r.X + $r.Width / 2)) ([int]($r.Y + $r.Height / 2)) $Quoi
}

# Ferme le menu sans rien choisir : un clic dans le retrait de la carte (aucune ligne) lui
# donne le clavier, puis Échap — seulement si le menu est au premier plan (jamais de touche
# hors des fenêtres de l'app). Sinon, un second clic sur l'icône le referme.
# $script:FermeParEchap dit ensuite si c'est Échap qui l'a fermé.
$script:FermeParEchap = $false
function Fermer-Menu-Icone($Menu, [string]$Quoi) {
    $M = [BreezyShotEssai.MenuIcone]
    $script:FermeParEchap = $false
    if (-not $M::IsWindowVisible($Menu.Fenetre)) { return }
    $r = $Menu.Rect
    Cliquer-Dans-Menu $Menu ([int]($r.X + $r.L / 2)) ([int]($r.Y + 2)) $Quoi
    $limite = [DateTime]::UtcNow.AddSeconds(1)
    while ($M::GetForegroundWindow() -ne $Menu.Fenetre -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 30 }
    if ($M::GetForegroundWindow() -eq $Menu.Fenetre) { $M::Touche(0x1B) }
    $limite = [DateTime]::UtcNow.AddSeconds(2)
    while ($M::IsWindowVisible($Menu.Fenetre) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 30 }
    $script:FermeParEchap = -not $M::IsWindowVisible($Menu.Fenetre)
    if ($M::IsWindowVisible($Menu.Fenetre)) {
        Write-Host "$Quoi : Échap n'a pas fermé le menu, second clic sur l'icône"
        Clic-Icone $Menu.IdProc $Quoi
        $limite = [DateTime]::UtcNow.AddSeconds(2)
        while ($M::IsWindowVisible($Menu.Fenetre) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 30 }
    }
    if ($M::IsWindowVisible($Menu.Fenetre)) { throw "$Quoi : le menu ne se ferme pas" }
    Start-Sleep -Milliseconds 200
}
