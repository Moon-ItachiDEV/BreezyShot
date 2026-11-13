// S6 : écouter la molette de toute la machine, dans un fil à soi qui a sa boucle de
// messages. Deux voies sur le même fil : un crochet WH_MOUSE_LL (qui ne fait que noter, puis
// passe la main), et l'entrée brute (RIDEV_INPUTSINK sur une fenêtre message seulement).
// Chaque cran noté : horodatage QueryPerformanceCounter (commun à tous les processus),
// donnée (delta), drapeaux (crochet : LLMHF_INJECTED = 1), dwExtraInfo / ulExtraInformation.
// Seulement des DllImport : compile sans Windows (vérifié sous Linux).
using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

namespace BreezyShotSonde
{
    public static class Ecoute
    {
        delegate IntPtr ProcBas(int code, IntPtr w, IntPtr l);
        delegate IntPtr ProcFenetre(IntPtr h, uint msg, IntPtr w, IntPtr l);

        [StructLayout(LayoutKind.Sequential)]
        struct MSLLHOOKSTRUCT { public int X; public int Y; public uint mouseData; public uint flags; public uint time; public IntPtr dwExtraInfo; }

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        struct WNDCLASSEX
        {
            public uint cbSize; public uint style; public ProcFenetre lpfnWndProc; public int cbClsExtra; public int cbWndExtra;
            public IntPtr hInstance; public IntPtr hIcon; public IntPtr hCursor; public IntPtr hbrBackground;
            public string lpszMenuName; public string lpszClassName; public IntPtr hIconSm;
        }

        [StructLayout(LayoutKind.Sequential)]
        struct RAWINPUTDEVICE { public ushort usUsagePage; public ushort usUsage; public uint dwFlags; public IntPtr hwndTarget; }

        [StructLayout(LayoutKind.Sequential)]
        struct MSG { public IntPtr hwnd; public uint message; public IntPtr wParam; public IntPtr lParam; public uint time; public int x; public int y; }

