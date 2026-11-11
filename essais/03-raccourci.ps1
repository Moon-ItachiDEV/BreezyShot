# Étape 3, raccourci et bandeau (ARCHITECTURE.md §5, §6).
# (a) Impr. écran libre : retenue, appui simulé, réussite affichée puis terminée ;
# (b) Impr. écran réservée par Windows : Ctrl+Maj+1, annonce (raison réservée), appui ; relance :
#     annonce déjà faite ;
# (c) Impr. écran tenue par une autre application : Ctrl+Maj+1, annonce (raison refusée) ;
# (d) clic sur la croix d'une info, sans changer la fenêtre au premier plan ; clic à côté
#     de la pilule sans effet ;
# (e) après --quitter, le raccourci est libéré.
# C'est l'ESSAI qui écrit PrintScreenKeyForSnippingEnabled dans HKCU de la machine d'essai
# (l'app ne l'écrit jamais) ; la valeur d'origine est remise dans le finally.
# Hors Windows : seule la partie commune tourne, le reste est « sauté hors Windows ».

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '03-raccourci' -Exe $Exe -Sortie $Sortie | Out-Null

$script:Auxiliaires = [System.Collections.Generic.List[object]]::new()
$script:Dossiers = [System.Collections.Generic.List[string]]::new()
$script:Pwsh = (Get-Process -Id $PID).Path

# --- Aides -------------------------------------------------------------------------------

# Un dossier d'essai par cas, pour partir sans réglages ni etat.json.
function Dossier-Cas([string]$Nom) {
    $d = Join-Path $script:Essai.Dossier $Nom
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $script:Dossiers.Add($d)
    return $d
}

# Dernière ligne du journal qui correspond à l'expression $Motif ; $null sinon.
function Derniere-Ligne([string]$Motif, [string]$Dossier) {
    $trouvee = $null
    foreach ($brute in ((Lire-Journal -Dossier $Dossier) -split "`n")) {
        $l = $brute.TrimEnd("`r")
        if ($l -match $Motif) { $trouvee = $l }
    }
    return $trouvee
}

# Nombre de lignes du journal qui correspondent à $Motif.
function Compter-Motif([string]$Motif, [string]$Dossier) {
    $n = 0
    foreach ($brute in ((Lire-Journal -Dossier $Dossier) -split "`n")) {
        if ($brute.TrimEnd("`r") -match $Motif) { $n++ }
    }
    return $n
}

# Attend qu'au moins $Fois lignes correspondent à $Motif. Rend la dernière, ou lève une
# erreur claire. Dans un dossier déjà servi (relance), passer $Fois = déjà vues + 1 : sinon
# la ligne du lancement précédent répond tout de suite.
function Attendre-Motif([string]$Motif, [string]$Dossier, [double]$DelaiS, [string]$Quoi, [int]$Fois = 1) {
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        if ((Compter-Motif -Motif $Motif -Dossier $Dossier) -ge $Fois) {
            return (Derniere-Ligne -Motif $Motif -Dossier $Dossier)
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $limite)
    throw "$Quoi : rien dans le journal en $DelaiS s (motif « $Motif », $Fois fois)"
}

# Démarre l'app et attend CE démarrage-ci (lignes comptées avant le lancement).
function Demarrer-Cas([string]$Dossier, [string]$Quoi) {
    $prets = Compter-Motif -Motif 'application pret$' -Dossier $Dossier
    $pages = Compter-Motif -Motif 'fenetres +bandeau pret$' -Dossier $Dossier
    $p = Demarrer-App -Dossier $Dossier
    $null = Attendre-Motif -Motif 'application pret$' -Dossier $Dossier -DelaiS 30 -Quoi "$Quoi : démarrage" -Fois ($prets + 1)
    $null = Attendre-Motif -Motif 'fenetres +bandeau pret$' -Dossier $Dossier -DelaiS 20 -Quoi "$Quoi : page du bandeau" -Fois ($pages + 1)
    return $p
}

