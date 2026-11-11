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
// Amène la souris et laisse au bandeau le temps de devenir cliquable (surveillance 30 ms).
public static void Placer(int x, int y) {
    SetCursorPos(x, y);
    System.Threading.Thread.Sleep(400);
}
// Clic gauche là où est la souris.
public static void Cliquer() {
    mouse_event(0x2, 0, 0, 0, UIntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    mouse_event(0x4, 0, 0, 0, UIntPtr.Zero);
}
[StructLayout(LayoutKind.Sequential)]
public struct POINT { public int X; public int Y; }
[DllImport("user32.dll")]
static extern IntPtr WindowFromPoint(POINT p);
[DllImport("user32.dll")]
static extern IntPtr GetAncestor(IntPtr h, uint flags);
// Fenêtre de premier niveau sous ce point (GA_ROOT = 2) : celle qui recevrait le clic.
// WindowFromPoint saute les fenêtres traversées par les clics (WS_EX_TRANSPARENT).
public static IntPtr RacineSous(int x, int y) {
    POINT p; p.X = x; p.Y = y;
    IntPtr h = WindowFromPoint(p);
    return h == IntPtr.Zero ? h : GetAncestor(h, 2);
}
[DllImport("user32.dll")]
public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
public static extern IntPtr GetWindowLongPtr(IntPtr h, int index);
'@
        Add-Type -AssemblyName System.Drawing
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

        # HWND du bandeau, écrit par l'app à la création (« bandeau fenetre <hwnd> »).
        function Fenetre-Bandeau([string]$Dossier) {
            $l = Attendre-Motif -Motif 'bandeau fenetre (\d+)$' -Dossier $Dossier -DelaiS 5 -Quoi 'HWND du bandeau'
            if ($l -notmatch 'bandeau fenetre (\d+)$') { throw "ligne de HWND illisible : $l" }
            return [IntPtr][long]$Matches[1]
        }

        # Thème des applis (la page suit prefers-color-scheme, donné par Windows aux applis).
        $clair = $true
        $themeApplis = Get-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize' -Name AppsUseLightTheme -ErrorAction SilentlyContinue
        if ($null -ne $themeApplis -and $themeApplis.AppsUseLightTheme -eq 0) { $clair = $false }
        # Fond de la pilule : --acrylic-solid de jetons.css (R, G, B).
        $fondAttendu = if ($clair) { @(0xF9, 0xF9, 0xF9) } else { @(0x2C, 0x2C, 0x2C) }
        Write-Host "thème des applis : $(if ($clair) { 'clair' } else { 'sombre' }) ; fond attendu $($fondAttendu -join ',')"

        function Proche($c, [int[]]$rgb) {
            # System.Drawing.Color donne R, G, B par leur nom : pas d'ordre BGR à gérer ici.
            return ([math]::Abs($c.R - $rgb[0]) -le 6) -and ([math]::Abs($c.G - $rgb[1]) -le 6) -and ([math]::Abs($c.B - $rgb[2]) -le 6)
        }

        # Copie l'écran autour de la pilule (80 px de marge), enregistre les deux PNG si
        # demandé, et rend { Fond : nb de points du fond à la bonne couleur (sur 4),
        # Dehors : vrai si le point hors pilule a cette couleur, Detail : couleurs lues }.
        function Lire-Pilule([int]$zx, [int]$zy, [int]$zl, [int]$zh, [double]$e, [string]$Nom) {
            $m = 80
            $bmp = [System.Drawing.Bitmap]::new($zl + 2 * $m, $zh + 2 * $m)
            $g = [System.Drawing.Graphics]::FromImage($bmp)
            try { $g.CopyFromScreen($zx - $m, $zy - $m, 0, 0, [System.Drawing.Size]::new($zl + 2 * $m, $zh + 2 * $m)) }
            finally { $g.Dispose() }
            if ($Nom) {
                $bmp.Save((Join-Path $script:Essai.Sortie "$Nom-large.png"), [System.Drawing.Imaging.ImageFormat]::Png)
                $pilule = $bmp.Clone([System.Drawing.Rectangle]::new($m, $m, $zl, $zh), $bmp.PixelFormat)
                $pilule.Save((Join-Path $script:Essai.Sortie "$Nom.png"), [System.Drawing.Imaging.ImageFormat]::Png)
                $pilule.Dispose()
            }
            # Points du fond : haut et bas au milieu, bord gauche, entre le texte et la croix.
            $points = @(
                @([int]($zl / 2), [int](4 * $e)),
                @([int]($zl / 2), [int]($zh - 4 * $e)),
                @([int](5 * $e), [int]($zh / 2)),
                @([int]($zl - 50 * $e), [int]($zh / 2)))
            $bons = 0; $lus = @()
            foreach ($pt in $points) {
                $c = $bmp.GetPixel($m + $pt[0], $m + $pt[1])
                $lus += "$($c.R),$($c.G),$($c.B)"
                if (Proche $c $fondAttendu) { $bons++ }
            }
            $dehors = $bmp.GetPixel([int]($m - 30 * $e), $m + [int]($zh / 2))
            $lus += "dehors $($dehors.R),$($dehors.G),$($dehors.B)"
            $bmp.Dispose()
            return [pscustomobject]@{ Fond = $bons; Dehors = (Proche $dehors $fondAttendu); Detail = ($lus -join ' ; ') }
        }

        # Dernière zone de la pilule du message $Id → x, y, l, h (pixels réels).
        function Lire-Zone([int]$Id, [string]$Dossier) {
            $null = Attendre-Motif -Motif "bandeau zone $Id " -Dossier $Dossier -DelaiS 5 -Quoi 'zone de la pilule'
            Start-Sleep -Milliseconds 800   # fin de l'animation d'entrée (300 ms)
            $z = Derniere-Ligne -Motif "bandeau zone $Id (-?\d+),(-?\d+) (\d+)x(\d+)" -Dossier $Dossier
            if ($z -notmatch "bandeau zone $Id (-?\d+),(-?\d+) (\d+)x(\d+)") { throw "ligne de zone illisible : $z" }
            return [pscustomobject]@{ X = [int]$Matches[1]; Y = [int]$Matches[2]; L = [int]$Matches[3]; H = [int]$Matches[4] }
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
            $hwnd = Fenetre-Bandeau $da
            $ex = [long][BreezyShotEssai.Entree]::GetWindowLongPtr($hwnd, -20)   # GWL_EXSTYLE
            $outil = ($ex -band 0x80) -ne 0          # WS_EX_TOOLWINDOW
            $appli = ($ex -band 0x40000) -ne 0       # WS_EX_APPWINDOW
            $sansActivation = ($ex -band 0x08000000) -ne 0   # WS_EX_NOACTIVATE
            Verifier '(a) style étendu : WS_EX_TOOLWINDOW, sans WS_EX_APPWINDOW, WS_EX_NOACTIVATE' ($outil -and -not $appli -and $sansActivation) ('0x{0:X8}' -f $ex) | Out-Null
            Verifier '(a) bandeau caché au départ' (-not [BreezyShotEssai.Entree]::IsWindowVisible($hwnd)) | Out-Null
            Verifier '(e) tenu par BreezyShot : une autre application ne peut pas le prendre' ((Tenir-Raccourci -Vk $VK_SNAPSHOT -Mods 0 -Quoi '(e) avant') -eq 'refuse') | Out-Null
            Fermer-Auxiliaires
            [BreezyShotEssai.Entree]::Touches([byte[]]@($VK_SNAPSHOT))
            $null = Attendre-Motif -Motif 'raccourci presse: zone$' -Dossier $da -DelaiS 5 -Quoi '(a) appui sur Impr. écran'
            Verifier '(a) raccourci presse: zone' $true | Out-Null
            $chrono = [System.Diagnostics.Stopwatch]::StartNew()
            $aff = Lire-Affiche (Attendre-Motif -Motif 'bandeau affiche \d+ reussite ' -Dossier $da -DelaiS 5 -Quoi '(a) réussite')
            Verifier '(a) bandeau affiche … reussite' ($aff.Genre -eq 'reussite') | Out-Null
            Verifier '(a) passage Repos -> Gel -> Repos' ((Compter-Lignes -Texte 'etat Gel -> Repos' -Dossier $da) -eq 1) | Out-Null
            Verifier '(a) bandeau visible pendant la réussite' ([BreezyShotEssai.Entree]::IsWindowVisible($hwnd)) | Out-Null
            $null = Attendre-Motif -Motif "bandeau termine $($aff.Id)$" -Dossier $da -DelaiS 5 -Quoi '(a) fin de la réussite'
            $chrono.Stop()
            Verifier '(a) bandeau termine en moins de 5 s' ($chrono.Elapsed.TotalSeconds -lt 5) "$([math]::Round($chrono.Elapsed.TotalSeconds, 2)) s" | Out-Null
            Start-Sleep -Milliseconds 200
            Verifier '(a) après bandeau termine, la fenêtre est cachée (IsWindowVisible faux)' (-not [BreezyShotEssai.Entree]::IsWindowVisible($hwnd)) | Out-Null
            Verifier '(a) pas de cache d''office' ((Compter-Lignes -Texte "bandeau cache d'office" -Dossier $da) -eq 0) | Out-Null
            Fermer-Cas $p $da '(a)'
            # (e) Après --quitter, le raccourci est libéré.
            Verifier '(e) ligne « raccourcis liberes »' ((Compter-Lignes -Texte 'raccourcis liberes' -Dossier $da) -eq 1) | Out-Null
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
            $hwnd = Fenetre-Bandeau $db
            $z = Lire-Zone -Id $info.Id -Dossier $db
            $zx = $z.X; $zy = $z.Y; $zl = $z.L; $zh = $z.H
            $e = $info.Echelle
            # Exclu des captures : la copie d'écran ne montre PAS la pilule.
            Verifier '(b) bandeau exclu des captures: oui' ((Compter-Lignes -Texte 'bandeau exclu des captures: oui' -Dossier $db) -eq 1) | Out-Null
            $vu = Lire-Pilule $zx $zy $zl $zh $e 'bandeau-windows-exclu'
            Verifier '(b) exclu des captures : la copie d''écran ne montre pas la pilule' ($vu.Fond -eq 0) $vu.Detail | Out-Null
            # Croix : bouton de 24 px, à 8 px du bord droit (jetons --bandeau-bouton, --bandeau-marge-large).
            $croixX = [int]($zx + $zl - (8 + 12) * $e)
            $croixY = [int]($zy + $zh / 2)
            $cote = [int]($zx - 20 * $e)
            Write-Host "pilule $zx,$zy ${zl}x$zh echelle $e ; croix $croixX,$croixY ; à côté $cote,$croixY ; bandeau $hwnd"
            [BreezyShotEssai.Entree]::Placer($cote, $croixY)
            $sousCote = [BreezyShotEssai.Entree]::RacineSous($cote, $croixY)
            Verifier '(d) à côté de la pilule, la fenêtre sous la souris n''est pas le bandeau' ($sousCote -ne $hwnd) "sous la souris $sousCote" | Out-Null
            [BreezyShotEssai.Entree]::Cliquer()
            Start-Sleep -Milliseconds 700
            Verifier '(d) clic à côté de la pilule : aucune bandeau action' ((Compter-Lignes -Texte 'bandeau action' -Dossier $db) -eq 0) | Out-Null
            $premierPlanAvant = [BreezyShotEssai.Entree]::GetForegroundWindow()
            [BreezyShotEssai.Entree]::Placer($croixX, $croixY)
            $sousCroix = [BreezyShotEssai.Entree]::RacineSous($croixX, $croixY)
            Verifier '(d) sur la croix, la fenêtre sous la souris est le bandeau' ($sousCroix -eq $hwnd) "sous la souris $sousCroix" | Out-Null
            [BreezyShotEssai.Entree]::Cliquer()
            $null = Attendre-Motif -Motif "bandeau action $($info.Id) fermer$" -Dossier $db -DelaiS 5 -Quoi '(d) clic sur la croix'
            Verifier '(d) bandeau action <id> fermer' $true | Out-Null
            $null = Attendre-Motif -Motif "bandeau termine $($info.Id)$" -Dossier $db -DelaiS 5 -Quoi '(d) fin de l''info'
            Verifier '(d) bandeau termine <id>' $true | Out-Null
            Start-Sleep -Milliseconds 200
            Verifier '(d) après bandeau termine, la fenêtre est cachée' (-not [BreezyShotEssai.Entree]::IsWindowVisible($hwnd)) | Out-Null
            Verifier '(b) annonce retenue une fois montrée' ((Compter-Lignes -Texte "annonce montree $($info.Id): situation retenue" -Dossier $db) -eq 1) | Out-Null
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
            Fermer-Auxiliaires

            # (f) Le bandeau est réellement peint : avec BREEZYSHOT_ESSAI_CAPTURABLE=1, la
            # copie d'écran montre le fond de la pilule.
            Poser-Valeur 1
            $df = Dossier-Cas 'f'
            $env:BREEZYSHOT_ESSAI_CAPTURABLE = '1'
            try { $p = Demarrer-Cas -Dossier $df -Quoi '(f)' }
            finally { Remove-Item Env:BREEZYSHOT_ESSAI_CAPTURABLE -ErrorAction SilentlyContinue }
            Verifier '(f) bandeau exclu des captures: non (essai)' ((Compter-Lignes -Texte 'bandeau exclu des captures: non (essai)' -Dossier $df) -eq 1) | Out-Null
            $infoF = Lire-Affiche (Attendre-Motif -Motif 'bandeau affiche \d+ info ' -Dossier $df -DelaiS 10 -Quoi '(f) info affichée')
            $zf = Lire-Zone -Id $infoF.Id -Dossier $df
            [BreezyShotEssai.Entree]::Placer($zf.X - 300, $zf.Y + 400)   # souris loin de la pilule
            $vu = Lire-Pilule $zf.X $zf.Y $zf.L $zf.H $infoF.Echelle 'bandeau-windows'
            Verifier '(f) capture : le fond de la pilule a la couleur attendue (4 points sur 4)' ($vu.Fond -eq 4) $vu.Detail | Out-Null
            Verifier '(f) capture : hors de la pilule, pas cette couleur' (-not $vu.Dehors) $vu.Detail | Out-Null
            Fermer-Cas $p $df '(f)'
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
