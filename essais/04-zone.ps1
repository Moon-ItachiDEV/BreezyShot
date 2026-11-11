# Étapes 4 à 6 assemblées : la capture d'une zone, de bout en bout (ARCHITECTURE.md §3.6,
# §8, §9, §10), sur le vrai .exe, avec de vraies entrées clavier et souris.
#
# Cas A (copie seule) : (1) raccourci → chronos et lignes du journal ; (2) fenêtres
# pendant la sélection (couverture, ordre, premier plan, styles étendus, bandeau caché,
# pas de focus_perdu parasite en 3 s) ; (3) glisser exact sur les quatre carrés du témoin
# → presse-papiers et « Récents » identiques pixel par pixel, Repos, premier plan rendu ;
# (4) Échap puis clic droit annulent ; (5) clic sans glisser et glisser de 2 px : la
# sélection reste, puis Échap ; (6) second raccourci pendant la sélection ignoré ;
# (7) glisser dans les autres sens : même image ; (9) deux captures rapprochées : le
# bandeau de la première n'est ni dans l'image ni au-dessus de la sélection ; et, demandé
# par la relecture de l'étape 5 : F5 et Ctrl+P sans effet, Alt+F4 annule proprement,
# perte de focus annule, trois captures sans fuite de fenêtres ni de fils.
# Cas B : (8) enregistrement après capture. Cas C : (10) dossier impossible.
# (11) À chaque fin : --quitter → arret propre, aucune fenêtre BreezyShot ne reste.
#
# Ce que la machine GitHub ne peut pas vérifier : plusieurs écrans, et des écrans
# d'échelles différentes (un seul écran virtuel à 100 %) ; une application tierce figée
# (garde de 5 s du fil principal) ; un vrai verrouillage de session ou un écran branché
# pendant la sélection ; le ressenti de vitesse (les chronos sont notés, sans seuil).
#
# Tout l'essai tourne avec BREEZYSHOT_ESSAI_CAPTURABLE=1 : le bandeau et la sélection ne
# sont pas exclus des captures, sinon le point (9) passerait sans rien prouver.
# Hors Windows : seul le démarrage est vérifié (écran figé et sélection préparés).

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '04-zone' -Exe $Exe -Sortie $Sortie | Out-Null

$script:Auxiliaires = [System.Collections.Generic.List[object]]::new()
$script:Dossiers = [System.Collections.Generic.List[string]]::new()
$script:Pwsh = (Get-Process -Id $PID).Path

# --- Aides (même forme que l'essai 03) ---------------------------------------------------

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

