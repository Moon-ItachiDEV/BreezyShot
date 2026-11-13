// Aides natives de la sonde défilante (C#, chargé par commun.ps1 avec Add-Type).
// Seulement des DllImport et des boucles de pixels : compile sans Windows (vérifié sous
// Linux), ne marche que sous Windows.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace BreezyShotSonde
{
    public static class Natif
    {
        // La marque des molettes envoyées par la sonde (dwExtraInfo), comme celle de l'app.
        public const long MARQUE = 0x425A5953;

        [StructLayout(LayoutKind.Sequential)]
        public struct POINT { public int X; public int Y; }

        [StructLayout(LayoutKind.Sequential)]
        public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

        [StructLayout(LayoutKind.Sequential)]
        struct MOUSEINPUT { public int dx; public int dy; public int mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }

        [StructLayout(LayoutKind.Sequential)]
        struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }

        [StructLayout(LayoutKind.Explicit)]
        struct UNION { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; }

        [StructLayout(LayoutKind.Sequential)]
        struct INPUT { public uint type; public UNION u; }

        [StructLayout(LayoutKind.Sequential)]
        public struct CURSORINFO { public int cbSize; public int flags; public IntPtr hCursor; public POINT pt; }

        [StructLayout(LayoutKind.Sequential)]
        struct PROCESS_MEMORY_COUNTERS
        {
            public uint cb; public uint PageFaultCount; public UIntPtr PeakWorkingSetSize; public UIntPtr WorkingSetSize;
            public UIntPtr QuotaPeakPagedPoolUsage; public UIntPtr QuotaPagedPoolUsage; public UIntPtr QuotaPeakNonPagedPoolUsage;
            public UIntPtr QuotaNonPagedPoolUsage; public UIntPtr PagefileUsage; public UIntPtr PeakPagefileUsage;
        }


        delegate bool Rappel(IntPtr h, IntPtr p);

        [DllImport("user32.dll", SetLastError = true)] public static extern bool SetProcessDpiAwarenessContext(IntPtr contexte);
        [DllImport("user32.dll")] static extern uint SendInput(uint n, INPUT[] entrees, int taille);
        [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
        [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
        [DllImport("user32.dll")] public static extern bool GetCursorInfo(ref CURSORINFO c);
        [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT p);
        [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint quoi);
        [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
        [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
        [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
        [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
        [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
        [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
        [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
        [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
        [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int commande);
        [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int l, int hh, bool repeindre);
        [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr apres, int x, int y, int l, int hh, uint drapeaux);
        [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder t, int n);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder t, int n);
        [DllImport("user32.dll")] static extern bool EnumWindows(Rappel f, IntPtr p);
        [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent, Rappel f, IntPtr p);
        [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")] public static extern IntPtr GetWindowLongPtr(IntPtr h, int index);
        [DllImport("user32.dll", SetLastError = true)] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
        [DllImport("user32.dll", SetLastError = true)] static extern bool SystemParametersInfo(uint action, uint ui, ref uint pv, uint ini);
        [DllImport("user32.dll", SetLastError = true, EntryPoint = "SystemParametersInfoW")] static extern bool SystemParametersInfoValeur(uint action, uint ui, IntPtr pv, uint ini);
        [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
        [DllImport("user32.dll")] public static extern uint GetDpiForSystem();
        [DllImport("user32.dll")] static extern uint GetGuiResources(IntPtr p, uint quoi);
        [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
        [DllImport("user32.dll")] static extern bool AllowSetForegroundWindow(int pid);
        [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int taille);
        [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h, int attr, out int v, int taille);
        [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
        [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint acces, bool herite, int pid);
        [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
        [DllImport("psapi.dll")] static extern bool GetProcessMemoryInfo(IntPtr p, out PROCESS_MEMORY_COUNTERS c, uint taille);

        // -----------------------------------------------------------------------------
        // Fenêtres

        public static int Pid(IntPtr h) { uint p; GetWindowThreadProcessId(h, out p); return (int)p; }
        public static string Classe(IntPtr h) { var t = new StringBuilder(256); GetClassName(h, t, t.Capacity); return t.ToString(); }
        public static string Titre(IntPtr h) { var t = new StringBuilder(512); GetWindowText(h, t, t.Capacity); return t.ToString(); }
        public static IntPtr SousPoint(int x, int y) { POINT p; p.X = x; p.Y = y; return WindowFromPoint(p); }
        public static IntPtr RacineSousPoint(int x, int y) { return GetAncestor(SousPoint(x, y), 2); }
        public static long StyleEtendu(IntPtr h) { return GetWindowLongPtr(h, -20).ToInt64(); }
        public static long Style(IntPtr h) { return GetWindowLongPtr(h, -16).ToInt64(); }
        public static bool Masquee(IntPtr h) { int v; return DwmGetWindowAttribute(h, 14, out v, 4) == 0 && v != 0; }

        /// Vrai bord (DWMWA_EXTENDED_FRAME_BOUNDS), sinon GetWindowRect.
        public static RECT Bord(IntPtr h)
        {
            RECT r;
            if (DwmGetWindowAttribute(h, 9, out r, Marshal.SizeOf(typeof(RECT))) == 0) return r;
            GetWindowRect(h, out r);
            return r;
        }

        /// Zone cliente en coordonnées de l'écran : x, y, l, h.
        public static int[] Client(IntPtr h)
        {
            RECT r; GetClientRect(h, out r);
            POINT p; p.X = 0; p.Y = 0; ClientToScreen(h, ref p);
            return new int[] { p.X, p.Y, r.Right - r.Left, r.Bottom - r.Top };
        }

        /// Fenêtres de premier niveau visibles, du premier plan vers le fond.
        public static IntPtr[] Fenetres(int pid)
        {
            var l = new List<IntPtr>();
            EnumWindows((h, p) => { if (IsWindowVisible(h) && (pid == 0 || Pid(h) == pid)) l.Add(h); return true; }, IntPtr.Zero);
            return l.ToArray();
        }

        /// Comme `gel::fenetres_visibles` : visibles, non réduites, non masquées, pas
        /// « en couches et transparentes », pas le bureau, d'aire non nulle ; sans `exclure`.
        public static IntPtr[] FenetresVisibles(int exclure)
        {
            var l = new List<IntPtr>();
            EnumWindows((h, p) =>
            {
                if (!IsWindowVisible(h) || IsIconic(h) || Pid(h) == exclure || Masquee(h)) return true;
                long ex = StyleEtendu(h);
                if ((ex & 0x80000) != 0 && (ex & 0x20) != 0) return true;
                string c = Classe(h);
                if (c == "Progman" || c == "WorkerW") return true;
                RECT r = Bord(h);
                if (r.Right - r.Left <= 0 || r.Bottom - r.Top <= 0) return true;
                l.Add(h);
                return true;
            }, IntPtr.Zero);
            return l.ToArray();
        }

        public static IntPtr[] Filles(IntPtr parent)
        {
            var l = new List<IntPtr>();
            EnumChildWindows(parent, (h, p) => { l.Add(h); return true; }, IntPtr.Zero);
            return l.ToArray();
        }

        /// Profondeur d'une fille sous `racine` (0 : la racine elle-même).
        public static int Profondeur(IntPtr h, IntPtr racine)
        {
            int n = 0;
            while (h != IntPtr.Zero && h != racine && n < 64) { h = GetAncestor(h, 1); n++; }
            return n;
        }

        public static string Decrire(IntPtr h)
        {
            if (h == IntPtr.Zero) return "aucune";
            string nom = "?";
            try { nom = Process.GetProcessById(Pid(h)).ProcessName; } catch { }
            string t = Titre(h);
            if (t.Length > 50) t = t.Substring(0, 50);
            return Classe(h) + " « " + t + " » (" + nom + ")";
        }

        /// Met `h` au premier plan ; rend vrai si c'est fait.
        public static bool PremierPlan(IntPtr h)
        {
            for (int essai = 0; essai < 3; essai++)
            {
                if (GetForegroundWindow() == h) return true;
                AllowSetForegroundWindow(-1);
                if (essai == 1) { Touche(0x12, false); }   // Alt seul : rend le droit de premier plan
                BringWindowToTop(h);
                SetForegroundWindow(h);
                System.Threading.Thread.Sleep(80);
            }
            return GetForegroundWindow() == h;
        }

        // -----------------------------------------------------------------------------
        // Entrées

        static uint Envoyer(INPUT i) { return SendInput(1, new INPUT[] { i }, Marshal.SizeOf(typeof(INPUT))); }

        public static void Poser(int x, int y)
        {
            SetCursorPos(x, y);
            INPUT i = new INPUT(); i.type = 0; i.u.mi.dwFlags = 0x1; Envoyer(i);
        }

        /// Un cran de molette (`crans` × 120, négatif : vers le bas) par SendInput, avec la
        /// marque donnée dans dwExtraInfo. Rend le nombre d'entrées acceptées (1) ou 0.
        public static uint Molette(int crans, long marque)
        {
            INPUT i = new INPUT();
            i.type = 0;
            i.u.mi.dwFlags = 0x0800;
            i.u.mi.mouseData = crans * 120;
            i.u.mi.dwExtraInfo = new IntPtr(marque);
            return Envoyer(i);
        }

        /// Une touche (appui puis relâché) par SendInput.
        public static uint Touche(ushort vk, bool etendue)
        {
            INPUT a = new INPUT(); a.type = 1; a.u.ki.wVk = vk; a.u.ki.dwFlags = etendue ? 1u : 0u;
            INPUT b = a; b.u.ki.dwFlags |= 2;
            uint n = SendInput(2, new INPUT[] { a, b }, Marshal.SizeOf(typeof(INPUT)));
            return n;
        }

        static IntPtr MotLong(int bas, int haut) { return new IntPtr(((long)(short)haut << 16) | (ushort)(short)bas); }

        /// WM_MOUSEWHEEL posté à `h`, coordonnées de l'écran (x, y). Rend 0 ou le code d'erreur.
        public static int PosterMolette(IntPtr h, int crans, int x, int y)
        {
            IntPtr w = new IntPtr((long)(crans * 120) << 16);
            return PostMessage(h, 0x020A, w, MotLong(x, y)) ? 0 : Marshal.GetLastWin32Error();
        }

        /// WM_VSCROLL (code SB_*) posté à `h`. Rend 0 ou le code d'erreur.
        public static int PosterDefilement(IntPtr h, int code)
        {
            return PostMessage(h, 0x0115, new IntPtr(code), IntPtr.Zero) ? 0 : Marshal.GetLastWin32Error();
        }

        // -----------------------------------------------------------------------------
        // Réglages

        public static int LireRoutageMolette()
        {
            uint v = 0;
            return SystemParametersInfo(0x201C, 0, ref v, 0) ? (int)v : -1 - Marshal.GetLastWin32Error();
        }

        /// SPI_SETMOUSEWHEELROUTING : la valeur dans pvParam (doc : « Set pvParam to the
        /// value »), avec SPIF_SENDCHANGE. Rend la valeur relue.
        public static int FixerRoutageMolette(int valeur)
        {
            SystemParametersInfoValeur(0x201D, 0, new IntPtr(valeur), 0x2);
            return LireRoutageMolette();
        }

        public static int LignesParCran()
        {
            uint v = 0;
            return SystemParametersInfo(0x0068, 0, ref v, 0) ? (int)v : -1;
        }

        // -----------------------------------------------------------------------------
        // Échelle de l'écran principal par DisplayConfig (paquets non documentés -3 et -4,
        // DISPLAYCONFIG_DEVICE_INFO_GET/SET_DPI_SCALE, ceux de l'app Paramètres) : S8.

        [DllImport("user32.dll")] static extern int GetDisplayConfigBufferSizes(uint drapeaux, out uint chemins, out uint modes);
        [DllImport("user32.dll")] static extern int QueryDisplayConfig(uint drapeaux, ref uint chemins, IntPtr tChemins, ref uint modes, IntPtr tModes, IntPtr topologie);
        [DllImport("user32.dll")] static extern int DisplayConfigGetDeviceInfo(IntPtr paquet);
        [DllImport("user32.dll")] static extern int DisplayConfigSetDeviceInfo(IntPtr paquet);
        [DllImport("shcore.dll")] static extern int GetDpiForMonitor(IntPtr ecran, int genre, out uint x, out uint y);
        [DllImport("user32.dll")] static extern IntPtr MonitorFromPoint(POINT p, uint drapeaux);

        /// Adaptateur (LUID bas, haut) et source du premier chemin actif, ou null.
        static int[] PremiereSource()
        {
            uint nc, nm;
            if (GetDisplayConfigBufferSizes(2, out nc, out nm) != 0 || nc == 0) return null;
            IntPtr c = Marshal.AllocHGlobal((int)nc * 72), m = Marshal.AllocHGlobal((int)nm * 64);
            try
            {
                if (QueryDisplayConfig(2, ref nc, c, ref nm, m, IntPtr.Zero) != 0) return null;
                return new int[] { Marshal.ReadInt32(c, 0), Marshal.ReadInt32(c, 4), Marshal.ReadInt32(c, 8) };
            }
            finally { Marshal.FreeHGlobal(c); Marshal.FreeHGlobal(m); }
        }

        /// {code, min, actuel, max} : pas relatifs à l'échelle recommandée (0 = recommandée).
        public static int[] EchelleRelative()
        {
            int[] s = PremiereSource();
            if (s == null) return new int[] { -1, 0, 0, 0 };
            IntPtr p = Marshal.AllocHGlobal(32);
            try
            {
                Marshal.WriteInt32(p, 0, -3); Marshal.WriteInt32(p, 4, 32);
                Marshal.WriteInt32(p, 8, s[0]); Marshal.WriteInt32(p, 12, s[1]); Marshal.WriteInt32(p, 16, s[2]);
                int r = DisplayConfigGetDeviceInfo(p);
                return new int[] { r, Marshal.ReadInt32(p, 20), Marshal.ReadInt32(p, 24), Marshal.ReadInt32(p, 28) };
            }
            finally { Marshal.FreeHGlobal(p); }
        }

        /// Pose l'échelle relative `rel` ; rend le code (0 : accepté).
        public static int FixerEchelleRelative(int rel)
        {
            int[] s = PremiereSource();
            if (s == null) return -1;
            IntPtr p = Marshal.AllocHGlobal(24);
            try
            {
                Marshal.WriteInt32(p, 0, -4); Marshal.WriteInt32(p, 4, 24);
                Marshal.WriteInt32(p, 8, s[0]); Marshal.WriteInt32(p, 12, s[1]); Marshal.WriteInt32(p, 16, s[2]);
                Marshal.WriteInt32(p, 20, rel);
                return DisplayConfigSetDeviceInfo(p);
            }
            finally { Marshal.FreeHGlobal(p); }
        }

        /// DPI effectif de l'écran qui contient (x, y) (MDT_EFFECTIVE_DPI), ou 0.
        public static uint DpiEcran(int x, int y)
        {
            POINT p; p.X = x; p.Y = y;
            uint dx, dy;
            return GetDpiForMonitor(MonitorFromPoint(p, 2), 0, out dx, out dy) == 0 ? dx : 0;
        }

        // -----------------------------------------------------------------------------
        // Ressources

        public static uint ObjetsGdi(int pid)
        {
            IntPtr p = OpenProcess(0x1000, false, pid);
            if (p == IntPtr.Zero) return 0;
            try { return GetGuiResources(p, 0); } finally { CloseHandle(p); }
        }

        public static long MemoirePrivee()
        {
            PROCESS_MEMORY_COUNTERS c;
            GetProcessMemoryInfo(GetCurrentProcess(), out c, (uint)Marshal.SizeOf(typeof(PROCESS_MEMORY_COUNTERS)));
            return (long)c.PagefileUsage.ToUInt64();
        }

        public static bool PointeurMontre()
        {
            CURSORINFO c = new CURSORINFO(); c.cbSize = Marshal.SizeOf(typeof(CURSORINFO));
            return GetCursorInfo(ref c) && (c.flags & 1) != 0;
        }

        // -----------------------------------------------------------------------------
        // Pixels : BGRA 32 bits, de haut en bas, largeur l.

        /// Marque de ligne des témoins et de longue.html : la ligne y du contenu (18 bits)
        /// porte, sur la bande de gauche, R = y & 0xFF, V = 0x40 | (y >> 8) & 0x3F,
        /// B = 0xA0 | (y >> 14) & 0x0F.
        public static int CouleurLigne(int y) { return (0xFF << 24) | ((y & 0xFF) << 16) | ((0x40 | ((y >> 8) & 0x3F)) << 8) | (0xA0 | ((y >> 14) & 0x0F)); }

        /// Le y du contenu lu au pixel (x, ligne) de l'image, ou -1.
        public static int LireLigne(byte[] a, int l, int x, int ligne)
        {
            int i = (ligne * l + x) * 4;
            if (x < 0 || x >= l || i < 0 || i + 3 >= a.Length) return -1;
            int b = a[i], v = a[i + 1], r = a[i + 2];
            if ((b & 0xF0) != 0xA0 || (v & 0xC0) != 0x40) return -1;
            return ((b & 0x0F) << 14) | ((v & 0x3F) << 8) | r;
        }

        /// Décalage vrai lu sur la bande : pour chaque ligne de [r0, r1) qui porte une marque,
        /// le décalage en pixels réels vérifie floor((ligne + d) / echelle) = y lu, donc
        /// d ∈ [y·e − ligne, (y+1)·e − ligne). Rend l'intersection {bas, haut, lignes lues},
        /// ou null si aucune ligne n'est lue ou si les lignes se contredisent.
        public static double[] DecalageBande(byte[] a, int l, int x, int r0, int r1, double echelle)
        {
            double bas = double.NegativeInfinity, haut = double.PositiveInfinity; int n = 0;
            for (int ligne = r0; ligne < r1; ligne++)
            {
                int y = LireLigne(a, l, x, ligne);
                if (y < 0) continue;
                n++;
                bas = Math.Max(bas, y * echelle - ligne);
                haut = Math.Min(haut, (y + 1) * echelle - ligne);
            }
            if (n == 0 || bas > haut + 1e-6) return null;
            return new double[] { bas, haut, n };
        }

        /// Première colonne de [0, max) où une marque de ligne se lit dans au moins la
        /// moitié des lignes [r0, r1), ou -1.
        public static int ColonneBande(byte[] a, int l, int h, int max, int r0, int r1)
        {
            for (int x = 0; x < Math.Min(max, l); x++)
            {
                int n = 0, t = 0;
                for (int ligne = Math.Max(0, r0); ligne < Math.Min(h, r1); ligne++) { t++; if (LireLigne(a, l, x, ligne) >= 0) n++; }
                if (t > 0 && n * 2 >= t) return x;
            }
            return -1;
        }

        static ulong[] Empreintes(byte[] a, int l, int h, int x0, int x1, out bool[] unie)
        {
            var e = new ulong[h]; unie = new bool[h];
            for (int y = 0; y < h; y++)
            {
                ulong f = 1469598103934665603UL; int p0 = (y * l + x0) * 4; bool u = true;
                for (int x = x0; x < x1; x++)
                {
                    int i = (y * l + x) * 4;
                    if (a[i] != a[p0] || a[i + 1] != a[p0 + 1] || a[i + 2] != a[p0 + 2]) u = false;
                    f = (f ^ a[i]) * 1099511628211UL; f = (f ^ a[i + 1]) * 1099511628211UL; f = (f ^ a[i + 2]) * 1099511628211UL;
                }
                e[y] = f; unie[y] = u;
            }
            return e;
        }

        /// Décalage vertical d de `b` par rapport à `a` (contenu monté de d : b[y] = a[y + d]),
        /// cherché dans [-max, max] sur les colonnes [x0, x1) : le d qui rend égales au bit
        /// près le plus de lignes non unies. Rend {d, lignes égales, lignes comparées,
        /// lignes égales à d = 0}. Une page immobile rend d = 0.
        public static int[] Decalage(byte[] a, byte[] b, int l, int h, int x0, int x1, int max)
        {
            bool[] ua, ub;
            ulong[] ea = Empreintes(a, l, h, x0, x1, out ua);
            ulong[] eb = Empreintes(b, l, h, x0, x1, out ub);
            int meilleur = 0, score = -1, compare = 0, zero = 0;
            for (int d = -max; d <= max; d++)
            {
                int n = 0, c = 0;
                for (int y = Math.Max(0, -d); y < h && y + d < h; y++)
                {
                    if (ub[y]) continue;
                    c++;
                    if (eb[y] == ea[y + d]) n++;
                }
                if (d == 0) zero = n;
                if (n > score || (n == score && Math.Abs(d) < Math.Abs(meilleur))) { score = n; meilleur = d; compare = c; }
            }
            return new int[] { meilleur, score, compare, zero };
        }

        /// Pour le décalage d (b[y] = a[y + d]) sur les lignes [y0, y1) et les colonnes
        /// [x0, x1) de b : {lignes égales au bit près, lignes égales à la tolérance de D12
        /// (90 % des pixels à ± 8 par canal), lignes comparées, pixels égaux au bit près,
        /// pixels à ± 8, pixels comparés}.
        public static long[] Comparer(byte[] a, byte[] b, int l, int h, int d, int y0, int y1, int x0, int x1)
        {
            long lignesExactes = 0, lignesTol = 0, lignes = 0, pixExacts = 0, pixTol = 0, pix = 0;
            x0 = Math.Max(0, x0); x1 = Math.Min(l, x1);
            int n = x1 - x0;
            if (n <= 0) return new long[6];
            for (int y = Math.Max(y0, Math.Max(0, -d)); y < Math.Min(y1, h) && y + d < h; y++)
            {
                lignes++; int ex = 0, tol = 0;
                for (int x = x0; x < x1; x++)
                {
                    int i = (y * l + x) * 4, j = ((y + d) * l + x) * 4;
                    int d0 = Math.Abs(b[i] - a[j]), d1 = Math.Abs(b[i + 1] - a[j + 1]), d2 = Math.Abs(b[i + 2] - a[j + 2]);
                    if (d0 == 0 && d1 == 0 && d2 == 0) ex++;
                    if (d0 <= 8 && d1 <= 8 && d2 <= 8) tol++;
                }
                pix += n; pixExacts += ex; pixTol += tol;
                if (ex == n) lignesExactes++;
                if (tol * 10 >= n * 9) lignesTol++;
            }
            return new long[] { lignesExactes, lignesTol, lignes, pixExacts, pixTol, pix };
        }

        /// Pixels différents (B, V ou R) entre deux images de même taille, dans le rectangle
        /// [x0, x1) × [y0, y1) ; -1 si les tailles diffèrent.
        public static int Differences(byte[] a, byte[] b, int l, int x0, int y0, int x1, int y1)
        {
            if (a == null || b == null || a.Length != b.Length) return -1;
            int h = a.Length / 4 / l, n = 0;
            for (int y = Math.Max(0, y0); y < Math.Min(h, y1); y++)
                for (int x = Math.Max(0, x0); x < Math.Min(l, x1); x++)
                {
                    int i = (y * l + x) * 4;
                    if (a[i] != b[i] || a[i + 1] != b[i + 1] || a[i + 2] != b[i + 2]) n++;
                }
            return n;
        }

        public static bool Egales(byte[] a, byte[] b)
        {
            if (a == null || b == null || a.Length != b.Length) return false;
            for (int i = 0; i < a.Length; i += 4)
                if (a[i] != b[i] || a[i + 1] != b[i + 1] || a[i + 2] != b[i + 2]) return false;
            return true;
        }

        /// Part (0 à 1) des pixels presque noirs (somme des canaux ≤ 24) du rectangle.
        public static double PartNoire(byte[] a, int l, int x0, int y0, int x1, int y1)
        {
            int h = a.Length / 4 / l; long n = 0, t = 0;
            for (int y = Math.Max(0, y0); y < Math.Min(h, y1); y++)
                for (int x = Math.Max(0, x0); x < Math.Min(l, x1); x++)
                {
                    int i = (y * l + x) * 4; t++;
                    if (a[i] + a[i + 1] + a[i + 2] <= 24) n++;
                }
            return t == 0 ? 1.0 : (double)n / t;
        }

        /// Pixels proches d'une couleur (± tol par canal) dans tout `a`.
        public static int PixelsProches(byte[] a, int r, int v, int bl, int tol)
        {
            int n = 0;
            for (int i = 0; i + 3 < a.Length; i += 4)
                if (Math.Abs(a[i] - bl) <= tol && Math.Abs(a[i + 1] - v) <= tol && Math.Abs(a[i + 2] - r) <= tol) n++;
            return n;
        }

        /// Recadre une image BGRA.
        public static byte[] Recadrer(byte[] a, int l, int x, int y, int cl, int ch)
        {
            var s = new byte[cl * ch * 4];
            for (int r = 0; r < ch; r++) Buffer.BlockCopy(a, ((y + r) * l + x) * 4, s, r * cl * 4, cl * 4);
            return s;
        }

        /// Écrit la bande de marques de lignes (colonnes [0, largeur)) dans une image BGRA
        /// en mémoire (pointeur, pas en octets), lignes [0, hauteur), décalée de `origine`.
        public static void PeindreBande(IntPtr scan0, int pas, int largeur, int hauteur, int origine)
        {
            for (int y = 0; y < hauteur; y++)
            {
                int c = CouleurLigne(y + origine);
                for (int x = 0; x < largeur; x++) Marshal.WriteInt32(scan0, y * pas + x * 4, c);
            }
        }

        /// La bande en entiers BGRA (pour une WriteableBitmap WPF).
        public static int[] Bande(int largeur, int hauteur)
        {
            var t = new int[largeur * hauteur];
            for (int y = 0; y < hauteur; y++) { int c = CouleurLigne(y); for (int x = 0; x < largeur; x++) t[y * largeur + x] = c; }
            return t;
        }
    }

    /// Une prise d'écran réutilisée (moyen 1 de S2) : BitBlt du DC de l'écran dans une
    /// section DIB 32 bits de la taille de la zone.
    public sealed class Prise : IDisposable
    {
        [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr h);
        [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr h, IntPtr dc);
        [DllImport("gdi32.dll")] static extern IntPtr CreateCompatibleDC(IntPtr dc);
        [DllImport("gdi32.dll")] static extern bool DeleteDC(IntPtr dc);
        [DllImport("gdi32.dll")] static extern IntPtr SelectObject(IntPtr dc, IntPtr o);
        [DllImport("gdi32.dll")] static extern bool DeleteObject(IntPtr o);
        [DllImport("gdi32.dll")] static extern bool GdiFlush();
        [DllImport("gdi32.dll", SetLastError = true)] static extern bool BitBlt(IntPtr dest, int x, int y, int l, int h, IntPtr src, int sx, int sy, uint rop);
        [StructLayout(LayoutKind.Sequential)]
        struct BIH
        {
            public uint biSize; public int biWidth; public int biHeight; public ushort biPlanes; public ushort biBitCount;
            public uint biCompression; public uint biSizeImage; public int biXPelsPerMeter; public int biYPelsPerMeter;
            public uint biClrUsed; public uint biClrImportant;
        }
        [DllImport("gdi32.dll", SetLastError = true)] static extern IntPtr CreateDIBSection(IntPtr dc, ref BIH bmi, uint usage, out IntPtr bits, IntPtr section, uint offset);

        IntPtr ecran, dc, bitmap, ancien, bits;
        public readonly int X, Y, L, H;
        public int DerniereErreur;

        public Prise(int x, int y, int l, int h)
        {
            X = x; Y = y; L = l; H = h;
            ecran = GetDC(IntPtr.Zero);
            dc = CreateCompatibleDC(ecran);
            BIH b = new BIH(); b.biSize = (uint)Marshal.SizeOf(typeof(BIH)); b.biWidth = l; b.biHeight = -h; b.biPlanes = 1; b.biBitCount = 32;
            bitmap = CreateDIBSection(dc, ref b, 0, out bits, IntPtr.Zero, 0);
            if (bitmap == IntPtr.Zero) { DerniereErreur = Marshal.GetLastWin32Error(); Dispose(); throw new InvalidOperationException("CreateDIBSection a échoué (" + DerniereErreur + ")"); }
            ancien = SelectObject(dc, bitmap);
        }

        /// Une image de la zone (BGRA) ; null si BitBlt échoue (code dans DerniereErreur).
        public byte[] Prendre(bool captureblt)
        {
            uint rop = 0x00CC0020u | (captureblt ? 0x40000000u : 0u);
            if (!BitBlt(dc, 0, 0, L, H, ecran, X, Y, rop)) { DerniereErreur = Marshal.GetLastWin32Error(); return null; }
            GdiFlush();
            var o = new byte[L * H * 4];
            Marshal.Copy(bits, o, 0, o.Length);
            return o;
        }

        static bool MemeCle(byte[] a, byte[] b, int l, int colonne, int largeur)
        {
            int h = a.Length / 4 / l;
            for (int y = 0; y < h; y++)
            {
                int x0 = colonne >= 0 ? colonne : 0, x1 = colonne >= 0 ? colonne + 1 : largeur;
                for (int x = x0; x < x1; x++)
                {
                    int i = (y * l + x) * 4;
                    if (a[i] != b[i] || a[i + 1] != b[i + 1] || a[i + 2] != b[i + 2]) return false;
                }
            }
            return true;
        }

        /// Images toutes les ~15 ms jusqu'à deux égales de suite, sur la colonne `colonne`
        /// (la bande : les animations de la page n'y sont pas) ou, si colonne < 0, sur les
        /// colonnes [0, largeur). `depart` : chrono démarré à la commande. Rend la dernière
        /// image ; `ms` : temps du départ à la première des deux égales ; `stable` : faux si
        /// `maxMs` est passé avant ; `images` : nombre d'images prises.
        public byte[] Stable(int colonne, int largeur, int maxMs, Stopwatch depart, out double ms, out bool stable, out int images)
        {
            byte[] avant = Prendre(false);
            if (avant == null) { ms = 0; stable = false; images = 0; return null; }
            double tAvant = depart.Elapsed.TotalMilliseconds;
            images = 1;
            while (depart.Elapsed.TotalMilliseconds < maxMs)
            {
                System.Threading.Thread.Sleep(12);
                byte[] img = Prendre(false);
                if (img == null) break;
                images++;
                if (MemeCle(avant, img, L, colonne, largeur)) { ms = tAvant; stable = true; return img; }
                avant = img; tAvant = depart.Elapsed.TotalMilliseconds;
            }
            ms = tAvant; stable = false;
            return avant;
        }

        public void Dispose()
        {
            if (dc != IntPtr.Zero && ancien != IntPtr.Zero) SelectObject(dc, ancien);
            if (bitmap != IntPtr.Zero) DeleteObject(bitmap);
            if (dc != IntPtr.Zero) DeleteDC(dc);
            if (ecran != IntPtr.Zero) ReleaseDC(IntPtr.Zero, ecran);
            dc = bitmap = ecran = ancien = IntPtr.Zero;
        }
    }
}