function Fermer-Cas($Processus, [string]$Dossier, [string]$Quoi) {
    $arrets = Compter-Motif -Motif 'arret propre$' -Dossier $Dossier
    $q = Quitter-App -DelaiS 10 -Dossier $Dossier
    if (-not $q.Fini) { throw "$Quoi : --quitter n'a pas répondu en 10 s" }
    if (-not $Processus.WaitForExit(10000)) { throw "$Quoi : l'app ne s'est pas fermée en 10 s" }
    $null = Attendre-Motif -Motif 'arret propre$' -Dossier $Dossier -DelaiS 3 -Quoi "$Quoi : arrêt" -Fois ($arrets + 1)
}

# --- Partie commune (tous systèmes) -------------------------------------------------------

try {
    $commun = Dossier-Cas 'commun'
    $p = Demarrer-Cas -Dossier $commun -Quoi 'commun'
    $ligne = Attendre-Motif -Motif 'raccourci zone: ' -Dossier $commun -DelaiS 5 -Quoi 'commun'
    Verifier 'ligne « raccourci zone: … »' ($null -ne $ligne) ($ligne -replace '^.*raccourci zone: ', '') | Out-Null
    Verifier 'bandeau préparé (bandeau pret)' $true | Out-Null
    Verifier 'ligne « bandeau exclu des captures »' ((Compter-Lignes -Texte 'bandeau exclu des captures: ' -Dossier $commun) -eq 1) | Out-Null
    if ($IsWindows) {
        Verifier 'bandeau exclu des captures: oui' ((Compter-Lignes -Texte 'bandeau exclu des captures: oui' -Dossier $commun) -eq 1) | Out-Null
    }
    Fermer-Cas $p $commun 'commun'
    Verifier 'raccourcis libérés à l''arrêt' ((Compter-Lignes -Texte 'raccourcis liberes' -Dossier $commun) -eq 1) | Out-Null

    if (-not $IsWindows) {
        foreach ($cas in '(a) Impr. écran libre', '(b) Impr. écran réservée', '(c) Impr. écran prise', '(d) clics sur le bandeau', '(e) raccourci libéré') {
            Write-Host "[saute] $cas : sauté hors Windows"
        }
    }
    else {
        # --- Windows : clavier, souris, registre ---------------------------------------
        Add-Type -Namespace BreezyShotEssai -Name Entree -MemberDefinition @'
[DllImport("user32.dll")]
static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
[DllImport("user32.dll")]
static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
[DllImport("user32.dll")]
public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")]
public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")]
static extern uint MapVirtualKey(uint code, uint type);
const uint ETENDUE = 0x1, RELACHEE = 0x2;
static uint Drapeaux(byte vk) { return vk == 0x2C ? ETENDUE : 0u; }
// Appuie les touches dans l'ordre, puis les relâche dans l'ordre inverse.
public static void Touches(byte[] vks) {
    foreach (var vk in vks) keybd_event(vk, (byte)MapVirtualKey(vk, 0), Drapeaux(vk), UIntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    for (int i = vks.Length - 1; i >= 0; i--) keybd_event(vks[i], (byte)MapVirtualKey(vks[i], 0), Drapeaux(vks[i]) | RELACHEE, UIntPtr.Zero);
}
// Amène la souris, laisse au bandeau le temps de devenir cliquable, puis clic gauche.
public static void Clic(int x, int y) {
    SetCursorPos(x, y);
    System.Threading.Thread.Sleep(400);
    mouse_event(0x2, 0, 0, 0, UIntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    mouse_event(0x4, 0, 0, 0, UIntPtr.Zero);
}
'@
        $VK_SNAPSHOT = [byte]0x2C
        $CTRL_MAJ_1 = [byte[]]@(0x11, 0x10, 0x31)
        $cle = 'HKCU:\Control Panel\Keyboard'
        $nomValeur = 'PrintScreenKeyForSnippingEnabled'
        $origine = (Get-ItemProperty -LiteralPath $cle -Name $nomValeur -ErrorAction SilentlyContinue)
        $valeurOrigine = if ($null -ne $origine) { $origine.$nomValeur } else { $null }
        Write-Host "valeur d'origine de $nomValeur : $(if ($null -eq $valeurOrigine) { 'absente' } else { $valeurOrigine })"

        function Poser-Valeur([int]$v) {
            Set-ItemProperty -LiteralPath $cle -Name $nomValeur -Value $v -Type DWord
        }

        # Lance l'aide qui tient un raccourci. Rend 'tenu' ou 'refuse'.
        function Tenir-Raccourci([int]$Vk, [int]$Mods, [string]$Quoi) {
            $f = Join-Path $script:Essai.Dossier ("aide-" + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.txt')
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -WindowStyle Hidden -ArgumentList @(
                '-NoProfile', '-File', (Join-Path $PSScriptRoot 'tenir-raccourci.ps1'),
                '-Vk', $Vk, '-Mods', $Mods, '-Fichier', $f, '-DureeS', 120)
            $script:Auxiliaires.Add($aide)
            $limite = [DateTime]::UtcNow.AddSeconds(20)
            while (-not (Test-Path -LiteralPath $f)) {
                if ([DateTime]::UtcNow -ge $limite) { throw "$Quoi : l'aide n'a rien répondu en 20 s" }
                Start-Sleep -Milliseconds 100
            }
            Start-Sleep -Milliseconds 100
            return (Get-Content -LiteralPath $f -Raw).Trim()
        }

        function Fermer-Auxiliaires {
            foreach ($a in $script:Auxiliaires) {
                try { if (-not $a.HasExited) { $a.Kill(); $null = $a.WaitForExit(5000) } } catch { }
            }
            $script:Auxiliaires.Clear()
        }

        # Ligne « bandeau affiche <id> <genre> ecran x,y lxh echelle e » → objet.
        function Lire-Affiche([string]$Ligne) {
            if ($Ligne -notmatch 'bandeau affiche (\d+) (\w+) ecran (-?\d+),(-?\d+) (\d+)x(\d+) echelle ([\d.]+)') {
                throw "ligne d'affichage illisible : $Ligne"
            }
            return [pscustomobject]@{ Id = [int]$Matches[1]; Genre = $Matches[2]; Echelle = [double]::Parse($Matches[7], [Globalization.CultureInfo]::InvariantCulture) }
        }

        try {
            # (a) Impr. écran libre.
            Poser-Valeur 0
            $da = Dossier-Cas 'a'
            $p = Demarrer-Cas -Dossier $da -Quoi '(a)'
            Verifier '(a) raccourci zone: ImprEcran' ((Compter-Lignes -Texte 'raccourci zone: ImprEcran' -Dossier $da) -eq 1) | Out-Null
            Verifier '(a) aucune annonce' ((Compter-Lignes -Texte 'annonce raccourci' -Dossier $da) -eq 0) | Out-Null
            Verifier '(e) tenu par BreezyShot : une autre application ne peut pas le prendre' ((Tenir-Raccourci -Vk $VK_SNAPSHOT -Mods 0 -Quoi '(e) avant') -eq 'refuse') | Out-Null
            Fermer-Auxiliaires
            [BreezyShotEssai.Entree]::Touches([byte[]]@($VK_SNAPSHOT))
            $null = Attendre-Motif -Motif 'raccourci presse: zone$' -Dossier $da -DelaiS 5 -Quoi '(a) appui sur Impr. écran'
            Verifier '(a) raccourci presse: zone' $true | Out-Null
            $chrono = [System.Diagnostics.Stopwatch]::StartNew()
            $aff = Lire-Affiche (Attendre-Motif -Motif 'bandeau affiche \d+ reussite ' -Dossier $da -DelaiS 5 -Quoi '(a) réussite')
            Verifier '(a) bandeau affiche … reussite' ($aff.Genre -eq 'reussite') | Out-Null
            Verifier '(a) passage Repos -> Gel -> Repos' ((Compter-Lignes -Texte 'etat Gel -> Repos' -Dossier $da) -eq 1) | Out-Null
            $null = Attendre-Motif -Motif "bandeau termine $($aff.Id)$" -Dossier $da -DelaiS 5 -Quoi '(a) fin de la réussite'
            $chrono.Stop()
            Verifier '(a) bandeau termine en moins de 5 s' ($chrono.Elapsed.TotalSeconds -lt 5) "$([math]::Round($chrono.Elapsed.TotalSeconds, 2)) s" | Out-Null
            Fermer-Cas $p $da '(a)'
            # (e) Après --quitter, le raccourci est libéré.
            Verifier '(e) après --quitter, une autre application prend Impr. écran' ((Tenir-Raccourci -Vk $VK_SNAPSHOT -Mods 0 -Quoi '(e) après') -eq 'tenu') | Out-Null
            Fermer-Auxiliaires

            # (b) Impr. écran réservée par Windows, puis (d) clics, puis relance.
            Poser-Valeur 1
            $db = Dossier-Cas 'b'
            $p = Demarrer-Cas -Dossier $db -Quoi '(b)'
            Verifier '(b) raccourci zone: Ctrl+Maj+1' ((Compter-Lignes -Texte 'raccourci zone: Ctrl+Maj+1' -Dossier $db) -eq 1) | Out-Null
            Verifier '(b) annonce (raison : réservée par Windows)' ((Compter-Lignes -Texte 'annonce raccourci: reserve_par_le_systeme' -Dossier $db) -eq 1) | Out-Null
            $info = Lire-Affiche (Attendre-Motif -Motif 'bandeau affiche \d+ info ' -Dossier $db -DelaiS 10 -Quoi '(b) info affichée')
            Verifier '(b) bandeau affiche … info' ($info.Genre -eq 'info') | Out-Null
            # (d) La zone de la pilule, une fois l'animation d'entrée finie.
            $null = Attendre-Motif -Motif "bandeau zone $($info.Id) " -Dossier $db -DelaiS 5 -Quoi '(d) zone de la pilule'
            Start-Sleep -Milliseconds 800
            $zone = Derniere-Ligne -Motif "bandeau zone $($info.Id) (-?\d+),(-?\d+) (\d+)x(\d+)" -Dossier $db
            if ($zone -notmatch "bandeau zone $($info.Id) (-?\d+),(-?\d+) (\d+)x(\d+)") { throw "(d) ligne de zone illisible : $zone" }
            $zx = [int]$Matches[1]; $zy = [int]$Matches[2]; $zl = [int]$Matches[3]; $zh = [int]$Matches[4]
            $e = $info.Echelle
            # Croix : bouton de 24 px, à 8 px du bord droit (jetons --bandeau-bouton, --bandeau-marge-large).
            $croixX = [int]($zx + $zl - (8 + 12) * $e)
            $croixY = [int]($zy + $zh / 2)
            $cote = [int]($zx - 20 * $e)
            Write-Host "pilule $zx,$zy ${zl}x$zh echelle $e ; croix $croixX,$croixY ; à côté $cote,$croixY"
            [BreezyShotEssai.Entree]::Clic($cote, $croixY)
            Start-Sleep -Milliseconds 700
            Verifier '(d) clic à côté de la pilule : aucune bandeau action' ((Compter-Lignes -Texte 'bandeau action' -Dossier $db) -eq 0) | Out-Null
            $premierPlanAvant = [BreezyShotEssai.Entree]::GetForegroundWindow()
            [BreezyShotEssai.Entree]::Clic($croixX, $croixY)
            $null = Attendre-Motif -Motif "bandeau action $($info.Id) fermer$" -Dossier $db -DelaiS 5 -Quoi '(d) clic sur la croix'
            Verifier '(d) bandeau action <id> fermer' $true | Out-Null
            $null = Attendre-Motif -Motif "bandeau termine $($info.Id)$" -Dossier $db -DelaiS 5 -Quoi '(d) fin de l''info'
            Verifier '(d) bandeau termine <id>' $true | Out-Null
            $premierPlanApres = [BreezyShotEssai.Entree]::GetForegroundWindow()
            Verifier '(d) fenêtre au premier plan inchangée' ($premierPlanAvant -eq $premierPlanApres) "avant $premierPlanAvant, après $premierPlanApres" | Out-Null
            # (b) Appui sur Ctrl+Maj+1.
            [BreezyShotEssai.Entree]::Touches($CTRL_MAJ_1)
            $null = Attendre-Motif -Motif 'raccourci presse: zone$' -Dossier $db -DelaiS 5 -Quoi '(b) appui sur Ctrl+Maj+1'
            Verifier '(b) raccourci presse: zone' $true | Out-Null
            Fermer-Cas $p $db '(b)'
            # Relance : la même situation n'est pas annoncée deux fois.
            $p = Demarrer-Cas -Dossier $db -Quoi '(b) relance'
            Verifier '(b) relance : deux démarrages complets dans ce dossier' ((Compter-Motif -Motif 'application pret$' -Dossier $db) -eq 2) | Out-Null
            Verifier '(b) relance : annonce raccourci: deja faite' ((Compter-Lignes -Texte 'annonce raccourci: deja faite' -Dossier $db) -eq 1) | Out-Null
            Verifier '(b) relance : une seule annonce affichée en tout' ((Compter-Lignes -Texte 'annonce raccourci: reserve_par_le_systeme' -Dossier $db) -eq 1) | Out-Null
            Fermer-Cas $p $db '(b) relance'

            # (c) Impr. écran tenue par une autre application.
            Poser-Valeur 0
            Verifier '(c) l''aide tient Impr. écran' ((Tenir-Raccourci -Vk $VK_SNAPSHOT -Mods 0 -Quoi '(c)') -eq 'tenu') | Out-Null
            $dc = Dossier-Cas 'c'
            $p = Demarrer-Cas -Dossier $dc -Quoi '(c)'
            Verifier '(c) raccourci zone: Ctrl+Maj+1' ((Compter-Lignes -Texte 'raccourci zone: Ctrl+Maj+1' -Dossier $dc) -eq 1) | Out-Null
            Verifier '(c) ImprEcran écarté : refusé par le système' ((Compter-Lignes -Texte 'raccourci zone ecarte: ImprEcran (refuse par le systeme)' -Dossier $dc) -eq 1) | Out-Null
            Verifier '(c) annonce (raison : refusée par le système)' ((Compter-Lignes -Texte 'annonce raccourci: refuse_par_le_systeme' -Dossier $dc) -eq 1) | Out-Null
            $null = Attendre-Motif -Motif 'bandeau affiche \d+ info ' -Dossier $dc -DelaiS 10 -Quoi '(c) info affichée'
            Fermer-Cas $p $dc '(c)'
        }
        finally {
            Fermer-Auxiliaires
            Arreter-Tout
            if ($null -eq $valeurOrigine) {
                Remove-ItemProperty -LiteralPath $cle -Name $nomValeur -ErrorAction SilentlyContinue
            }
            else {
                Set-ItemProperty -LiteralPath $cle -Name $nomValeur -Value $valeurOrigine -Type DWord
            }
            Write-Host "valeur de $nomValeur remise"
        }
    }
}
catch {
    Verifier "déroulement de l'essai" $false "$_" | Out-Null
}
finally {
    # En cas d'échec, la fin du journal de chaque cas dans la sortie de l'essai : ce que
    # l'app a réellement fait, lisible même sans télécharger les résultats.
    if (@($script:Verifications | Where-Object { -not $_.Reussi }).Count -gt 0) {
        foreach ($d in $script:Dossiers) {
            Write-Host "--- fin du journal du cas $(Split-Path -Leaf $d) ---"
            $lignes = @((Lire-Journal -Dossier $d) -split "`n" | Where-Object { $_.Trim() -ne '' })
            $lignes | Select-Object -Last 25 | ForEach-Object { Write-Host $_.TrimEnd("`r") }
        }
    }
    # Journaux de chaque cas, puis bilan.
    foreach ($a in $script:Auxiliaires) { try { if (-not $a.HasExited) { $a.Kill() } } catch { } }
    foreach ($d in $script:Dossiers) {
        $source = Join-Path $d 'local/Journal'
        if (Test-Path -LiteralPath $source) {
            $cible = Join-Path $script:Essai.Sortie ("journaux/03-raccourci-" + (Split-Path -Leaf $d))
            New-Item -ItemType Directory -Force -Path $cible | Out-Null
            Copy-Item -Path (Join-Path $source '*') -Destination $cible -Force
        }
    }
    Terminer-Essai
}
