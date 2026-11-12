# Étape 7 : la capture plein écran, de bout en bout (ARCHITECTURE.md §3.7), sur le vrai
# .exe, avec de vraies entrées clavier.
#
# (1) Démarrage : « raccourci plein_ecran: Ctrl+Maj+2 », aucune annonce. (2) Souris sur
# l'écran principal, Ctrl+Maj+2 : Repos -> Gel, Gel -> Decoupe (PhotoPrise plein_ecran),
# « plein ecran … souris », Decoupe -> Repos, « Copié et enregistré » (réglage
# d'enregistrement faux, vignette absente : enregistré quand même, réglages inchangés).
# (3) Pour cette capture, aucune ligne « ecran fige », « selection » ni « echap global », et
# ni l'écran figé ni la sélection ne deviennent visibles (guettés pendant et après).
# (4) Le PNG enregistré et l'image du presse-papiers ont exactement la taille de l'écran
# principal en pixels réels, et la fenêtre témoin y est à sa place, aux bonnes couleurs.
# (5) Deux captures de suite, la seconde pendant le bandeau de la première : la zone du
# bandeau (« bandeau zone … ») est la même dans les deux images. (6) Un second appui
# pendant la capture est ignoré ; après, l'app capture de nouveau. (7) --quitter → arret
# propre, aucune instance BreezyShot restée en vie.
#
# Ce que la machine GitHub ne peut pas vérifier : le choix entre plusieurs écrans (un seul
# écran ; prouvé seulement par les tests purs de `capture::ecran_de_capture`), des
# écrans d'échelles différentes, et la capture lancée depuis le menu de l'icône (aucun
# clic sur l'icône : le délai ATTENTE_MENU n'est pas essayé).
#
# Tout l'essai tourne avec BREEZYSHOT_ESSAI_CAPTURABLE=1 : le bandeau n'est pas exclu des
# captures, sinon le point (5) passerait sans rien prouver.
# Hors Windows : seul le démarrage est vérifié.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '05-plein-ecran' -Exe $Exe -Sortie $Sortie | Out-Null

$script:Auxiliaires = [System.Collections.Generic.List[object]]::new()
$script:Dossiers = [System.Collections.Generic.List[string]]::new()
$script:Pwsh = (Get-Process -Id $PID).Path

# --- Aides (même forme que l'essai 04) ---------------------------------------------------

function Dossier-Cas([string]$Nom) {
    $d = Join-Path $script:Essai.Dossier $Nom
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $script:Dossiers.Add($d)
    return $d
}

function Lignes-Journal([string]$Dossier) {
    return @((Lire-Journal -Dossier $Dossier) -split "`n" | ForEach-Object { $_.TrimEnd("`r") } | Where-Object { $_ -ne '' })
}

function Derniere-Ligne([string]$Motif, [string]$Dossier) {
    $trouvee = $null
    foreach ($l in (Lignes-Journal $Dossier)) { if ($l -match $Motif) { $trouvee = $l } }
    return $trouvee
}

function Compter-Motif([string]$Motif, [string]$Dossier) {
    $n = 0
    foreach ($l in (Lignes-Journal $Dossier)) { if ($l -match $Motif) { $n++ } }
    return $n
}

# Attend qu'au moins $Fois lignes correspondent à $Motif ; rend la dernière, ou lève une
# erreur qui dit quoi, quel motif et combien de fois.
function Attendre-Motif([string]$Motif, [string]$Dossier, [double]$DelaiS, [string]$Quoi, [int]$Fois = 1) {
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        if ((Compter-Motif -Motif $Motif -Dossier $Dossier) -ge $Fois) {
            return (Derniere-Ligne -Motif $Motif -Dossier $Dossier)
        }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $limite)
    $vu = Compter-Motif -Motif $Motif -Dossier $Dossier
    throw "$Quoi : rien dans le journal en $DelaiS s (motif « $Motif », attendu $Fois fois, vu $vu)"
}

# Les lignes du journal à partir de la n-ième (comptée depuis 0).
function Lignes-Depuis([string]$Dossier, [int]$Debut) {
    $toutes = @(Lignes-Journal $Dossier)
    if ($Debut -ge $toutes.Count) { return @() }
    return @($toutes[$Debut..($toutes.Count - 1)])
}

# Réglages du cas : Ctrl+Maj+1 pour la zone, Ctrl+Maj+2 pour le plein écran (pas
# d'Impr. écran, pas d'annonce au démarrage), copie, enregistrement faux (le défaut).
function Ecrire-Reglages([string]$Dossier) {
    $r = [ordered]@{
        version                   = 1
        raccourcis                = [ordered]@{ zone = @('Ctrl+Maj+1'); plein_ecran = @('Ctrl+Maj+2'); fenetre = @() }
        copier_apres_capture      = $true
        enregistrer_apres_capture = $false
        dossier_enregistrement    = $null
    }
    $d = Join-Path $Dossier 'reglages'
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $r | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $d 'reglages.json') -Encoding utf8
}

