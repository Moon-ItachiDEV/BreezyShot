"""Synthèse des sons de capture BreezyShot (originaux, aucun échantillon tiers).
Usage : python3 synth.py <dossier de sortie>
"""
import sys, wave
import numpy as np

SR = 44100
rng = np.random.default_rng(7)


def t_(d):
    return np.arange(int(SR * d)) / SR


def env(n, a, d):
    """attaque a s (cosinus), puis décroissance exponentielle de constante d s"""
    t = np.arange(n) / SR
    e = np.exp(-t / d)
    na = max(1, int(a * SR))
    e[:na] *= 0.5 - 0.5 * np.cos(np.pi * np.arange(na) / na)
    return e


def lowpass(x, fc):
    a = np.exp(-2 * np.pi * fc / SR)
    y = np.zeros_like(x)
    s = 0.0
    for i, v in enumerate(x):
        s = (1 - a) * v + a * s
        y[i] = s
    return y


def lowpass_sweep(x, f0, f1):
    y = np.zeros_like(x)
    s = 0.0
    n = len(x)
    for i, v in enumerate(x):
        fc = f0 + (f1 - f0) * (i / n)
        a = np.exp(-2 * np.pi * fc / SR)
        s = (1 - a) * v + a * s
        y[i] = s
    return y


def place(buf, x, at):
    i = int(at * SR)
    buf[i:i + len(x)] += x[:len(buf) - i]


def finish(x, peak=0.5):
    x = x - np.mean(x)
    x = x / (np.max(np.abs(x)) + 1e-9) * peak
    n = int(0.004 * SR)  # fondu de sortie, pas de clic
    x[-n:] *= np.linspace(1, 0, n)
    return x


def goutte():
    """Une goutte d'eau : sinus dont la hauteur monte vite, puis un souffle de résonance."""
    d = 0.42
    t = t_(d)
    f = 520 + 900 * (1 - np.exp(-t / 0.03))  # monte de 520 à ~1400 Hz
    ph = 2 * np.pi * np.cumsum(f) / SR
    x = np.sin(ph) * env(len(t), 0.002, 0.07)
    x += 0.18 * np.sin(2 * ph) * env(len(t), 0.002, 0.04)
    x += 0.10 * np.sin(ph * 0.5) * env(len(t), 0.004, 0.12)
    return finish(x)


def souffle():
    """Un souffle d'air qui se referme (clin d'œil à Brise) et une note basse très douce."""
    d = 0.38
    n = int(SR * d)
    bruit = rng.standard_normal(n)
    air = lowpass_sweep(bruit, 5200, 900)
    air -= lowpass(air, 300)
    t = t_(d)
    e = np.sin(np.pi * np.minimum(1, t / 0.16)) ** 2 * np.exp(-np.maximum(0, t - 0.08) / 0.09)
    e[: int(0.01 * SR)] *= np.linspace(0, 1, int(0.01 * SR))
    x = air * e * 1.0
    note = np.sin(2 * np.pi * 392 * t) * env(n, 0.012, 0.11) * 0.18
    note += np.sin(2 * np.pi * 588 * t) * env(n, 0.012, 0.07) * 0.06
    return finish(x / (np.max(np.abs(x)) + 1e-9) * 0.7 + note, 0.5)


def carillon():
    """Deux notes de cloche douce (sol 5 puis ré 6), synthèse FM à faible indice."""
    d = 0.75
    buf = np.zeros(int(SR * d))
    for at, fr, g in ((0.0, 784.0, 1.0), (0.085, 1174.7, 0.8)):
        t = t_(0.65)
        mod = np.sin(2 * np.pi * fr * 3.5 * t) * 1.6 * np.exp(-t / 0.05)
        x = np.sin(2 * np.pi * fr * t + mod) * env(len(t), 0.002, 0.16)
        x += 0.25 * np.sin(2 * np.pi * fr * 2.01 * t) * env(len(t), 0.002, 0.06)
        place(buf, x * g, at)
    return finish(buf)


def feutre():
    """Un obturateur feutré : deux petits chocs mats, comme du tissu sur du bois."""
    d = 0.30
    buf = np.zeros(int(SR * d))
    for at, f0, g in ((0.0, 210.0, 1.0), (0.062, 150.0, 0.75)):
        n = int(0.16 * SR)
        t = np.arange(n) / SR
        corps = np.sin(2 * np.pi * (f0 + 90 * np.exp(-t / 0.012)) * t) * env(n, 0.0008, 0.035)
        tap = lowpass(rng.standard_normal(n), 2600) * env(n, 0.0004, 0.006) * 0.9
        tap -= lowpass(tap, 400)
        place(buf, (corps * 0.9 + tap) * g, at)
    return finish(buf, 0.55)


SONS = {"goutte": goutte, "souffle": souffle, "carillon": carillon, "feutre": feutre}


def ecrire(nom, x, dossier):
    pcm = (np.clip(x, -1, 1) * 32767).astype("<i2")
    with wave.open(f"{dossier}/{nom}.wav", "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())


if __name__ == "__main__":
    out = sys.argv[1]
    for k, f in SONS.items():
        ecrire(k, f(), out)
        print(k, "ok")