        [DllImport("user32.dll", SetLastError = true)] static extern IntPtr SetWindowsHookEx(int id, ProcBas proc, IntPtr module, uint fil);
        [DllImport("user32.dll", SetLastError = true)] static extern bool UnhookWindowsHookEx(IntPtr h);
        [DllImport("user32.dll")] static extern IntPtr CallNextHookEx(IntPtr h, int code, IntPtr w, IntPtr l);
        [DllImport("user32.dll")] static extern int GetMessage(out MSG m, IntPtr h, uint min, uint max);
        [DllImport("user32.dll")] static extern bool TranslateMessage(ref MSG m);
        [DllImport("user32.dll")] static extern IntPtr DispatchMessage(ref MSG m);
        [DllImport("user32.dll")] static extern bool PostThreadMessage(uint fil, uint msg, IntPtr w, IntPtr l);
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)] static extern ushort RegisterClassEx(ref WNDCLASSEX c);
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        static extern IntPtr CreateWindowEx(uint ex, string classe, string titre, uint style, int x, int y, int l, int h, IntPtr parent, IntPtr menu, IntPtr inst, IntPtr p);
        [DllImport("user32.dll")] static extern bool DestroyWindow(IntPtr h);
        [DllImport("user32.dll")] static extern IntPtr DefWindowProc(IntPtr h, uint msg, IntPtr w, IntPtr l);
        [DllImport("user32.dll", SetLastError = true)] static extern bool RegisterRawInputDevices(RAWINPUTDEVICE[] d, uint n, uint taille);
        [DllImport("user32.dll")] static extern uint GetRawInputData(IntPtr h, uint quoi, IntPtr donnees, ref uint taille, uint entete);
        [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern IntPtr GetModuleHandle(string nom);

        const int MAX = 20000;
        public static readonly long[] CTemps = new long[MAX];
        public static readonly int[] CDonnee = new int[MAX];
        public static readonly uint[] CDrapeaux = new uint[MAX];
        public static readonly long[] CExtra = new long[MAX];
        public static int NCrochet;
        public static readonly long[] RTemps = new long[MAX];
        public static readonly int[] RDonnee = new int[MAX];
        public static readonly long[] RExtra = new long[MAX];
        public static int NBrut;
        public static string ErreurCrochet = "";
        public static string ErreurBrut = "";
        public static volatile bool Pret;

        static ProcBas procBas;
        static ProcFenetre procFenetre;
        static IntPtr crochet = IntPtr.Zero;
        static IntPtr fenetre = IntPtr.Zero;
        static uint fil;
        static Thread filGere;
        static IntPtr tampon = IntPtr.Zero;

        static IntPtr Crochet(int code, IntPtr w, IntPtr l)
        {
            if (code >= 0 && w.ToInt64() == 0x020A)
            {
                int i = Interlocked.Increment(ref NCrochet) - 1;
                if (i < MAX)
                {
                    CTemps[i] = Stopwatch.GetTimestamp();
                    MSLLHOOKSTRUCT s = (MSLLHOOKSTRUCT)Marshal.PtrToStructure(l, typeof(MSLLHOOKSTRUCT));
                    CDonnee[i] = (short)(s.mouseData >> 16);
                    CDrapeaux[i] = s.flags;
                    CExtra[i] = s.dwExtraInfo.ToInt64();
                }
            }
            return CallNextHookEx(crochet, code, w, l);
        }

        static IntPtr Fenetre(IntPtr h, uint msg, IntPtr w, IntPtr l)
        {
            if (msg == 0x00FF)
            {
                uint taille = 256;
                uint entete = (uint)(8 + 2 * IntPtr.Size);
                if (GetRawInputData(l, 0x10000003, tampon, ref taille, entete) != uint.MaxValue)
                {
                    int type = Marshal.ReadInt32(tampon, 0);
                    int m = (int)entete;
                    ushort boutons = (ushort)Marshal.ReadInt16(tampon, m + 4);
                    if (type == 0 && (boutons & 0x0400) != 0)
                    {
                        int i = Interlocked.Increment(ref NBrut) - 1;
                        if (i < MAX)
                        {
                            RTemps[i] = Stopwatch.GetTimestamp();
                            RDonnee[i] = Marshal.ReadInt16(tampon, m + 6);
                            RExtra[i] = (uint)Marshal.ReadInt32(tampon, m + 20);
                        }
                    }
                }
            }
            return DefWindowProc(h, msg, w, l);
        }

        static void Boucle()
        {
            fil = GetCurrentThreadId();
            tampon = Marshal.AllocHGlobal(256);
            procBas = Crochet;
            procFenetre = Fenetre;
            IntPtr module = GetModuleHandle(null);
            crochet = SetWindowsHookEx(14, procBas, module, 0);
            if (crochet == IntPtr.Zero)
            {
                ErreurCrochet = "SetWindowsHookEx(module de l'exe) : " + Marshal.GetLastWin32Error();
                crochet = SetWindowsHookEx(14, procBas, GetModuleHandle("user32.dll"), 0);
                if (crochet == IntPtr.Zero) ErreurCrochet += " ; SetWindowsHookEx(user32) : " + Marshal.GetLastWin32Error();
                else ErreurCrochet = "";
            }
            WNDCLASSEX c = new WNDCLASSEX();
            c.cbSize = (uint)Marshal.SizeOf(typeof(WNDCLASSEX));
            c.lpfnWndProc = procFenetre;
            c.hInstance = module;
            c.lpszClassName = "BreezyShotSondeEcoute";
            if (RegisterClassEx(ref c) == 0) ErreurBrut = "RegisterClassEx : " + Marshal.GetLastWin32Error();
            fenetre = CreateWindowEx(0, "BreezyShotSondeEcoute", "", 0, 0, 0, 0, 0, new IntPtr(-3), IntPtr.Zero, module, IntPtr.Zero);
            if (fenetre == IntPtr.Zero) ErreurBrut += " CreateWindowEx : " + Marshal.GetLastWin32Error();
            else
            {
                RAWINPUTDEVICE d = new RAWINPUTDEVICE();
                d.usUsagePage = 1; d.usUsage = 2; d.dwFlags = 0x100; d.hwndTarget = fenetre;
                if (!RegisterRawInputDevices(new RAWINPUTDEVICE[] { d }, 1, (uint)Marshal.SizeOf(typeof(RAWINPUTDEVICE))))
                    ErreurBrut = "RegisterRawInputDevices : " + Marshal.GetLastWin32Error();
            }
            Pret = true;
            MSG m;
            while (GetMessage(out m, IntPtr.Zero, 0, 0) > 0) { TranslateMessage(ref m); DispatchMessage(ref m); }
            if (crochet != IntPtr.Zero) UnhookWindowsHookEx(crochet);
            if (fenetre != IntPtr.Zero) DestroyWindow(fenetre);
        }

        public static void Demarrer()
        {
            filGere = new Thread(Boucle);
            filGere.IsBackground = true;
            filGere.Start();
            for (int i = 0; i < 100 && !Pret; i++) Thread.Sleep(20);
        }

        public static void Arreter()
        {
            if (fil != 0) PostThreadMessage(fil, 0x0012, IntPtr.Zero, IntPtr.Zero);
            if (filGere != null) filGere.Join(2000);
        }

        public static bool CrochetPose() { return crochet != IntPtr.Zero; }

        /// Tout ce qui a été noté, une ligne par cran : « c <qpc> <delta> <drapeaux> <extra> »
        /// (crochet) ou « r <qpc> <delta> <extra> » (entrée brute).
        public static string Detail()
        {
            var t = new StringBuilder();
            int nc = Math.Min(NCrochet, MAX), nb = Math.Min(NBrut, MAX);
            for (int i = 0; i < nc; i++) t.Append("c ").Append(CTemps[i]).Append(' ').Append(CDonnee[i]).Append(' ').Append(CDrapeaux[i]).Append(' ').Append(CExtra[i]).Append('\n');
            for (int i = 0; i < nb; i++) t.Append("r ").Append(RTemps[i]).Append(' ').Append(RDonnee[i]).Append(' ').Append(RExtra[i]).Append('\n');
            return t.ToString();
        }
    }
}