function Demarrer-Cas([string]$Dossier, [string]$Quoi) {
    $env:BREEZYSHOT_ESSAI_CAPTURABLE = '1'
    try { $p = Demarrer-App -Dossier $Dossier }
    finally { Remove-Item Env:BREEZYSHOT_ESSAI_CAPTURABLE -ErrorAction SilentlyContinue }
    $null = Attendre-Motif -Motif 'application +pret$' -Dossier $Dossier -DelaiS 30 -Quoi "$Quoi : démarrage"
    $null = Attendre-Motif -Motif 'ecran fige pret$' -Dossier $Dossier -DelaiS 1 -Quoi "$Quoi : écran figé préparé"
    $null = Attendre-Motif -Motif 'selection prete a servir$' -Dossier $Dossier -DelaiS 1 -Quoi "$Quoi : sélection créée"
    $null = Attendre-Motif -Motif 'bandeau pret$' -Dossier $Dossier -DelaiS 20 -Quoi "$Quoi : page du bandeau"
    $null = Attendre-Motif -Motif 'selection chargee$' -Dossier $Dossier -DelaiS 20 -Quoi "$Quoi : page de la sélection"
    return $p
}

# (7) --quitter → arret propre, processus sorti, aucune fenêtre BreezyShot restante.
function Fermer-Cas($Processus, [string]$Dossier, [string]$Quoi) {
    $arrets = Compter-Motif -Motif 'arret propre$' -Dossier $Dossier
    $q = Quitter-App -DelaiS 10 -Dossier $Dossier
    if (-not $q.Fini) { throw "$Quoi : --quitter n'a pas répondu en 10 s" }
    if (-not $Processus.WaitForExit(10000)) { throw "$Quoi : l'app ne s'est pas fermée en 10 s" }
    $null = Attendre-Motif -Motif 'arret propre$' -Dossier $Dossier -DelaiS 3 -Quoi "$Quoi : arrêt" -Fois ($arrets + 1)
    Verifier "(7) $Quoi : --quitter → arret propre" $true | Out-Null
    if ($IsWindows) {
        # Windows détruit les fenêtres d'un processus qui sort : ceci prouve seulement
        # qu'aucune autre instance n'est restée en vie avec un écran figé (EnumWindows).
        $figees = [BreezyShotEssai.PleinEcran]::FenetresDeClasse('BreezyShotEcranFige')
        $restes = [BreezyShotEssai.PleinEcran]::FenetresDe([int[]]@($Processus.Id))
        Verifier "(7) $Quoi : aucune instance BreezyShot restée en vie (toutes les fenêtres énumérées)" (($figees.Length -eq 0) -and ($restes.Length -eq 0)) "écrans figés $($figees.Length), fenêtres du PID $($Processus.Id) : $($restes.Length)" | Out-Null
    }
}