# Réglages du cas : raccourci Ctrl+Maj+1 seul (pas d'Impr. écran, pas d'annonce au
# démarrage), copie, enregistrement et dossier au choix.
function Ecrire-Reglages([string]$Dossier, [bool]$Enregistrer, $DossierEnregistrement) {
    $r = [ordered]@{
        version                   = 1
        raccourcis                = [ordered]@{ zone = @('Ctrl+Maj+1'); plein_ecran = @(); fenetre = @() }
        copier_apres_capture      = $true
        enregistrer_apres_capture = $Enregistrer
        dossier_enregistrement    = $DossierEnregistrement
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

# (11) --quitter → arret propre, processus sorti, aucune fenêtre BreezyShot restante.
function Fermer-Cas($Processus, [string]$Dossier, [string]$Quoi) {
    $arrets = Compter-Motif -Motif 'arret propre$' -Dossier $Dossier
    $q = Quitter-App -DelaiS 10 -Dossier $Dossier
    if (-not $q.Fini) { throw "$Quoi : --quitter n'a pas répondu en 10 s" }
    if (-not $Processus.WaitForExit(10000)) { throw "$Quoi : l'app ne s'est pas fermée en 10 s" }
    $null = Attendre-Motif -Motif 'arret propre$' -Dossier $Dossier -DelaiS 3 -Quoi "$Quoi : arrêt" -Fois ($arrets + 1)
    Verifier "(11) $Quoi : --quitter → arret propre" $true | Out-Null
    if ($IsWindows) {
        $restes = [BreezyShotEssai.Zone]::FenetresDe([int[]]@($Processus.Id), $false)
        $figee = [BreezyShotEssai.Zone]::FindWindow('BreezyShotEcranFige', $null)
        Verifier "(11) $Quoi : aucune fenêtre BreezyShot ne reste" (($restes.Length -eq 0) -and ($figee -eq [IntPtr]::Zero)) "fenêtres du processus $($restes.Length), écran figé $figee" | Out-Null
    }
}

try {
    if (-not $IsWindows) {
        # --- Hors Windows : démarrage seulement --------------------------------------------
        $commun = Dossier-Cas 'commun'
        Ecrire-Reglages -Dossier $commun -Enregistrer $false -DossierEnregistrement $null
        $p = Demarrer-Cas -Dossier $commun -Quoi 'commun'
        Verifier 'écran figé et sélection préparés au démarrage' $true | Out-Null
        Fermer-Cas $p $commun 'commun'
        foreach ($cas in '(1) à (7), (9)', '(8) enregistrement', '(10) dossier impossible') {
            Write-Host "[saute] $cas : sauté hors Windows"
        }
    }
    else {
        # --- Windows : entrées, fenêtres, presse-papiers ---------------------------------
        Add-Type -Namespace BreezyShotEssai -Name Zone -MemberDefinition @'
[DllImport("user32.dll")]
static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
[DllImport("user32.dll")]
static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
[DllImport("user32.dll")]
public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SetProcessDpiAwarenessContext(IntPtr contexte);
[DllImport("user32.dll")]
public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")]
public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")]
static extern uint MapVirtualKey(uint code, uint type);
[DllImport("user32.dll")]
public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
public static extern IntPtr GetWindowLongPtr(IntPtr h, int index);
[DllImport("user32.dll")]
public static extern IntPtr GetWindow(IntPtr h, uint cmd);
[DllImport("user32.dll", CharSet = CharSet.Unicode)]
public static extern IntPtr FindWindow(string classe, string titre);
[DllImport("user32.dll")]
public static extern int GetSystemMetrics(int index);
[DllImport("user32.dll")]
public static extern uint GetClipboardSequenceNumber();
[StructLayout(LayoutKind.Sequential)]
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
[DllImport("user32.dll")]
public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll")]
static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
delegate bool Rappel(IntPtr h, IntPtr p);
[DllImport("user32.dll")]
static extern bool EnumWindows(Rappel f, IntPtr p);

const uint ETENDUE = 0x1, RELACHEE = 0x2;
static uint Drapeaux(byte vk) { return (vk == 0x2C || vk == 0x2E) ? ETENDUE : 0u; }
// Appuie les touches dans l'ordre, puis les relâche dans l'ordre inverse.
public static void Touches(byte[] vks) {
    foreach (var vk in vks) keybd_event(vk, (byte)MapVirtualKey(vk, 0), Drapeaux(vk), UIntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    for (int i = vks.Length - 1; i >= 0; i--) keybd_event(vks[i], (byte)MapVirtualKey(vks[i], 0), Drapeaux(vks[i]) | RELACHEE, UIntPtr.Zero);
}
// Pose le curseur et envoie un vrai mouvement d'entrée (relatif nul) à cet endroit.
static void Poser(int x, int y) {
    SetCursorPos(x, y);
    mouse_event(0x1, 0, 0, 0, UIntPtr.Zero);
}
// Glisser bouton gauche de (x1, y1) à (x2, y2) par 16 positions intermédiaires, en
// laissant à la page le temps de recevoir chaque mouvement.
public static void Glisser(int x1, int y1, int x2, int y2) {
    Poser(x1, y1);
    System.Threading.Thread.Sleep(120);
    mouse_event(0x2, 0, 0, 0, UIntPtr.Zero);
    System.Threading.Thread.Sleep(80);
    for (int i = 1; i <= 16; i++) {
        Poser(x1 + (x2 - x1) * i / 16, y1 + (y2 - y1) * i / 16);
        System.Threading.Thread.Sleep(30);
    }
    Poser(x2, y2);
    System.Threading.Thread.Sleep(150);
    mouse_event(0x4, 0, 0, 0, UIntPtr.Zero);
}
// Clic gauche (ou droit) sans déplacer la souris.
public static void Cliquer(int x, int y, bool droit) {
    Poser(x, y);
    System.Threading.Thread.Sleep(120);
    mouse_event(droit ? 0x8u : 0x2u, 0, 0, 0, UIntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    mouse_event(droit ? 0x10u : 0x4u, 0, 0, 0, UIntPtr.Zero);
}
// Fenêtres de premier niveau des processus donnés (visibles seulement si demandé).
public static IntPtr[] FenetresDe(int[] pids, bool visibles) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    var ensemble = new System.Collections.Generic.HashSet<int>(pids);
    EnumWindows((h, p) => {
        uint pid; GetWindowThreadProcessId(h, out pid);
        if (ensemble.Contains((int)pid) && (!visibles || IsWindowVisible(h))) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
// Compare une image (octets BGRA de LockBits, Format32bppArgb) aux quatre carrés
// attendus (couleurs r,v,b : haut gauche, haut droite, bas gauche, bas droite). Rend
// « total N » puis les $max premiers écarts « x,y attendu r,v,b obtenu r,v,b ».
public static string[] Ecarts(byte[] o, int stride, int l, int h, int[] c, int max) {
    var r = new System.Collections.Generic.List<string>();
    int total = 0, mx = l / 2, my = h / 2;
    for (int y = 0; y < h; y++) {
        for (int x = 0; x < l; x++) {
            int q = (y < my ? 0 : 2) + (x < mx ? 0 : 1);
            int i = y * stride + 4 * x;
            int b = o[i], g = o[i + 1], rr = o[i + 2];
            if (rr != c[3 * q] || g != c[3 * q + 1] || b != c[3 * q + 2]) {
                total++;
                if (r.Count < max) r.Add(string.Format("{0},{1} attendu {2},{3},{4} obtenu {5},{6},{7}", x, y, c[3 * q], c[3 * q + 1], c[3 * q + 2], rr, g, b));
            }
        }
    }
    r.Insert(0, "total " + total);
    return r.ToArray();
}
'@
        try {
            $dpiOk = [BreezyShotEssai.Zone]::SetProcessDpiAwarenessContext([IntPtr]::new(-4))
            Write-Host "SetProcessDpiAwarenessContext(-4) : $(if ($dpiOk) { 'ok' } else { "refusé ($([System.Runtime.InteropServices.Marshal]::GetLastWin32Error()))" })"
        } catch { Write-Host "SetProcessDpiAwarenessContext(-4) indisponible : $_" }
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms

        $CTRL_MAJ_1 = [byte[]]@(0x11, 0x10, 0x31)
        $ECHAP = [byte[]]@(0x1B)
        $GWL_EXSTYLE = -20
        $GW_HWNDNEXT = 2
        $COTE = 200
        # Haut gauche, haut droite, bas gauche, bas droite (r, v, b).
        $COULEURS = [int[]]@(220, 40, 40, 40, 180, 60, 40, 80, 220, 240, 200, 30)
        $texteCouleurs = '220,40,40;40,180,60;40,80,220;240,200,30'

        # Le témoin : en haut au milieu de la zone utile de l'écran principal, sous l'endroit
        # où le bandeau pose sa pilule (64 px du haut), pour que le point (9) prouve quelque
        # chose.
        $utile = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        $TX = [int]($utile.X + ($utile.Width - $COTE) / 2)
        $TY = [int]($utile.Y + 40)
        $bureau = [pscustomobject]@{
            X = [BreezyShotEssai.Zone]::GetSystemMetrics(76); Y = [BreezyShotEssai.Zone]::GetSystemMetrics(77)
            L = [BreezyShotEssai.Zone]::GetSystemMetrics(78); H = [BreezyShotEssai.Zone]::GetSystemMetrics(79)
        }
        Write-Host "bureau virtuel $($bureau.X),$($bureau.Y) $($bureau.L)x$($bureau.H) ; zone utile $utile ; témoin $TX,$TY ${COTE}x$COTE"

        function Lancer-Temoin {
            $f = Join-Path $script:Essai.Dossier ("temoin-" + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.txt')
            # Sans -WindowStyle Hidden : Windows appliquerait ce masquage au premier
            # ShowWindow du processus, c'est-à-dire à la fenêtre témoin elle-même.
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

        function Fermer-Auxiliaires {
            foreach ($a in $script:Auxiliaires) {
                try { if (-not $a.HasExited) { $a.Kill(); $null = $a.WaitForExit(5000) } } catch { }
            }
            $script:Auxiliaires.Clear()
        }

        function Hwnd-Journal([string]$Motif, [string]$Dossier, [string]$Quoi) {
            $l = Attendre-Motif -Motif "$Motif (\d+)$" -Dossier $Dossier -DelaiS 5 -Quoi $Quoi
            if ($l -notmatch "$Motif (\d+)$") { throw "$Quoi : ligne illisible : $l" }
            return [IntPtr][long]$Matches[1]
        }

        function Rect-Fenetre([IntPtr]$h) {
            $r = [BreezyShotEssai.Zone+RECT]::new()
            [void][BreezyShotEssai.Zone]::GetWindowRect($h, [ref]$r)
            return [pscustomobject]@{ X = $r.Left; Y = $r.Top; L = $r.Right - $r.Left; H = $r.Bottom - $r.Top }
        }

        function Texte-Rect($r) { return "$($r.X),$($r.Y),$($r.L),$($r.H)" }

        function Visible([IntPtr]$h) { return [BreezyShotEssai.Zone]::IsWindowVisible($h) }

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

        # Compare une image aux quatre carrés : taille exacte, puis chaque pixel. En cas
        # d'écart, les premiers pixels fautifs dans le détail.
        function Verifier-Image([string]$Nom, $Image) {
            if ($null -eq $Image) { Verifier $Nom $false 'aucune image' | Out-Null; return }
            $taille = "$($Image.Width)x$($Image.Height)"
            if (-not (Verifier "$Nom : taille ${COTE}x$COTE" (($Image.Width -eq $COTE) -and ($Image.Height -eq $COTE)) "obtenu $taille")) {
                $Image.Save((Join-Path $script:Essai.Sortie ("04-" + ($Nom -replace '[^\w]+', '-') + '.png')), [System.Drawing.Imaging.ImageFormat]::Png)
                return
            }
            $o = Octets-Image $Image
            $e = [BreezyShotEssai.Zone]::Ecarts($o.Octets, $o.Stride, $Image.Width, $Image.Height, $COULEURS, 8)
            $ok = $e[0] -eq 'total 0'
            if (-not $ok) {
                $Image.Save((Join-Path $script:Essai.Sortie ("04-" + ($Nom -replace '[^\w]+', '-') + '.png')), [System.Drawing.Imaging.ImageFormat]::Png)
            }
            Verifier "$Nom : chaque pixel a la couleur attendue" $ok ($e -join ' ; ') | Out-Null
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
                $r = $ps.Invoke()
                foreach ($x in $r) {
                    if ($null -ne $x) {
                        $b = if ($x -is [System.Management.Automation.PSObject]) { $x.BaseObject } else { $x }
                        if ($b -is [System.Drawing.Bitmap]) { return $b }
                    }
                }
                return $null
            }
            finally { $rs.Dispose() }
        }

        function Fichiers-Png([string]$Dossier) {
            if (-not (Test-Path -LiteralPath $Dossier)) { return @() }
            return @(Get-ChildItem -LiteralPath $Dossier -File -Filter '*.png' | Sort-Object LastWriteTimeUtc)
        }

        function Lire-Png([string]$Chemin) {
            $octets = [System.IO.File]::ReadAllBytes($Chemin)
            $flux = [System.IO.MemoryStream]::new($octets)
            $img = [System.Drawing.Bitmap]::new($flux)
            return [System.Drawing.Bitmap]::new($img)
        }

        # Lance une capture et attend la sélection prête. Rend { Id, Sel, Fige }.
        function Ouvrir-Selection([string]$Dossier, [string]$Quoi) {
            $gels = Compter-Motif -Motif 'etat Repos -> Gel' -Dossier $Dossier
            $pretes = Compter-Motif -Motif 'chrono selection prete \d+' -Dossier $Dossier
            [BreezyShotEssai.Zone]::Touches($CTRL_MAJ_1)
            $null = Attendre-Motif -Motif 'etat Repos -> Gel' -Dossier $Dossier -DelaiS 5 -Quoi "$Quoi : raccourci" -Fois ($gels + 1)
            $m = Attendre-Motif -Motif 'selection montree (\d+) ' -Dossier $Dossier -DelaiS 5 -Quoi "$Quoi : sélection montrée" -Fois ($gels + 1)
            $null = Attendre-Motif -Motif 'chrono selection prete \d+' -Dossier $Dossier -DelaiS 5 -Quoi "$Quoi : page de sélection prête" -Fois ($pretes + 1)
            $null = $m -match 'selection montree (\d+) '
            Start-Sleep -Milliseconds 150
            return [pscustomobject]@{ Id = [int]$Matches[1] }
        }

        # Attend le retour au Repos (n-ième fois) et vérifie que les deux fenêtres sont
        # cachées.
        function Attendre-Repos([string]$Dossier, [string]$Quoi, [int]$Fois, [IntPtr]$Sel, [IntPtr]$Fige) {
            $null = Attendre-Motif -Motif 'etat \w+ -> Repos' -Dossier $Dossier -DelaiS 5 -Quoi "$Quoi : retour au Repos" -Fois $Fois
            Start-Sleep -Milliseconds 150
            Verifier "$Quoi : écran figé et sélection cachés" ((-not (Visible $Sel)) -and (-not (Visible $Fige))) "sélection $(Visible $Sel), figé $(Visible $Fige)" | Out-Null
        }

        function Repos-Vus([string]$Dossier) { return (Compter-Motif -Motif 'etat \w+ -> Repos' -Dossier $Dossier) }

        # Glisser sur les carrés et attendre la fin de la sortie (n-ième annonce).
        function Capturer([string]$Dossier, [string]$Quoi, [int[]]$De, [int[]]$A) {
            $annonces = Compter-Motif -Motif 'sortie annonce ' -Dossier $Dossier
            $null = Ouvrir-Selection -Dossier $Dossier -Quoi $Quoi
            [BreezyShotEssai.Zone]::Glisser($De[0], $De[1], $A[0], $A[1])
            return (Attendre-Motif -Motif 'sortie annonce ' -Dossier $Dossier -DelaiS 10 -Quoi "$Quoi : annonce de la sortie" -Fois ($annonces + 1))
        }

        $HG = @($TX, $TY); $BD = @(($TX + $COTE - 1), ($TY + $COTE - 1))
        $BG = @($TX, ($TY + $COTE - 1)); $HD = @(($TX + $COTE - 1), $TY)

        try {
            $temoin = Lancer-Temoin
            Write-Host "témoin $temoin"

            # ===== Cas A : copie seule ==========================================================
            $da = Dossier-Cas 'a'
            Ecrire-Reglages -Dossier $da -Enregistrer $false -DossierEnregistrement $null
            $p = Demarrer-Cas -Dossier $da -Quoi '(A)'
            Verifier '(A) raccourci zone: Ctrl+Maj+1' ((Compter-Lignes -Texte 'raccourci zone: Ctrl+Maj+1' -Dossier $da) -eq 1) | Out-Null
            $sel = Hwnd-Journal 'selection fenetre' $da 'HWND de la sélection'
            $fige = Hwnd-Journal 'ecran fige fenetre' $da 'HWND de l''écran figé'
            $bandeau = Hwnd-Journal 'bandeau fenetre' $da 'HWND du bandeau'
            $parClasse = [BreezyShotEssai.Zone]::FindWindow('BreezyShotEcranFige', $null)
            Verifier '(2) écran figé : classe BreezyShotEcranFige, même HWND que le journal' ($parClasse -eq $fige) "classe $parClasse, journal $fige" | Out-Null
            Verifier '(2) au repos : écran figé et sélection cachés' ((-not (Visible $sel)) -and (-not (Visible $fige))) | Out-Null
            $recents = Join-Path $da 'local/Recents'
            $avantPremierPlan = [BreezyShotEssai.Zone]::GetForegroundWindow()
            $proc = Get-Process -Id $p.Id
            $fenetresAvant = [BreezyShotEssai.Zone]::FenetresDe([int[]]@($p.Id), $false).Length
            $filsAvant = $proc.Threads.Count

            # (1) Raccourci → chronos et lignes.
            $s = Ouvrir-Selection -Dossier $da -Quoi '(1)'
            Verifier '(1) chrono gel photo … total … visible …' ((Compter-Motif -Motif 'chrono gel photo \d+ total \d+ visible \d+$' -Dossier $da) -ge 1) | Out-Null
            Verifier '(1) selection montree …' $true | Out-Null
            $montree = Derniere-Ligne -Motif "selection montree $($s.Id) voulu (\S+) reel (\S+) echelle" -Dossier $da
            $voulu = $null; $reel = $null
            if ($montree -match 'voulu (\S+) reel (\S+) echelle') { $voulu = $Matches[1]; $reel = $Matches[2] }
            Verifier '(1) selection montree : reel = voulu' (($null -ne $voulu) -and ($voulu -eq $reel)) "voulu $voulu, reel $reel" | Out-Null
            Verifier '(1) voulu = bureau virtuel' ($voulu -eq "$($bureau.X),$($bureau.Y),$($bureau.L),$($bureau.H)") "voulu $voulu" | Out-Null
            $prete = Derniere-Ligne -Motif 'chrono selection prete (\d+)' -Dossier $da
            $msPrete = if ($prete -match 'chrono selection prete (\d+)') { [int]$Matches[1] } else { 99999 }
            Verifier '(1) chrono selection prete en moins de 2 s' ($msPrete -lt 2000) "$msPrete ms" | Out-Null
            Noter-Mesure -Nom 'chrono selection prete (première)' -Valeur $msPrete -Unite 'ms'
            $chronoGel = Derniere-Ligne -Motif 'chrono gel ' -Dossier $da
            if ($chronoGel -match 'visible (\d+)') { Noter-Mesure -Nom 'chrono gel visible (première)' -Valeur ([int]$Matches[1]) -Unite 'ms' }
            Verifier '(1) echap global reserve' ((Compter-Lignes -Texte 'echap global reserve' -Dossier $da) -ge 1) | Out-Null

            # (2) Les fenêtres pendant la sélection.
            $rb = "$($bureau.X),$($bureau.Y),$($bureau.L),$($bureau.H)"
            $rf = Rect-Fenetre $fige; $rs = Rect-Fenetre $sel
            Verifier '(2) écran figé visible et couvre le bureau virtuel' ((Visible $fige) -and ((Texte-Rect $rf) -eq $rb)) "figé $(Texte-Rect $rf), bureau $rb" | Out-Null
            Verifier '(2) sélection visible et couvre le bureau virtuel' ((Visible $sel) -and ((Texte-Rect $rs) -eq $rb)) "sélection $(Texte-Rect $rs), bureau $rb" | Out-Null
            $suivante = [BreezyShotEssai.Zone]::GetWindow($sel, $GW_HWNDNEXT)
            Verifier '(2) ordre : l''écran figé est juste sous la sélection' ($suivante -eq $fige) "sous la sélection $suivante, figé $fige" | Out-Null
            $devant = [BreezyShotEssai.Zone]::GetForegroundWindow()
            Verifier '(2) la sélection a le premier plan' ($devant -eq $sel) "premier plan $devant, sélection $sel" | Out-Null
            $exF = [long][BreezyShotEssai.Zone]::GetWindowLongPtr($fige, $GWL_EXSTYLE)
            $exS = [long][BreezyShotEssai.Zone]::GetWindowLongPtr($sel, $GWL_EXSTYLE)
            Verifier '(2) écran figé : WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST, sans WS_EX_APPWINDOW' ((($exF -band 0x80) -ne 0) -and (($exF -band 0x08000000) -ne 0) -and (($exF -band 0x8) -ne 0) -and (($exF -band 0x40000) -eq 0)) ('0x{0:X8}' -f $exF) | Out-Null
            Verifier '(2) sélection : WS_EX_TOOLWINDOW | WS_EX_TOPMOST, sans WS_EX_APPWINDOW ni WS_EX_NOACTIVATE' ((($exS -band 0x80) -ne 0) -and (($exS -band 0x8) -ne 0) -and (($exS -band 0x40000) -eq 0) -and (($exS -band 0x08000000) -eq 0)) ('0x{0:X8}' -f $exS) | Out-Null
            Verifier '(2) bandeau caché pendant la sélection' (-not (Visible $bandeau)) | Out-Null
            Start-Sleep -Seconds 3
            Verifier '(2) 3 s sans action : aucune fin focus_perdu, sélection toujours là' (((Compter-Lignes -Texte 'focus_perdu' -Dossier $da) -eq 0) -and (Visible $sel)) | Out-Null

            # (3) Glisser exact sur les quatre carrés.
            $seqAvant = [BreezyShotEssai.Zone]::GetClipboardSequenceNumber()
            $reposAvant = Repos-Vus $da
            [BreezyShotEssai.Zone]::Glisser($HG[0], $HG[1], $BD[0], $BD[1])
            $null = Attendre-Motif -Motif "selection fin $($s.Id) zone$" -Dossier $da -DelaiS 5 -Quoi '(3) fin de la sélection'
            Attendre-Repos -Dossier $da -Quoi '(3)' -Fois ($reposAvant + 1) -Sel $sel -Fige $fige
            Verifier '(3) Découpe -> Repos (ImagePreteSansVignette)' ((Compter-Lignes -Texte 'etat Decoupe -> Repos (ImagePreteSansVignette)' -Dossier $da) -eq 1) | Out-Null
            $zone = Derniere-Ligne -Motif 'zone choisie ' -Dossier $da
            Verifier '(3) zone choisie = le témoin' ($zone -match "zone choisie $TX,$TY ${COTE}x$COTE$") $zone | Out-Null
            Verifier '(3) le journal dit « Copié dans le presse-papiers »' ((Compter-Lignes -Texte 'sortie annonce reussite: Copié dans le presse-papiers' -Dossier $da) -eq 1) | Out-Null
            Verifier '(3) sortie copie faite recent ok fichier non demande' ((Compter-Lignes -Texte 'sortie copie faite recent ok fichier non demande' -Dossier $da) -eq 1) | Out-Null
            Verifier '(3) echap global libere' ((Compter-Lignes -Texte 'echap global libere' -Dossier $da) -ge 1) | Out-Null
            Verifier '(3) le presse-papiers a changé' ([BreezyShotEssai.Zone]::GetClipboardSequenceNumber() -ne $seqAvant) | Out-Null
            Verifier-Image '(3) presse-papiers' (Lire-PressePapiers)
            $pngs = Fichiers-Png $recents
            Verifier '(3) « Récents » contient un PNG' ($pngs.Count -eq 1) "$($pngs.Count) fichier(s)" | Out-Null
            if ($pngs.Count -ge 1) { Verifier-Image '(3) PNG de « Récents »' (Lire-Png $pngs[-1].FullName) }
            $apresPremierPlan = [BreezyShotEssai.Zone]::GetForegroundWindow()
            Verifier '(3) la fenêtre qui avait le premier plan l''a retrouvé' ($apresPremierPlan -eq $avantPremierPlan) "avant $avantPremierPlan, après $apresPremierPlan" | Out-Null
            # Bandeau de réussite : montré, puis parti seul, sans cache d'office.
            $aff = Attendre-Motif -Motif 'bandeau affiche (\d+) reussite ' -Dossier $da -DelaiS 5 -Quoi '(3) bandeau de réussite'
            $null = $aff -match 'bandeau affiche (\d+) reussite '
            $idReussite = $Matches[1]
            $null = Attendre-Motif -Motif "bandeau termine $idReussite$" -Dossier $da -DelaiS 6 -Quoi '(3) fin de la réussite'
            Start-Sleep -Milliseconds 200
            Verifier '(3) bandeau termine puis caché, sans cache d''office' ((-not (Visible $bandeau)) -and ((Compter-Lignes -Texte "bandeau cache d'office" -Dossier $da) -eq 0)) | Out-Null

            # (4) Échap, puis clic droit : annulation, rien de plus.
            foreach ($geste in 'Échap', 'clic droit') {
                $seq = [BreezyShotEssai.Zone]::GetClipboardSequenceNumber()
                $n = (Fichiers-Png $recents).Count
                $reposAvant = Repos-Vus $da
                $null = Ouvrir-Selection -Dossier $da -Quoi "(4) $geste"
                if ($geste -eq 'Échap') { [BreezyShotEssai.Zone]::Touches($ECHAP) }
                else { [BreezyShotEssai.Zone]::Cliquer($TX + 50, $TY + 50, $true) }
                Attendre-Repos -Dossier $da -Quoi "(4) $geste" -Fois ($reposAvant + 1) -Sel $sel -Fige $fige
                Verifier "(4) $geste : Selection -> Repos (SelectionAnnulee)" ((Derniere-Ligne -Motif 'etat \w+ -> Repos' -Dossier $da) -match 'etat Selection -> Repos \(SelectionAnnulee\)') | Out-Null
                Verifier "(4) $geste : aucun fichier de plus, presse-papiers inchangé" (((Fichiers-Png $recents).Count -eq $n) -and ([BreezyShotEssai.Zone]::GetClipboardSequenceNumber() -eq $seq)) | Out-Null
            }
            Verifier '(4) Échap passe par l''Échap global' ((Compter-Lignes -Texte 'echap global presse' -Dossier $da) -ge 1) | Out-Null

            # (5) Clic sans glisser, puis glisser de 2 px : la sélection reste ; puis Échap.
            $reposAvant = Repos-Vus $da
            $null = Ouvrir-Selection -Dossier $da -Quoi '(5)'
            [BreezyShotEssai.Zone]::Cliquer($TX + 30, $TY + 30, $false)
            Start-Sleep -Milliseconds 600
            Verifier '(5) clic sans glisser : la sélection reste ouverte' ((Visible $sel) -and ((Repos-Vus $da) -eq $reposAvant)) | Out-Null
            [BreezyShotEssai.Zone]::Glisser($TX + 30, $TY + 30, $TX + 32, $TY + 32)
            Start-Sleep -Milliseconds 600
            Verifier '(5) glisser de 2 px : la sélection reste ouverte' ((Visible $sel) -and ((Repos-Vus $da) -eq $reposAvant)) | Out-Null
            [BreezyShotEssai.Zone]::Touches($ECHAP)
            Attendre-Repos -Dossier $da -Quoi '(5) puis Échap' -Fois ($reposAvant + 1) -Sel $sel -Fige $fige

            # (6) Second raccourci pendant la sélection : ignoré (une seule ligne Gel).
            $reposAvant = Repos-Vus $da
            $null = Ouvrir-Selection -Dossier $da -Quoi '(6)'
            $gels = Compter-Motif -Motif 'etat Repos -> Gel' -Dossier $da
            [BreezyShotEssai.Zone]::Touches($CTRL_MAJ_1)
            $null = Attendre-Motif -Motif 'capture deja en cours: appui ignore$' -Dossier $da -DelaiS 5 -Quoi '(6) second appui'
            Start-Sleep -Milliseconds 300
            Verifier '(6) second raccourci ignoré : aucune nouvelle ligne Gel, sélection toujours là' (((Compter-Motif -Motif 'etat Repos -> Gel' -Dossier $da) -eq $gels) -and (Visible $sel)) | Out-Null
            [BreezyShotEssai.Zone]::Touches($ECHAP)
            Attendre-Repos -Dossier $da -Quoi '(6) puis Échap' -Fois ($reposAvant + 1) -Sel $sel -Fige $fige

            # (7) Glisser de droite à gauche et de bas en haut (et en diagonale inverse).
            foreach ($sens in @(@('bas droite → haut gauche', $BD, $HG), @('bas gauche → haut droite', $BG, $HD))) {
                $null = Capturer -Dossier $da -Quoi "(7) $($sens[0])" -De $sens[1] -A $sens[2]
                Verifier-Image "(7) $($sens[0]) : presse-papiers" (Lire-PressePapiers)
            }

            # (9) Deux captures rapprochées : la seconde part pendant le bandeau de la
            # première ; le bandeau n'est ni au-dessus de la sélection ni dans l'image.
            $affiches = Compter-Motif -Motif 'bandeau affiche \d+ reussite ' -Dossier $da
            $null = Capturer -Dossier $da -Quoi '(9) première' -De $HG -A $BD
            $null = Attendre-Motif -Motif 'bandeau affiche \d+ reussite ' -Dossier $da -DelaiS 5 -Quoi '(9) bandeau de la première' -Fois ($affiches + 1)
            Start-Sleep -Milliseconds 400   # pilule entrée (300 ms), encore là (2,5 s)
            Verifier '(9) le bandeau de la première est à l''écran' (Visible $bandeau) | Out-Null
            $annonces = Compter-Motif -Motif 'sortie annonce ' -Dossier $da
            $null = Ouvrir-Selection -Dossier $da -Quoi '(9) seconde'
            Verifier '(9) pendant la seconde sélection, le bandeau est caché' (-not (Visible $bandeau)) | Out-Null
            [BreezyShotEssai.Zone]::Glisser($HG[0], $HG[1], $BD[0], $BD[1])
            $null = Attendre-Motif -Motif 'sortie annonce ' -Dossier $da -DelaiS 10 -Quoi '(9) annonce de la seconde' -Fois ($annonces + 1)
            Verifier-Image '(9) seconde image, sans le bandeau' (Lire-PressePapiers)

            # F5 puis Échap ; Ctrl+P sans fenêtre d'impression.
            $reposAvant = Repos-Vus $da
            $null = Ouvrir-Selection -Dossier $da -Quoi '(F5)'
            [BreezyShotEssai.Zone]::Touches([byte[]]@(0x74))
            Start-Sleep -Milliseconds 800
            [BreezyShotEssai.Zone]::Touches($ECHAP)
            Attendre-Repos -Dossier $da -Quoi '(F5) puis Échap' -Fois ($reposAvant + 1) -Sel $sel -Fige $fige
            $pids = @($p.Id) + @(Get-Process -Name 'msedgewebview2' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
            $visiblesAvant = @([BreezyShotEssai.Zone]::FenetresDe([int[]]$pids, $true) | ForEach-Object { [long]$_ })
            $reposAvant = Repos-Vus $da
            $null = Ouvrir-Selection -Dossier $da -Quoi '(Ctrl+P)'
            [BreezyShotEssai.Zone]::Touches([byte[]]@(0x11, 0x50))
            Start-Sleep -Milliseconds 1500
            $pids = @($p.Id) + @(Get-Process -Name 'msedgewebview2' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
            $nouvelles = @([BreezyShotEssai.Zone]::FenetresDe([int[]]$pids, $true) | ForEach-Object { [long]$_ } |
                Where-Object { ($visiblesAvant -notcontains $_) -and ($_ -ne [long]$sel) -and ($_ -ne [long]$fige) })
            Verifier '(Ctrl+P) aucune fenêtre d''impression (aucune nouvelle fenêtre visible)' ($nouvelles.Count -eq 0) "nouvelles : $($nouvelles -join ', ')" | Out-Null
            [BreezyShotEssai.Zone]::Touches($ECHAP)
            Attendre-Repos -Dossier $da -Quoi '(Ctrl+P) puis Échap' -Fois ($reposAvant + 1) -Sel $sel -Fige $fige

            # Alt+F4 : annulation propre, puis une capture marche.
            $reposAvant = Repos-Vus $da
            $null = Ouvrir-Selection -Dossier $da -Quoi '(Alt+F4)'
            [BreezyShotEssai.Zone]::Touches([byte[]]@(0x12, 0x73))
            Attendre-Repos -Dossier $da -Quoi '(Alt+F4)' -Fois ($reposAvant + 1) -Sel $sel -Fige $fige
            Verifier '(Alt+F4) fin « annulee », aucune erreur affichée' (((Derniere-Ligne -Motif 'selection fin \d+ ' -Dossier $da) -match 'annulee') -and ((Compter-Lignes -Texte 'capture impossible' -Dossier $da) -eq 0)) (Derniere-Ligne -Motif 'selection fin \d+ ' -Dossier $da) | Out-Null
            $null = Capturer -Dossier $da -Quoi '(Alt+F4) capture suivante' -De $HG -A $BD
            Verifier-Image '(Alt+F4) capture suivante : presse-papiers' (Lire-PressePapiers)

            # Perte de focus : le témoin prend le premier plan.
            $reposAvant = Repos-Vus $da
            $null = Ouvrir-Selection -Dossier $da -Quoi '(focus)'
            [BreezyShotEssai.Zone]::Touches([byte[]]@(0x12))   # Alt : lève le verrou du premier plan
            $donne = [BreezyShotEssai.Zone]::SetForegroundWindow($temoin)
            $null = Attendre-Motif -Motif 'selection fin \d+ annulee focus_perdu$' -Dossier $da -DelaiS 5 -Quoi "(focus) perte de focus (SetForegroundWindow : $donne)"
            Attendre-Repos -Dossier $da -Quoi '(focus) perdu' -Fois ($reposAvant + 1) -Sel $sel -Fige $fige

            # Trois captures de suite : pas de fuite de fenêtres ni de fils.
            for ($i = 1; $i -le 3; $i++) { $null = Capturer -Dossier $da -Quoi "(fuite) capture $i" -De $HG -A $BD }
            Start-Sleep -Seconds 3
            $proc.Refresh()
            $fenetresApres = [BreezyShotEssai.Zone]::FenetresDe([int[]]@($p.Id), $false).Length
            $filsApres = $proc.Threads.Count
            Noter-Mesure -Nom 'fenêtres du processus avant / après' -Valeur "$fenetresAvant / $fenetresApres"
            Noter-Mesure -Nom 'fils du processus avant / après' -Valeur "$filsAvant / $filsApres"
            Verifier '(fuite) fenêtres du processus stables (±2)' ([math]::Abs($fenetresApres - $fenetresAvant) -le 2) "$fenetresAvant → $fenetresApres" | Out-Null
            Verifier '(fuite) fils du processus stables (±6)' ([math]::Abs($filsApres - $filsAvant) -le 6) "$filsAvant → $filsApres" | Out-Null
            Verifier '(A) aucune garde déclenchée, aucune erreur de capture' (((Compter-Lignes -Texte 'arret force' -Dossier $da) -eq 0) -and ((Compter-Lignes -Texte 'capture impossible' -Dossier $da) -eq 0)) | Out-Null
            Fermer-Cas $p $da '(A)'

            # ===== Cas B : (8) enregistrement après capture =====================================
            $dbb = Dossier-Cas 'b'
            Ecrire-Reglages -Dossier $dbb -Enregistrer $true -DossierEnregistrement $null
            $p = Demarrer-Cas -Dossier $dbb -Quoi '(8)'
            $null = Capturer -Dossier $dbb -Quoi '(8)' -De $HG -A $BD
            Verifier '(8) le journal dit « Copié et enregistré »' ((Compter-Lignes -Texte 'sortie annonce reussite: Copié et enregistré' -Dossier $dbb) -eq 1) | Out-Null
            Verifier '(8) sortie copie faite recent ok fichier fait' ((Compter-Lignes -Texte 'sortie copie faite recent ok fichier fait' -Dossier $dbb) -eq 1) | Out-Null
            $images = @(Fichiers-Png (Join-Path $dbb 'images'))
            $nom = if ($images.Count -ge 1) { $images[-1].Name } else { '' }
            Verifier '(8) un fichier dans le dossier, nommé d''après le modèle' (($images.Count -eq 1) -and ($nom -match '^BreezyShot \d{4}-\d{2}-\d{2} à \d{2}\.\d{2}\.\d{2}\.png$')) "$($images.Count) fichier(s) : $nom" | Out-Null
            if ($images.Count -ge 1) { Verifier-Image '(8) fichier enregistré' (Lire-Png $images[-1].FullName) }
            Fermer-Cas $p $dbb '(8)'

            # ===== Cas C : (10) dossier impossible (un fichier porte son nom) ===================
            $dc = Dossier-Cas 'c'
            $bloque = Join-Path $dc 'bloque'
            Set-Content -LiteralPath $bloque -Value 'pas un dossier' -Encoding utf8
            Ecrire-Reglages -Dossier $dc -Enregistrer $true -DossierEnregistrement $bloque
            $p = Demarrer-Cas -Dossier $dc -Quoi '(10)'
            $seq = [BreezyShotEssai.Zone]::GetClipboardSequenceNumber()
            $null = Capturer -Dossier $dc -Quoi '(10)' -De $HG -A $BD
            Verifier '(10) le journal dit « Impossible d''enregistrer »' ((Compter-Lignes -Texte "sortie annonce erreur: Impossible d'enregistrer" -Dossier $dc) -eq 1) | Out-Null
            Verifier '(10) bandeau d''erreur' ($null -ne (Attendre-Motif -Motif 'bandeau affiche \d+ erreur ' -Dossier $dc -DelaiS 5 -Quoi '(10) bandeau')) | Out-Null
            Verifier '(10) la copie est faite quand même' (((Compter-Lignes -Texte 'sortie copie faite recent ok fichier echec' -Dossier $dc) -eq 1) -and ([BreezyShotEssai.Zone]::GetClipboardSequenceNumber() -ne $seq)) | Out-Null
            Verifier-Image '(10) presse-papiers' (Lire-PressePapiers)
            Fermer-Cas $p $dc '(10)'
        }
        finally {
            Fermer-Auxiliaires
            Arreter-Tout
        }
    }
}
catch {
    Verifier "déroulement de l'essai" $false "$_" | Out-Null
}
finally {
    # Les chronos de chaque cas, recopiés dans la sortie de l'essai.
    foreach ($d in $script:Dossiers) {
        $chronos = @(Lignes-Journal $d | Where-Object { $_ -match ' chrono ' })
        if ($chronos.Count -gt 0) {
            Write-Host "--- chronos du cas $(Split-Path -Leaf $d) ---"
            $chronos | ForEach-Object { Write-Host $_ }
        }
    }
    # En cas d'échec, les 25 dernières lignes du journal de chaque cas.
    if (@($script:Verifications | Where-Object { -not $_.Reussi }).Count -gt 0) {
        foreach ($d in $script:Dossiers) {
            Write-Host "--- fin du journal du cas $(Split-Path -Leaf $d) ---"
            Lignes-Journal $d | Select-Object -Last 25 | ForEach-Object { Write-Host $_ }
        }
    }
    foreach ($a in $script:Auxiliaires) { try { if (-not $a.HasExited) { $a.Kill() } } catch { } }
    foreach ($d in $script:Dossiers) {
        $source = Join-Path $d 'local/Journal'
        if (Test-Path -LiteralPath $source) {
            $cible = Join-Path $script:Essai.Sortie ("journaux/04-zone-" + (Split-Path -Leaf $d))
            New-Item -ItemType Directory -Force -Path $cible | Out-Null
            Copy-Item -Path (Join-Path $source '*') -Destination $cible -Force
        }
    }
    Terminer-Essai
}
