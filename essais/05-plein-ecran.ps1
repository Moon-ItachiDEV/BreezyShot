# Étape 7 : la capture plein écran, de bout en bout (ARCHITECTURE.md §3.7), sur le vrai
# .exe, avec de vraies entrées clavier.
#
# (1) Démarrage : « raccourci plein_ecran: Ctrl+Maj+2 », aucune annonce. (2) Souris sur
# l'écran principal, Ctrl+Maj+2 : Repos -> Gel, Gel -> Decoupe (PhotoPrise plein_ecran),
# « plein ecran … souris », Decoupe -> Repos, « Copié et enregistré » (réglage
# d'enregistrement vrai, vignette coupée : le parcours de 0.2 ; réglages inchangés).
# (3) Pour cette capture, aucune ligne « ecran fige », « selection » ni « echap global », et
# ni l'écran figé ni la sélection ne deviennent visibles (guettés pendant et après).
# (4) Le PNG enregistré et l'image du presse-papiers ont exactement la taille de l'écran
# principal en pixels réels, et la fenêtre témoin y est à sa place, aux bonnes couleurs.
# (5) Deux captures de suite, la seconde pendant le bandeau de la première (vérifié
# visible juste avant l'appui, sans « bandeau termine ») : sous la fenêtre du bandeau, la
# seconde image a les couleurs du témoin. (6) Presse-papiers tenu par l'essai, deux appuis
# à 30 ms : le second, pendant la capture, est ignoré (la copie échoue : « Impossible de
# copier ») ; après, une capture normale, presse-papiers compris. (7) --quitter → arret
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
# d'Impr. écran, pas d'annonce au démarrage), copie, enregistrement vrai, vignette coupée
# (elle a son essai 07).
function Ecrire-Reglages([string]$Dossier) {
    $r = [ordered]@{
        version                   = 1
        raccourcis                = [ordered]@{ zone = @('Ctrl+Maj+1'); plein_ecran = @('Ctrl+Maj+2'); fenetre = @() }
        copier_apres_capture      = $true
        enregistrer_apres_capture = $true
        dossier_enregistrement    = $null
        vignette                  = [ordered]@{ afficher = $false; duree_s = 6; coin = 'bas_gauche' }
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
[StructLayout(LayoutKind.Sequential)]
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
[DllImport("user32.dll")]
public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll", SetLastError = true)]
static extern bool OpenClipboard(IntPtr h);
[DllImport("user32.dll")]
static extern bool CloseClipboard();
[DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
static extern IntPtr CreateWindowExW(uint exStyle, string classe, string titre, uint style, int x, int y, int l, int h, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr param);
[DllImport("user32.dll", SetLastError = true)]
static extern bool DestroyWindow(IntPtr h);
// Appuie (comme Appuyer) en guettant les fenêtres données, toutes les 2 ms, depuis avant
// l'appui jusqu'à $dureeMs après : rend le nombre de relevés où l'une était visible.
// Avec $tenueMs > 0, le presse-papiers est ouvert (donc occupé pour l'app) avant l'appui
// et refermé $tenueMs après : la copie de l'app réessaie jusqu'à son échéance (250 ms),
// la Découpe dure donc au moins ce temps. Il est ouvert au nom d'une vraie fenêtre de
// l'essai : un OpenClipboard(NULL) venu d'ailleurs réussit quand le premier a aussi été
// ouvert avec NULL, et l'app ouvre avec NULL. Rend -1 si la fenêtre n'a pas pu être créée
// ou le presse-papiers ouvert en 10 essais (rien n'est alors appuyé).
public static int AppuyerEnGuettant(byte[] mods, byte touche, int fois, int ecart, IntPtr[] guettees, int dureeMs, int tenueMs) {
    IntPtr tenante = IntPtr.Zero;
    if (tenueMs > 0) {
        // Fenêtre de messages (HWND_MESSAGE = -3) sur ce fil, jamais montrée.
        tenante = CreateWindowExW(0, "STATIC", "", 0, 0, 0, 0, 0, new IntPtr(-3), IntPtr.Zero, IntPtr.Zero, IntPtr.Zero);
        if (tenante == IntPtr.Zero) return -1;
        // Quelques essais, 20 ms d'écart : une autre application peut le tenir un instant.
        bool ouvert = false;
        for (int i = 0; i < 10 && !ouvert; i++) {
            if (i > 0) System.Threading.Thread.Sleep(20);
            ouvert = OpenClipboard(tenante);
        }
        if (!ouvert) { DestroyWindow(tenante); return -1; }
    }
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
    try {
        System.Threading.Thread.Sleep(20);
        Appuyer(mods, touche, fois, ecart);
        if (tenueMs > 0) System.Threading.Thread.Sleep(tenueMs);
    }
    finally { if (tenueMs > 0) { CloseClipboard(); DestroyWindow(tenante); } }
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
// Pixels du rectangle (x, y, l, h) d'une image (octets BGRA de LockBits,
// Format32bppArgb), déjà borné à l'image et au témoin, qui n'ont pas la couleur du carré
// du témoin à cet endroit. (tx, ty) : coin haut gauche du témoin dans l'image ; c : r,v,b
// des carrés haut gauche, haut droite, bas gauche, bas droite.
public static int EcartsTemoin(byte[] o, int stride, int x, int y, int l, int h, int tx, int ty, int cote, int[] c) {
    int n = 0, m = cote / 2;
    for (int j = y; j < y + h; j++) {
        for (int i = x; i < x + l; i++) {
            int q = ((j - ty) < m ? 0 : 2) + ((i - tx) < m ? 0 : 1);
            int k = j * stride + 4 * i;
            if (o[k + 2] != c[3 * q] || o[k + 1] != c[3 * q + 1] || o[k] != c[3 * q + 2]) n++;
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
        $COULEURS_PLAT = [int[]]@(220, 40, 40, 40, 180, 60, 40, 80, 220, 240, 200, 30)

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
        # Avec -TenirPressePapiersMs, le presse-papiers est tenu ouvert pendant l'appui.
        # Rend { Debut (lignes du journal avant), Lignes (journal de cette capture), Vues
        # (relevés où l'une était visible), Ecran, LigneEcran, Seq (presse-papiers avant) }.
        function Capturer-PleinEcran([string]$Dossier, [string]$Quoi, [IntPtr[]]$Guettees, [int]$Fois = 1, [int]$TenirPressePapiersMs = 0) {
            $debut = @(Lignes-Journal $Dossier).Count
            $annonces = Compter-Motif -Motif 'sortie annonce ' -Dossier $Dossier
            $seq = [BreezyShotEssai.PleinEcran]::GetClipboardSequenceNumber()
            $vues = [BreezyShotEssai.PleinEcran]::AppuyerEnGuettant($MODS, $TOUCHE_2, $Fois, 30, $Guettees, 300, $TenirPressePapiersMs)
            if ($vues -lt 0) {
                # Échec explicite, jamais un point sauté.
                Verifier "$Quoi : presse-papiers tenu par l'essai" $false 'presse-papiers non tenu (fenêtre de l''essai ou ouverture impossible), rien n''a été appuyé' | Out-Null
                throw "$Quoi : presse-papiers non tenu"
            }
            $null = Attendre-Motif -Motif 'sortie annonce ' -Dossier $Dossier -DelaiS 10 -Quoi "$Quoi : annonce de la sortie" -Fois ($annonces + 1)
            $null = Attendre-Motif -Motif 'chrono decoupe \d+ sortie \d+ relache-annonce \d+$' -Dossier $Dossier -DelaiS 3 -Quoi "$Quoi : fin de la sortie" -Fois ($annonces + 1)
            # Après : toujours rien de visible.
            Start-Sleep -Milliseconds 300
            foreach ($h in $Guettees) { if (Visible $h) { $vues++ } }
            $lignes = @(Lignes-Depuis $Dossier $debut)
            $ligneEcran = $null
            foreach ($l in $lignes) { if ($l -match 'plein ecran ') { $ligneEcran = $l } }
            return [pscustomobject]@{ Debut = $debut; Lignes = $lignes; Vues = $vues; Ecran = (Lire-PleinEcran $ligneEcran); LigneEcran = $ligneEcran; Seq = $seq }
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
                $affiches = Compter-Motif -Motif 'bandeau affiche \d+ reussite ' -Dossier $da
                $c1 = Capturer-PleinEcran -Dossier $da -Quoi '(2) première capture' -Guettees $guettees

                # (5) Tout de suite : la seconde capture pendant le bandeau de la première
                # (une réussite part seule après 2,5 s). Les vérifications de la première
                # viennent après.
                $aff = Attendre-Motif -Motif 'bandeau affiche (\d+) reussite ' -Dossier $da -DelaiS 5 -Quoi '(5) bandeau de la première' -Fois ($affiches + 1)
                $null = $aff -match 'bandeau affiche (\d+) reussite '
                $idBandeau = $Matches[1]
                # Une réussite n'envoie jamais sa zone (`bandeau.js`, `envoyerZone` : pas de
                # zone cliquable), il n'y a donc pas de « bandeau zone » : on prend le
                # rectangle de la fenêtre du bandeau, en pixels réels, dans la ligne
                # « bandeau affiche <id> reussite ecran <x>,<y> <l>x<h> echelle <e> »
                # (`fenetres::placer` : 560 × 220 pixels logiques fois l'échelle, centrée en
                # haut de la zone utile). La pilule y est centrée à 64 px logiques du haut,
                # environ 170 × 34 pixels logiques pour « Copié et enregistré »
                # (`bandeau.css`, jetons) : à 1024 px de large et 100 %, fenêtre
                # 232..792 × 0..220, témoin 412..612 × 40..240, pilule vers 427..597 × 64..98,
                # donc dans l'intersection. Le témoin de la preuve ci-dessous le vérifie.
                if ($aff -notmatch "bandeau affiche $idBandeau reussite ecran (-?\d+),(-?\d+) (\d+)x(\d+) echelle") { throw "(5) ligne d'affichage illisible : $aff" }
                $zb = [pscustomobject]@{ X = [int]$Matches[1]; Y = [int]$Matches[2]; L = [int]$Matches[3]; H = [int]$Matches[4] }
                $rf = [BreezyShotEssai.PleinEcran+RECT]::new()
                [void][BreezyShotEssai.PleinEcran]::GetWindowRect($bandeau, [ref]$rf)
                Write-Host "(5) fenêtre du bandeau : journal $($zb.X),$($zb.Y) $($zb.L)x$($zb.H), GetWindowRect $($rf.Left),$($rf.Top) $($rf.Right - $rf.Left)x$($rf.Bottom - $rf.Top)"
                # Seule la part de la fenêtre du bandeau qui couvre le témoin est comparée :
                # ses couleurs sont connues, rien n'y bouge.
                $ix = [math]::Max($zb.X, $TX); $iy = [math]::Max($zb.Y, $TY)
                $il = [math]::Min($zb.X + $zb.L, $TX + $COTE) - $ix
                $ih = [math]::Min($zb.Y + $zb.H, $TY + $COTE) - $iy
                Write-Host "(5) fenêtre du bandeau sur le témoin : $ix,$iy ${il}x$ih"
                $surTemoin = ($il -gt 0) -and ($ih -gt 0)
                Verifier '(5) la fenêtre du bandeau couvre une part du témoin' $surTemoin "fenêtre $($zb.X),$($zb.Y) $($zb.L)x$($zb.H), témoin $TX,$TY ${COTE}x$COTE" | Out-Null
                Start-Sleep -Milliseconds 400   # pilule entrée (300 ms), encore là (2,5 s)
                if ($surTemoin) {
                    # Témoin de la preuve : le bandeau est réellement peint à l'écran à cet
                    # endroit (une copie d'écran n'y a plus les couleurs du témoin).
                    $copieEcran = [System.Drawing.Bitmap]::new($il, $ih, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
                    try {
                        $g = [System.Drawing.Graphics]::FromImage($copieEcran)
                        try { $g.CopyFromScreen($ix, $iy, 0, 0, $copieEcran.Size) }
                        finally { $g.Dispose() }
                        $oc = Octets-Image $copieEcran
                        $peints = [BreezyShotEssai.PleinEcran]::EcartsTemoin($oc.Octets, $oc.Stride, 0, 0, $il, $ih, ($TX - $ix), ($TY - $iy), $COTE, $COULEURS_PLAT)
                    }
                    finally { $copieEcran.Dispose() }
                    Verifier '(5) témoin de la preuve : le bandeau est peint à l''écran, sur le témoin' ($peints -gt 0) "$peints pixel(s) sans la couleur du témoin" | Out-Null
                }
                # Juste avant l'appui : le bandeau est là et n'a pas fini.
                $termineAvant = Compter-Motif -Motif "bandeau termine $idBandeau$" -Dossier $da
                $bandeauLa = Visible $bandeau
                Verifier '(5) le bandeau de la première est à l''écran juste avant l''appui' ($bandeauLa -and ($termineAvant -eq 0)) "visible $bandeauLa, bandeau termine vu $termineAvant fois" | Out-Null
                $c2 = Capturer-PleinEcran -Dossier $da -Quoi '(5) seconde capture' -Guettees $guettees
                # Aucun « bandeau termine » de ce message avant le Repos -> Gel de la seconde.
                $toutes = @(Lignes-Journal $da)
                $iGel = -1
                for ($k = $c2.Debut; $k -lt $toutes.Count; $k++) { if ($toutes[$k] -match 'etat Repos -> Gel ') { $iGel = $k; break } }
                $iTermine = -1
                for ($k = 0; $k -lt $toutes.Count; $k++) { if ($toutes[$k] -match "bandeau termine $idBandeau$") { $iTermine = $k; break } }
                Verifier '(5) aucun « bandeau termine » de la première avant le Repos -> Gel de la seconde' (($iGel -ge 0) -and (($iTermine -lt 0) -or ($iTermine -gt $iGel))) "ligne du Gel $iGel, ligne de la fin du bandeau $iTermine" | Out-Null

                # (2) La première capture, d'après son journal.
                $t1 = $c1.Lignes -join "`n"
                Verifier '(2) etat Repos -> Gel (DemandeCapture plein_ecran)' ($t1.Contains('etat Repos -> Gel (DemandeCapture plein_ecran)')) | Out-Null
                Verifier '(2) etat Gel -> Decoupe (PhotoPrise plein_ecran)' ($t1.Contains('etat Gel -> Decoupe (PhotoPrise plein_ecran)')) | Out-Null
                Verifier '(2) plein ecran … souris : l''écran principal' (($null -ne $c1.Ecran) -and ($c1.Ecran.Origine -eq 'souris') -and ($c1.Ecran.X -eq $principal.X) -and ($c1.Ecran.Y -eq $principal.Y) -and ("$($c1.Ecran.L)x$($c1.Ecran.H)" -eq $TAILLE)) "$($c1.LigneEcran)" | Out-Null
                Verifier '(2) chrono gel photo … total … (sans visible)' (@($c1.Lignes | Where-Object { $_ -match 'chrono gel photo \d+ total \d+$' }).Count -eq 1) | Out-Null
                Verifier '(2) etat Decoupe -> Repos (ImagePreteSansVignette)' ($t1.Contains('etat Decoupe -> Repos (ImagePreteSansVignette)')) | Out-Null
                Verifier '(2) sortie annonce reussite: Copié et enregistré' ($t1.Contains('sortie annonce reussite: Copié et enregistré')) | Out-Null
                Verifier '(2) sortie copie faite recent ok fichier fait' ($t1.Contains('sortie copie faite recent ok fichier fait')) | Out-Null
                Verifier '(2) aucune « capture impossible »' (-not $t1.Contains('capture impossible')) | Out-Null
                $t2 = $c2.Lignes -join "`n"
                Verifier '(5) seconde capture : Copié et enregistré' ($t2.Contains('sortie annonce reussite: Copié et enregistré')) | Out-Null

                # (3) Rien de montré, pour les deux captures.
                foreach ($c in @(@('première', $c1), @('seconde', $c2))) {
                    $montrees = @($c[1].Lignes | Where-Object { $_ -match ' (ecran fige|selection|echap global)' })
                    Verifier "(3) $($c[0]) : aucune ligne « ecran fige », « selection » ni « echap global »" ($montrees.Count -eq 0) ($montrees -join ' ; ') | Out-Null
                    Verifier "(3) $($c[0]) : écran figé et sélection jamais visibles (guettés pendant et après)" ($c[1].Vues -eq 0) "$($c[1].Vues) relevé(s) visible(s)" | Out-Null
                }

                # (4) Les fichiers et le presse-papiers (celui de la seconde capture : la
                # première l'avait déjà changé, la seconde l'a remplacé).
                $pngs = @(Fichiers-Png $images)
                Verifier '(4) deux PNG dans le dossier Images › BreezyShot de l''essai' ($pngs.Count -eq 2) "$($pngs.Count) fichier(s)" | Out-Null
                if ($pngs.Count -ge 1) {
                    $image1 = Lire-Png $pngs[0].FullName
                    try { Verifier-Image '(4) premier PNG' $image1 $c1.Ecran }
                    finally { $image1.Dispose() }
                }
                $seqFin = [BreezyShotEssai.PleinEcran]::GetClipboardSequenceNumber()
                Verifier '(4) le presse-papiers a changé à chaque capture' (($c2.Seq -ne $c1.Seq) -and ($seqFin -ne $c2.Seq)) "séquence $($c1.Seq) → $($c2.Seq) → $seqFin" | Out-Null
                $copie = Lire-PressePapiers
                try { Verifier-Image '(4) presse-papiers' $copie $c2.Ecran }
                finally { if ($null -ne $copie) { $copie.Dispose() } }
                $reglagesApres = [System.IO.File]::ReadAllBytes($cheminReglages)
                Verifier '(2) réglages non réécrits (enregistrement automatique sans toucher au fichier)' ([Convert]::ToBase64String($reglagesAvant) -eq [Convert]::ToBase64String($reglagesApres)) | Out-Null

                # (5) La seconde image : sur la part du témoin sous le bandeau, les couleurs du
                # témoin (le bandeau n'y est pas).
                if (($pngs.Count -ge 2) -and $surTemoin -and ($null -ne $c2.Ecran)) {
                    $image2 = Lire-Png $pngs[-1].FullName
                    try {
                        Verifier-Image '(5) second PNG' $image2 $c2.Ecran
                        $taille2 = "$($image2.Width)x$($image2.Height)"
                        if ($taille2 -eq $TAILLE) {
                            $ob = Octets-Image $image2
                            $ex = $c2.Ecran.X; $ey = $c2.Ecran.Y
                            $diff = [BreezyShotEssai.PleinEcran]::EcartsTemoin($ob.Octets, $ob.Stride, ($ix - $ex), ($iy - $ey), $il, $ih, ($TX - $ex), ($TY - $ey), $COTE, $COULEURS_PLAT)
                            if ($diff -ne 0) { $image2.Save((Join-Path $script:Essai.Sortie '05-seconde-image.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
                            Verifier '(5) sous la fenêtre du bandeau, la seconde image a les couleurs du témoin (bandeau absent)' ($diff -eq 0) "$diff pixel(s) différents sur ${il}x$ih" | Out-Null
                        }
                        else {
                            Verifier '(5) comparaison sous la fenêtre du bandeau' $false "image $taille2, écran $TAILLE" | Out-Null
                        }
                    }
                    finally { $image2.Dispose() }
                }
                else {
                    Verifier '(5) comparaison sous la fenêtre du bandeau possible' $false "PNG $($pngs.Count), zone sur le témoin $surTemoin, écran lu $($null -ne $c2.Ecran)" | Out-Null
                }

                # (6) Deux appuis à 30 ms, presse-papiers tenu 500 ms par l'essai : la copie de
                # l'app réessaie jusqu'à son échéance (250 ms), la Découpe dure au moins
                # autant, le second appui tombe forcément pendant la capture.
                $nPng = @(Fichiers-Png $images).Count
                $c3 = Capturer-PleinEcran -Dossier $da -Quoi '(6) double appui' -Guettees $guettees -Fois 2 -TenirPressePapiersMs 500
                $l3 = @($c3.Lignes)
                $iGel3 = -1; $iIgnore3 = -1; $iRepos3 = -1; $gels3 = 0
                for ($k = 0; $k -lt $l3.Count; $k++) {
                    if ($l3[$k] -match 'etat Repos -> Gel ') { $gels3++; if ($iGel3 -lt 0 -and $l3[$k].Contains('etat Repos -> Gel (DemandeCapture plein_ecran)')) { $iGel3 = $k } }
                    if (($iIgnore3 -lt 0) -and $l3[$k].EndsWith('capture deja en cours: appui ignore')) { $iIgnore3 = $k }
                    if (($iRepos3 -lt 0) -and $l3[$k].Contains('etat Decoupe -> Repos')) { $iRepos3 = $k }
                }
                $pngApres = @(Fichiers-Png $images).Count
                Verifier '(6) ordre : Repos -> Gel, puis appui ignoré, puis Decoupe -> Repos' (($iGel3 -ge 0) -and ($iIgnore3 -gt $iGel3) -and ($iRepos3 -gt $iIgnore3)) "Gel $iGel3, ignoré $iIgnore3, Repos $iRepos3" | Out-Null
                Verifier '(6) une seule capture : un Gel, un PNG de plus, rien de visible' (($gels3 -eq 1) -and ($pngApres -eq $nPng + 1) -and ($c3.Vues -eq 0)) "Gel $gels3, PNG $nPng → $pngApres, $($c3.Vues) relevé(s) visible(s)" | Out-Null
                # Copie en échec, fichier fait : « Impossible de copier » (sortie/annonce.rs).
                $t3 = $l3 -join "`n"
                Verifier '(6) copie en échec, fichier enregistré : « Impossible de copier »' (($t3.Contains('sortie copie echec recent ok fichier fait')) -and ($t3.Contains("sortie annonce erreur: Impossible de copier"))) | Out-Null

                # Puis une capture normale : l'app capture de nouveau, le presse-papiers remarche.
                Start-Sleep -Milliseconds 300
                $c4 = Capturer-PleinEcran -Dossier $da -Quoi '(6) capture suivante' -Guettees $guettees
                $t4 = $c4.Lignes -join "`n"
                Verifier '(6) après, l''app capture de nouveau : Copié et enregistré' (($t4.Contains('etat Decoupe -> Repos (ImagePreteSansVignette)')) -and ($t4.Contains('sortie annonce reussite: Copié et enregistré')) -and (@(Fichiers-Png $images).Count -eq $pngApres + 1)) | Out-Null
                Verifier '(6) le presse-papiers a changé' ([BreezyShotEssai.PleinEcran]::GetClipboardSequenceNumber() -ne $c4.Seq) | Out-Null
                $copie4 = Lire-PressePapiers
                try { Verifier-Image '(6) presse-papiers de la capture suivante' $copie4 $c4.Ecran }
                finally { if ($null -ne $copie4) { $copie4.Dispose() } }

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