try {
    if (-not $IsWindows) {
        # --- Hors Windows : démarrage seulement --------------------------------------------
        $commun = Dossier-Cas 'commun'
        Ecrire-Reglages -Dossier $commun
        $p = Demarrer-Cas -Dossier $commun -Quoi 'commun'
        Verifier '(1) ligne « raccourci plein_ecran: … »' ((Compter-Lignes -Texte 'raccourci plein_ecran: ' -Dossier $commun) -eq 1) | Out-Null
        Fermer-Cas $p $commun 'commun'
        Write-Host '[saute] (2) à (6) : sautés hors Windows'
    }
    else {
        # --- Windows : entrées, fenêtres, presse-papiers, fichiers ------------------------
        Add-Type -Namespace BreezyShotEssai -Name PleinEcran -MemberDefinition @'
[DllImport("user32.dll")]
static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
[DllImport("user32.dll")]
static extern uint MapVirtualKey(uint code, uint type);
[DllImport("user32.dll")]
static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
[DllImport("user32.dll")]
public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SetProcessDpiAwarenessContext(IntPtr contexte);
[DllImport("user32.dll")]
public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")]
public static extern int GetSystemMetrics(int index);
[DllImport("user32.dll")]
public static extern uint GetClipboardSequenceNumber();
[DllImport("user32.dll")]
static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
delegate bool Rappel(IntPtr h, IntPtr p);
[DllImport("user32.dll")]
static extern bool EnumWindows(Rappel f, IntPtr p);
[DllImport("user32.dll", CharSet = CharSet.Unicode)]
static extern int GetClassName(IntPtr h, System.Text.StringBuilder nom, int taille);

const uint RELACHEE = 0x2;
static void Bas(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), 0u, UIntPtr.Zero); }
static void Haut(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), RELACHEE, UIntPtr.Zero); }
// Appuie les modificateurs, puis la touche $fois fois (relâchée et rappuyée, $ecart ms
// entre deux appuis), puis relâche tout.
public static void Appuyer(byte[] mods, byte touche, int fois, int ecart) {
    foreach (var m in mods) Bas(m);
    for (int i = 0; i < fois; i++) {
        if (i > 0) System.Threading.Thread.Sleep(ecart);
        Bas(touche);
        System.Threading.Thread.Sleep(30);
        Haut(touche);
    }
    for (int i = mods.Length - 1; i >= 0; i--) Haut(mods[i]);
}
// Pose le curseur et envoie un vrai mouvement d'entrée (relatif nul) à cet endroit.
public static void Poser(int x, int y) {
    SetCursorPos(x, y);
    mouse_event(0x1, 0, 0, 0, UIntPtr.Zero);
}
// Appuie (comme Appuyer) en guettant les fenêtres données, toutes les 2 ms, depuis avant
// l'appui jusqu'à $dureeMs après : rend le nombre de relevés où l'une était visible.
public static int AppuyerEnGuettant(byte[] mods, byte touche, int fois, int ecart, IntPtr[] guettees, int dureeMs) {
    int vues = 0;
    bool fin = false;
    var guet = new System.Threading.Thread(() => {
        while (!System.Threading.Volatile.Read(ref fin)) {
            foreach (var h in guettees) if (IsWindowVisible(h)) { System.Threading.Interlocked.Increment(ref vues); break; }
            System.Threading.Thread.Sleep(2);
        }
    });
    guet.IsBackground = true;
    guet.Start();
    System.Threading.Thread.Sleep(20);
    Appuyer(mods, touche, fois, ecart);
    System.Threading.Thread.Sleep(dureeMs);
    System.Threading.Volatile.Write(ref fin, true);
    guet.Join();
    return vues;
}
// Fenêtres de premier niveau des processus donnés.
public static IntPtr[] FenetresDe(int[] pids) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    var ensemble = new System.Collections.Generic.HashSet<int>(pids);
    EnumWindows((h, p) => {
        uint pid; GetWindowThreadProcessId(h, out pid);
        if (ensemble.Contains((int)pid)) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
// Toutes les fenêtres de haut niveau (de tous les processus) de cette classe.
public static IntPtr[] FenetresDeClasse(string classe) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, p) => {
        var nom = new System.Text.StringBuilder(256);
        if (GetClassName(h, nom, nom.Capacity) > 0 && nom.ToString() == classe) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
// Pixels différents entre deux images (octets BGRA de LockBits, Format32bppArgb) dans le
// rectangle (x, y, l, h), déjà borné aux deux images.
public static int Differences(byte[] a, int sa, byte[] b, int sb, int x, int y, int l, int h) {
    int n = 0;
    for (int j = y; j < y + h; j++) {
        for (int i = x; i < x + l; i++) {
            int ia = j * sa + 4 * i, ib = j * sb + 4 * i;
            if (a[ia] != b[ib] || a[ia + 1] != b[ib + 1] || a[ia + 2] != b[ib + 2]) n++;
        }
    }
    return n;
}
'@
        try {
            $dpiOk = [BreezyShotEssai.PleinEcran]::SetProcessDpiAwarenessContext([IntPtr]::new(-4))
            Write-Host "SetProcessDpiAwarenessContext(-4) : $(if ($dpiOk) { 'ok' } else { "refusé ($([System.Runtime.InteropServices.Marshal]::GetLastWin32Error()))" })"
        } catch { Write-Host "SetProcessDpiAwarenessContext(-4) indisponible : $_" }
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms

        $MODS = [byte[]]@(0x11, 0x10)   # Ctrl, Maj
        $TOUCHE_2 = [byte]0x32
        $COTE = 200
        # Haut gauche, haut droite, bas gauche, bas droite (r, v, b).
        $COULEURS = @(@(220, 40, 40), @(40, 180, 60), @(40, 80, 220), @(240, 200, 30))
        $texteCouleurs = '220,40,40;40,180,60;40,80,220;240,200,30'

        # L'écran principal en pixels réels, par deux chemins qui doivent s'accorder.
        $principal = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        $cx = [BreezyShotEssai.PleinEcran]::GetSystemMetrics(0)   # SM_CXSCREEN
        $cy = [BreezyShotEssai.PleinEcran]::GetSystemMetrics(1)   # SM_CYSCREEN
        $utile = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        Write-Host "écran principal $principal ; GetSystemMetrics ${cx}x$cy ; zone utile $utile"
        Verifier 'écran principal : Screen et GetSystemMetrics donnent la même taille' (($principal.Width -eq $cx) -and ($principal.Height -eq $cy)) "Screen $($principal.Width)x$($principal.Height), GetSystemMetrics ${cx}x$cy" | Out-Null
        $TAILLE = "$($principal.Width)x$($principal.Height)"

        # Le témoin : en haut au milieu de la zone utile de l'écran principal, sous la place
        # de la pilule du bandeau (64 px du haut), pour que le point (5) prouve quelque chose.
        $TX = [int]($utile.X + ($utile.Width - $COTE) / 2)
        $TY = [int]($utile.Y + 40)

        function Lancer-Temoin {
            $f = Join-Path $script:Essai.Dossier ("temoin-" + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.txt')
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -ArgumentList @(
                '-NoProfile', '-File', (Join-Path $PSScriptRoot 'temoin.ps1'),
                '-X', $TX, '-Y', $TY, '-Cote', $COTE, '-Couleurs', $texteCouleurs, '-Fichier', $f)
            $script:Auxiliaires.Add($aide)
            $limite = [DateTime]::UtcNow.AddSeconds(30)
            while (-not (Test-Path -LiteralPath $f)) {
                if ([DateTime]::UtcNow -ge $limite) { throw "fenêtre témoin : aucun HWND en 30 s" }
                Start-Sleep -Milliseconds 100
            }
            Start-Sleep -Milliseconds 500   # composée à l'écran
            return [IntPtr][long](Get-Content -LiteralPath $f -Raw).Trim()
        }

        function Hwnd-Journal([string]$Motif, [string]$Dossier, [string]$Quoi) {
            $l = Attendre-Motif -Motif "$Motif (\d+)$" -Dossier $Dossier -DelaiS 5 -Quoi $Quoi
            if ($l -notmatch "$Motif (\d+)$") { throw "$Quoi : ligne illisible : $l" }
            return [IntPtr][long]$Matches[1]
        }

        function Visible([IntPtr]$h) { return [BreezyShotEssai.PleinEcran]::IsWindowVisible($h) }

        # Octets BGRA d'une image (Format32bppArgb).
        function Octets-Image([System.Drawing.Bitmap]$Image) {
            $r = [System.Drawing.Rectangle]::new(0, 0, $Image.Width, $Image.Height)
            $d = $Image.LockBits($r, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
            try {
                $n = $d.Stride * $Image.Height
                $o = [byte[]]::new($n)
                [System.Runtime.InteropServices.Marshal]::Copy($d.Scan0, $o, 0, $n)
                return [pscustomobject]@{ Octets = $o; Stride = $d.Stride }
            }
            finally { $Image.UnlockBits($d) }
        }

        # Le presse-papiers, lu dans un fil STA (seul permis par Windows). $null si vide.
        function Lire-PressePapiers {
            $rs = [runspacefactory]::CreateRunspace()
            $rs.ApartmentState = [System.Threading.ApartmentState]::STA
            $rs.ThreadOptions = [System.Management.Automation.Runspaces.PSThreadOptions]::ReuseThread
            $rs.Open()
            try {
                $ps = [powershell]::Create()
                $ps.Runspace = $rs
                [void]$ps.AddScript({
                        Add-Type -AssemblyName System.Windows.Forms
                        Add-Type -AssemblyName System.Drawing
                        for ($i = 0; $i -lt 10; $i++) {
                            try {
                                $img = [System.Windows.Forms.Clipboard]::GetImage()
                                if ($null -ne $img) {
                                    $copie = [System.Drawing.Bitmap]::new($img)
                                    $img.Dispose()
                                    return , $copie
                                }
                            } catch { }
                            Start-Sleep -Milliseconds 100
                        }
                        return $null
                    })
                try { $r = $ps.Invoke() }
                finally { $ps.Dispose() }
                foreach ($x in $r) {
                    if ($null -ne $x) {
                        $b = $x.psobject.BaseObject
                        if ($b -is [System.Drawing.Bitmap]) { return $b }
                    }
                }
                return $null
            }
            finally { $rs.Dispose() }
        }

        # Une fonction rend ses éléments un à un : un seul devient un scalaire, aucun devient
        # rien, et sous StrictMode `.Count` lève sur les deux. Chaque appel est entouré de @().
        function Fichiers-Png([string]$Dossier) {
            if (-not (Test-Path -LiteralPath $Dossier)) { return @() }
            return @(Get-ChildItem -LiteralPath $Dossier -File -Filter '*.png' | Sort-Object LastWriteTimeUtc, Name)
        }

        function Lire-Png([string]$Chemin) {
            $octets = [System.IO.File]::ReadAllBytes($Chemin)
            $flux = [System.IO.MemoryStream]::new($octets)
            try {
                $img = [System.Drawing.Bitmap]::new($flux)
                try { return [System.Drawing.Bitmap]::new($img) }
                finally { $img.Dispose() }
            }
            finally { $flux.Dispose() }
        }

        # « plein ecran x,y LxH origine » → { X, Y, L, H, Origine } ; $null si illisible.
        function Lire-PleinEcran([string]$Ligne) {
            if ($Ligne -notmatch 'plein ecran (-?\d+),(-?\d+) (\d+)x(\d+) (\w+)$') { return $null }
            return [pscustomobject]@{ X = [int]$Matches[1]; Y = [int]$Matches[2]; L = [int]$Matches[3]; H = [int]$Matches[4]; Origine = $Matches[5] }
        }

        # (4) Taille exacte de l'écran principal, puis la fenêtre témoin à sa place (points
        # pris dans chaque carré, à 3 px de ses bords, et au centre), aux bonnes couleurs.
        # $Ecran : l'écran capturé (origine de l'image en coordonnées du bureau).
        function Verifier-Image([string]$Nom, $Image, $Ecran) {
            if ($null -eq $Image) { Verifier $Nom $false 'aucune image' | Out-Null; return }
            if ($null -eq $Ecran) { Verifier $Nom $false 'ligne « plein ecran » illisible' | Out-Null; return }
            $taille = "$($Image.Width)x$($Image.Height)"
            if (-not (Verifier "$Nom : taille de l'écran principal ($TAILLE)" ($taille -eq $TAILLE) "obtenu $taille")) { return }
            $moitie = [int]($COTE / 2)
            $ecarts = [System.Collections.Generic.List[string]]::new()
            $places = @(@(0, 0), @($moitie, 0), @(0, $moitie), @($moitie, $moitie))
            for ($q = 0; $q -lt 4; $q++) {
                $bx = $TX - $Ecran.X + $places[$q][0]; $by = $TY - $Ecran.Y + $places[$q][1]
                $points = @(@(3, 3), @(($moitie - 4), 3), @(3, ($moitie - 4)), @(($moitie - 4), ($moitie - 4)), @([int]($moitie / 2), [int]($moitie / 2)))
                foreach ($pt in $points) {
                    $x = $bx + $pt[0]; $y = $by + $pt[1]
                    $c = $Image.GetPixel($x, $y)
                    $attendu = $COULEURS[$q]
                    if (($c.R -ne $attendu[0]) -or ($c.G -ne $attendu[1]) -or ($c.B -ne $attendu[2])) {
                        $ecarts.Add("$x,$y attendu $($attendu -join ',') obtenu $($c.R),$($c.G),$($c.B)")
                    }
                }
            }
            $ok = $ecarts.Count -eq 0
            if (-not $ok) {
                $Image.Save((Join-Path $script:Essai.Sortie ("05-" + ($Nom -replace '[^\w]+', '-') + '.png')), [System.Drawing.Imaging.ImageFormat]::Png)
            }
            Verifier "$Nom : la fenêtre témoin est à sa place, aux bonnes couleurs" $ok (@($ecarts | Select-Object -First 8) -join ' ; ') | Out-Null
        }

        # Une capture plein écran par Ctrl+Maj+2, en guettant l'écran figé et la sélection.
        # Rend { Lignes (journal de cette capture), Vues (relevés où l'une était visible),
        # Ecran, Fichier (dernier PNG), Seq (presse-papiers avant) }.
        function Capturer-PleinEcran([string]$Dossier, [string]$Quoi, [IntPtr[]]$Guettees, [int]$Fois = 1) {
            $debut = @(Lignes-Journal $Dossier).Count
            $annonces = Compter-Motif -Motif 'sortie annonce ' -Dossier $Dossier
            $seq = [BreezyShotEssai.PleinEcran]::GetClipboardSequenceNumber()
            $vues = [BreezyShotEssai.PleinEcran]::AppuyerEnGuettant($MODS, $TOUCHE_2, $Fois, 30, $Guettees, 300)
            $null = Attendre-Motif -Motif 'sortie annonce ' -Dossier $Dossier -DelaiS 10 -Quoi "$Quoi : annonce de la sortie" -Fois ($annonces + 1)
            $null = Attendre-Motif -Motif 'chrono decoupe \d+ sortie \d+ relache-annonce \d+$' -Dossier $Dossier -DelaiS 3 -Quoi "$Quoi : fin de la sortie" -Fois ($annonces + 1)
            # Après : toujours rien de visible.
            Start-Sleep -Milliseconds 300
            foreach ($h in $Guettees) { if (Visible $h) { $vues++ } }
            $lignes = @(Lignes-Depuis $Dossier $debut)
            $ligneEcran = $null
            foreach ($l in $lignes) { if ($l -match 'plein ecran ') { $ligneEcran = $l } }
            return [pscustomobject]@{ Lignes = $lignes; Vues = $vues; Ecran = (Lire-PleinEcran $ligneEcran); LigneEcran = $ligneEcran; Seq = $seq }
        }

        try {
            $temoin = Lancer-Temoin
            Write-Host "témoin $temoin à $TX,$TY ${COTE}x$COTE"

            $da = Dossier-Cas 'a'
            Ecrire-Reglages -Dossier $da
            $p = Demarrer-Cas -Dossier $da -Quoi '(A)'
            try {
                $cheminReglages = Join-Path $da 'reglages/reglages.json'
                $reglagesAvant = [System.IO.File]::ReadAllBytes($cheminReglages)
                $images = Join-Path $da 'images'

                # (1) Démarrage.
                Verifier '(1) raccourci plein_ecran: Ctrl+Maj+2' ((Compter-Lignes -Texte 'raccourci plein_ecran: Ctrl+Maj+2' -Dossier $da) -eq 1) | Out-Null
                Verifier '(1) raccourci zone: Ctrl+Maj+1' ((Compter-Lignes -Texte 'raccourci zone: Ctrl+Maj+1' -Dossier $da) -eq 1) | Out-Null
                Verifier '(1) aucune annonce au démarrage (le bandeau est vide avant la première photo)' ((Compter-Lignes -Texte 'annonce raccourci' -Dossier $da) -eq 0) | Out-Null
                $sel = Hwnd-Journal 'selection fenetre' $da 'HWND de la sélection'
                $fige = Hwnd-Journal 'ecran fige fenetre' $da 'HWND de l''écran figé'
                $bandeau = Hwnd-Journal 'bandeau fenetre' $da 'HWND du bandeau'
                $guettees = [IntPtr[]]@($fige, $sel)

                # (2) Souris sur l'écran principal, loin du témoin et du bandeau.
                [BreezyShotEssai.PleinEcran]::Poser([int]($utile.X + $utile.Width / 2), [int]($utile.Y + $utile.Height - 120))
                Start-Sleep -Milliseconds 200
                $c1 = Capturer-PleinEcran -Dossier $da -Quoi '(2) première capture' -Guettees $guettees
                $t1 = $c1.Lignes -join "`n"
                Verifier '(2) etat Repos -> Gel (DemandeCapture plein_ecran)' ($t1.Contains('etat Repos -> Gel (DemandeCapture plein_ecran)')) | Out-Null
                Verifier '(2) etat Gel -> Decoupe (PhotoPrise plein_ecran)' ($t1.Contains('etat Gel -> Decoupe (PhotoPrise plein_ecran)')) | Out-Null
                Verifier '(2) plein ecran … souris : l''écran principal' (($null -ne $c1.Ecran) -and ($c1.Ecran.Origine -eq 'souris') -and ($c1.Ecran.X -eq $principal.X) -and ($c1.Ecran.Y -eq $principal.Y) -and ("$($c1.Ecran.L)x$($c1.Ecran.H)" -eq $TAILLE)) "$($c1.LigneEcran)" | Out-Null
                Verifier '(2) chrono gel photo … total … (sans visible)' (@($c1.Lignes | Where-Object { $_ -match 'chrono gel photo \d+ total \d+$' }).Count -eq 1) | Out-Null
                Verifier '(2) etat Decoupe -> Repos (ImagePreteSansVignette)' ($t1.Contains('etat Decoupe -> Repos (ImagePreteSansVignette)')) | Out-Null
                Verifier '(2) sortie annonce reussite: Copié et enregistré' ($t1.Contains('sortie annonce reussite: Copié et enregistré')) | Out-Null
                Verifier '(2) sortie copie faite recent ok fichier fait' ($t1.Contains('sortie copie faite recent ok fichier fait')) | Out-Null
                Verifier '(2) aucune « capture impossible »' (-not $t1.Contains('capture impossible')) | Out-Null

                # (3) Rien de montré.
                $montrees = @($c1.Lignes | Where-Object { $_ -match ' (ecran fige|selection|echap global)' })
                Verifier '(3) aucune ligne « ecran fige », « selection » ni « echap global » pour cette capture' ($montrees.Count -eq 0) ($montrees -join ' ; ') | Out-Null
                Verifier '(3) écran figé et sélection jamais visibles (guettés pendant et après)' ($c1.Vues -eq 0) "$($c1.Vues) relevé(s) visible(s)" | Out-Null

                # (4) Le fichier et le presse-papiers.
                $pngs = @(Fichiers-Png $images)
                Verifier '(4) un PNG dans le dossier Images › BreezyShot de l''essai' ($pngs.Count -eq 1) "$($pngs.Count) fichier(s)" | Out-Null
                $image1 = $null
                if ($pngs.Count -ge 1) {
                    $image1 = Lire-Png $pngs[-1].FullName
                    Verifier-Image '(4) PNG enregistré' $image1 $c1.Ecran
                }
                Verifier '(4) le presse-papiers a changé' ([BreezyShotEssai.PleinEcran]::GetClipboardSequenceNumber() -ne $c1.Seq) | Out-Null
                $copie = Lire-PressePapiers
                Verifier-Image '(4) presse-papiers' $copie $c1.Ecran
                if ($null -ne $copie) { $copie.Dispose() }
                $reglagesApres = [System.IO.File]::ReadAllBytes($cheminReglages)
                Verifier '(2) réglages non réécrits (enregistrement automatique sans toucher au fichier)' ([Convert]::ToBase64String($reglagesAvant) -eq [Convert]::ToBase64String($reglagesApres)) | Out-Null

                # (5) La seconde pendant le bandeau de la première.
                $aff = Attendre-Motif -Motif 'bandeau affiche (\d+) reussite ' -Dossier $da -DelaiS 5 -Quoi '(5) bandeau de la première'
                $null = $aff -match 'bandeau affiche (\d+) reussite '
                $idBandeau = $Matches[1]
                $lz = Attendre-Motif -Motif "bandeau zone $idBandeau (-?\d+),(-?\d+) (\d+)x(\d+)$" -Dossier $da -DelaiS 3 -Quoi '(5) zone du bandeau'
                $null = $lz -match "bandeau zone $idBandeau (-?\d+),(-?\d+) (\d+)x(\d+)$"
                $zb = [pscustomobject]@{ X = [int]$Matches[1]; Y = [int]$Matches[2]; L = [int]$Matches[3]; H = [int]$Matches[4] }
                Start-Sleep -Milliseconds 400   # pilule entrée (300 ms), encore là (2,5 s)
                Verifier '(5) le bandeau de la première est à l''écran' (Visible $bandeau) | Out-Null
                # La zone du bandeau, bornée à l'image (coordonnées de l'image).
                $ex = 0; $ey = 0
                if ($null -ne $c1.Ecran) { $ex = $c1.Ecran.X; $ey = $c1.Ecran.Y }
                $zx = [math]::Max(0, $zb.X - $ex); $zy = [math]::Max(0, $zb.Y - $ey)
                $zl = [math]::Min($principal.Width, $zb.X - $ex + $zb.L) - $zx
                $zh = [math]::Min($principal.Height, $zb.Y - $ey + $zb.H) - $zy
                Write-Host "(5) zone du bandeau $($zb.X),$($zb.Y) $($zb.L)x$($zb.H) ; dans l'image $zx,$zy ${zl}x$zh"
                # Témoin de la preuve : le bandeau est réellement peint à l'écran à cet endroit
                # (une copie d'écran y diffère de la première image).
                if (($null -ne $image1) -and ($zl -gt 0) -and ($zh -gt 0)) {
                    $ecranCopie = [System.Drawing.Bitmap]::new($principal.Width, $principal.Height, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
                    $g = [System.Drawing.Graphics]::FromImage($ecranCopie)
                    try { $g.CopyFromScreen($principal.X, $principal.Y, 0, 0, $ecranCopie.Size) }
                    finally { $g.Dispose() }
                    $oa = Octets-Image $image1; $oc = Octets-Image $ecranCopie
                    $peints = [BreezyShotEssai.PleinEcran]::Differences($oa.Octets, $oa.Stride, $oc.Octets, $oc.Stride, $zx, $zy, $zl, $zh)
                    $ecranCopie.Dispose()
                    Verifier '(5) témoin de la preuve : le bandeau est peint à l''écran, dans sa zone' ($peints -gt 0) "$peints pixel(s) différents de la première image" | Out-Null
                }
                $c2 = Capturer-PleinEcran -Dossier $da -Quoi '(5) seconde capture' -Guettees $guettees
                Verifier '(5) seconde capture : Copié et enregistré, rien de visible' ((($c2.Lignes -join "`n").Contains('sortie annonce reussite: Copié et enregistré')) -and ($c2.Vues -eq 0)) "$($c2.Vues) relevé(s) visible(s)" | Out-Null
                $pngs = @(Fichiers-Png $images)
                Verifier '(5) un second PNG' ($pngs.Count -eq 2) "$($pngs.Count) fichier(s)" | Out-Null
                if (($pngs.Count -ge 2) -and ($null -ne $image1) -and ($zl -gt 0) -and ($zh -gt 0)) {
                    $image2 = Lire-Png $pngs[-1].FullName
                    Verifier-Image '(5) second PNG' $image2 $c2.Ecran
                    if ("$($image2.Width)x$($image2.Height)" -eq $TAILLE) {
                        $oa = Octets-Image $image1; $ob = Octets-Image $image2
                        $diff = [BreezyShotEssai.PleinEcran]::Differences($oa.Octets, $oa.Stride, $ob.Octets, $ob.Stride, $zx, $zy, $zl, $zh)
                        if ($diff -ne 0) { $image2.Save((Join-Path $script:Essai.Sortie '05-seconde-image.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
                        Verifier '(5) zone du bandeau : la seconde image est identique à la première (bandeau absent)' ($diff -eq 0) "$diff pixel(s) différents sur ${zl}x$zh" | Out-Null
                    }
                    $image2.Dispose()
                }
                else {
                    Verifier '(5) comparaison des deux images possible' $false "PNG $($pngs.Count), première image $($null -ne $image1), zone ${zl}x$zh" | Out-Null
                }
                if ($null -ne $image1) { $image1.Dispose() }

                # (6) Deux appuis à 30 ms : le second arrive pendant la capture, ignoré.
                $gels = Compter-Motif -Motif 'etat Repos -> Gel' -Dossier $da
                $ignores = Compter-Motif -Motif 'capture deja en cours: appui ignore$' -Dossier $da
                $nPng = @(Fichiers-Png $images).Count
                $c3 = Capturer-PleinEcran -Dossier $da -Quoi '(6) double appui' -Guettees $guettees -Fois 2
                $null = Attendre-Motif -Motif 'capture deja en cours: appui ignore$' -Dossier $da -DelaiS 3 -Quoi '(6) second appui' -Fois ($ignores + 1)
                Start-Sleep -Milliseconds 500
                $gelsApres = Compter-Motif -Motif 'etat Repos -> Gel' -Dossier $da
                $pngApres = @(Fichiers-Png $images).Count
                Verifier '(6) second appui ignoré : une seule capture (un Gel, un PNG de plus)' (($gelsApres -eq $gels + 1) -and ($pngApres -eq $nPng + 1) -and ($c3.Vues -eq 0)) "Gel $gels → $gelsApres, PNG $nPng → $pngApres" | Out-Null
                $c4 = Capturer-PleinEcran -Dossier $da -Quoi '(6) capture suivante' -Guettees $guettees
                Verifier '(6) après, l''app capture de nouveau' ((($c4.Lignes -join "`n").Contains('etat Decoupe -> Repos (ImagePreteSansVignette)')) -and (@(Fichiers-Png $images).Count -eq $pngApres + 1)) | Out-Null

                Verifier '(A) aucune garde déclenchée, aucune erreur de capture' (((Compter-Lignes -Texte 'arret force' -Dossier $da) -eq 0) -and ((Compter-Lignes -Texte 'capture impossible' -Dossier $da) -eq 0)) | Out-Null
            }
            finally {
                Fermer-Cas $p $da '(A)'
            }
        }
        finally {
            foreach ($a in $script:Auxiliaires) {
                try { if (-not $a.HasExited) { $a.Kill(); $null = $a.WaitForExit(5000) } } catch { }
            }
            Arreter-Tout
        }
    }
}
catch {
    Verifier "déroulement de l'essai" $false "$_" | Out-Null
}
finally {
    foreach ($d in $script:Dossiers) {
        $chronos = @(Lignes-Journal $d | Where-Object { $_ -match ' chrono ' })
        if ($chronos.Count -gt 0) {
            Write-Host "--- chronos du cas $(Split-Path -Leaf $d) ---"
            $chronos | ForEach-Object { Write-Host $_ }
        }
    }
    if (@($script:Verifications | Where-Object { -not $_.Reussi }).Count -gt 0) {
        foreach ($d in $script:Dossiers) {
            Write-Host "--- fin du journal du cas $(Split-Path -Leaf $d) ---"
            Lignes-Journal $d | Select-Object -Last 30 | ForEach-Object { Write-Host $_ }
        }
    }
    foreach ($d in $script:Dossiers) {
        $source = Join-Path $d 'local/Journal'
        if (Test-Path -LiteralPath $source) {
            $cible = Join-Path $script:Essai.Sortie ("journaux/05-plein-ecran-" + (Split-Path -Leaf $d))
            New-Item -ItemType Directory -Force -Path $cible | Out-Null
            Copy-Item -Path (Join-Path $source '*') -Destination $cible -Force
        }
    }
    Terminer-Essai
}
