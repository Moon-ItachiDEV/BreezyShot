//! L'image en cours d'assemblage (contrat 0.6, §13.2, D11 et D13).
//!
//! Assemblage **par placement** : chaque image gardée est posée à sa position cumulée
//! (somme des avances) et ses lignes réécrivent celles déjà posées, sauf les lignes du
//! haut qui ne suivent pas l'avance (en-tête fixe, connu ou qui apparaît, et l'ombre
//! sous lui) : l'en-tête n'est donc posé qu'une fois, par la première image. Un pied,
//! connu ou non, ou une bannière apparue en route, est recouvert par l'image suivante et
//! ne reste qu'en bas, celui de la dernière image gardée. L'image est gardée en bandes de
//! `LIGNES_PAR_BANDE` lignes réservées entières une fois chacune (jamais déplacées,
//! remplies à mesure, réécrites en place), avec une copie de la dernière image gardée
//! (`derniere`). `finir` fait la seule copie de toute l'image.

use super::{Ajout, Fixes, Limite, Mesure, Paire, Vue, TOLERANCE};
use crate::systeme::gel::Pixels;

/// Lignes par bande.
const LIGNES_PAR_BANDE: usize = 1024;

/// L'image en cours d'assemblage : bandes de 1 024 lignes, jamais une réallocation de
/// toute l'image ; une seule copie à `finir`.
#[derive(Debug)]
pub struct Assemblage {
    largeur: u32,
    /// Hauteur des images de la zone.
    zone: u32,
    /// Hauteur de l'image assemblée (dernière image gardée comprise, pied compris).
    hauteur: u32,
    /// Position, dans l'image assemblée, du haut de la dernière image gardée.
    position: u32,
    /// Les lignes, par bandes de `LIGNES_PAR_BANDE` ; chaque bande a sa taille entière
    /// dès sa création.
    bandes: Vec<Vec<u8>>,
    fixes: Fixes,
    /// Les bandes sont connues (première `Avance` vue).
    fixes_connues: bool,
    /// Hauteur finale maximale (D13).
    hauteur_max: u32,
    /// La dernière image gardée.
    precedente: Pixels,
    /// La limite est atteinte (ou la mémoire refusée) : plus rien n'entre.
    plein: bool,
    memoire_refusee: bool,
}

impl Assemblage {
    /// Commence avec la première image. Erreur : image vide ou incohérente, plus grande que
    /// la limite, ou mémoire refusée.
    pub fn nouveau(premiere: &Pixels, limite: Limite) -> Result<Assemblage, String> {
        let v = Vue::de(premiere).ok_or("assemblage: image vide ou incoherente")?;
        let hauteur_max = u64::from(limite.hauteur).min(limite.pixels / v.l as u64);
        if (v.h as u64) > hauteur_max {
            return Err(format!(
                "assemblage: zone {}x{} hors de la limite",
                v.l, v.h
            ));
        }
        let mut copie = Vec::new();
        copie
            .try_reserve_exact(premiere.rgba.len())
            .map_err(|_| "assemblage: memoire refusee")?;
        copie.extend_from_slice(&premiere.rgba);
        let mut a = Assemblage {
            largeur: premiere.largeur,
            zone: premiere.hauteur,
            hauteur: 0,
            position: 0,
            bandes: Vec::new(),
            fixes: Fixes::default(),
            fixes_connues: false,
            hauteur_max: hauteur_max as u32,
            precedente: Pixels {
                largeur: premiere.largeur,
                hauteur: premiere.hauteur,
                rgba: copie,
            },
            plein: false,
            memoire_refusee: false,
        };
        if !a.assurer(v.h) {
            return Err("assemblage: memoire refusee".into());
        }
        a.ecrire(&premiere.rgba, 0, v.h, 0);
        a.hauteur = premiere.hauteur;
        Ok(a)
    }

    /// Pose `image` selon `mesure` (faite contre la dernière image gardée). Seule `Avance`
    /// pose ; l'image devient alors la dernière gardée. Elle est posée `d` lignes plus bas
    /// que la précédente et réécrit tout sous ses lignes du haut qui ne suivent pas
    /// l'avance (D12 : les lignes viennent de l'image la plus récente). À la première
    /// `Avance`, les bandes fixes sont reconnues (D11), ensuite elles ne font que rétrécir.
    /// La limite (D13) coupe le dernier ajout en gardant le pied connu en bas, et tout ce
    /// qui suit rend `Limite(0)` ; une mémoire refusée finit de même (`memoire_refusee`).
    pub fn ajouter(&mut self, image: &Pixels, mesure: &Mesure) -> Ajout {
        if self.plein {
            return Ajout::Limite(0);
        }
        let Some(v) = Vue::de(image) else {
            return Ajout::Rien;
        };
        if v.l != self.largeur as usize || v.h != self.zone as usize {
            return Ajout::Rien;
        }
        let d = match *mesure {
            Mesure::Avance(d) if d > 0 && d < self.zone => d,
            _ => return Ajout::Rien,
        };
        let h = self.zone;
        let Some(paire) = Paire::de(&self.precedente, image, &TOLERANCE) else {
            return Ajout::Rien;
        };
        self.fixes = if self.fixes_connues {
            paire.retrecir(d, self.fixes)
        } else {
            self.fixes_connues = true;
            paire.fixes(d)
        };
        let pied = self.fixes.bas.min(h - d);
        // Les lignes du haut à ne pas poser, sans laisser de trou : la précédente couvre
        // ses lignes `d..h`, son pied doit être recouvert.
        let garde = paire
            .haut_a_garder(d)
            .max(self.fixes.haut)
            .min(h - d - pied);
        let position = self.position + d;
        let voulue = position + h;
        let ancienne = self.hauteur;
        let (finale, coupee) = if voulue > self.hauteur_max {
            (self.hauteur_max, true)
        } else {
            (voulue, false)
        };
        if !self.assurer(finale as usize) {
            self.plein = true;
            return Ajout::Limite(0);
        }
        let (h, garde, position, pied) =
            (h as usize, garde as usize, position as usize, pied as usize);
        if coupee {
            // Le corps jusqu'à la place du pied, puis le pied de cette image.
            let fin_corps = (finale as usize).saturating_sub(pied);
            if fin_corps > position + garde {
                self.ecrire(
                    &image.rgba,
                    garde,
                    fin_corps - position - garde,
                    position + garde,
                );
            }
            let p = pied.min(finale as usize);
            self.ecrire(&image.rgba, h - p, p, finale as usize - p);
        } else {
            self.ecrire(&image.rgba, garde, h - garde, position + garde);
        }
        self.hauteur = finale;
        self.position = position as u32;
        self.precedente.rgba.copy_from_slice(&image.rgba);
        let ajoutees = finale.saturating_sub(ancienne);
        if coupee {
            self.plein = true;
            return Ajout::Limite(ajoutees);
        }
        Ajout::Ajoutees(ajoutees)
    }

    /// La dernière image gardée (celle contre laquelle mesurer la suivante).
    pub fn derniere(&self) -> &Pixels {
        &self.precedente
    }

    /// Les bandes fixes reconnues.
    pub fn fixes(&self) -> Fixes {
        self.fixes
    }

    /// La hauteur de l'image si elle finissait maintenant (pied compris), en pixels.
    pub fn hauteur(&self) -> u32 {
        self.hauteur
    }

    /// Une mémoire refusée a arrêté l'assemblage (D19 : `defilante impossible: memoire`).
    pub fn memoire_refusee(&self) -> bool {
        self.memoire_refusee
    }

    /// L'aperçu en direct : le bas de l'image assemblée (pied de la dernière image gardée
    /// compris), réduit à `largeur` au plus (jamais agrandi) et au plus `hauteur_max`
    /// lignes. Chaque pixel est la moyenne de 3 × 3 points répartis dans la case qu'il
    /// couvre (budget : 10 ms par envoi à 2 560 de large).
    pub fn apercu(&self, largeur: u32, hauteur_max: u32) -> Pixels {
        const PRIS: usize = 3;
        let total = self.hauteur as usize;
        let l = self.largeur as usize;
        if largeur == 0 || hauteur_max == 0 || total == 0 {
            return Pixels {
                largeur: 0,
                hauteur: 0,
                rgba: Vec::new(),
            };
        }
        let nl = (largeur as usize).min(l);
        let nh = ((total * nl) / l).clamp(1, hauteur_max as usize);
        let source = ((nh * l).div_ceil(nl)).min(total);
        let depart = total - source;
        // `PRIS` points répartis dans la case de chaque pixel d'arrivée, sur chaque axe (une
        // case plus étroite reprend ses points : ils comptent alors plusieurs fois).
        let places = |i: usize, n: usize, total: usize| -> [usize; PRIS] {
            let (a, b) = (i * total / n, ((i + 1) * total / n).max(i * total / n + 1));
            std::array::from_fn(|k| (a + (2 * k + 1) * (b - a) / (2 * PRIS)).min(total - 1))
        };
        let colonnes: Vec<[usize; PRIS]> =
            (0..nl).map(|c| places(c, nl, l).map(|x| x * 4)).collect();
        let mut rgba = Vec::new();
        if rgba.try_reserve_exact(nl * nh * 4).is_err() {
            return Pixels {
                largeur: 0,
                hauteur: 0,
                rgba,
            };
        }
        rgba.resize(nl * nh * 4, 0);
        for (y, sortie) in rgba.chunks_exact_mut(nl * 4).enumerate() {
            let lignes = places(y, nh, source).map(|sy| self.rangee(depart + sy));
            for (px, xs) in sortie.chunks_exact_mut(4).zip(&colonnes) {
                let mut s = [0u32; 4];
                for ligne in &lignes {
                    for &x in xs {
                        let p = &ligne[x..x + 4];
                        for k in 0..4 {
                            s[k] += u32::from(p[k]);
                        }
                    }
                }
                for k in 0..4 {
                    px[k] = ((s[k] + (PRIS * PRIS / 2) as u32) / (PRIS * PRIS) as u32) as u8;
                }
            }
        }
        Pixels {
            largeur: nl as u32,
            hauteur: nh as u32,
            rgba,
        }
    }

    /// L'image finale, une seule copie (les bandes sont rendues à mesure). Son bas est le
    /// pied de la dernière image gardée, déjà posé : `derniere` ne sert qu'à vérifier la
    /// taille (une image plus récente, qui a pu défiler, ne change rien). Erreur :
    /// `derniere` d'une autre taille ou mémoire refusée.
    pub fn finir(self, derniere: &Pixels) -> Result<Pixels, String> {
        let v = Vue::de(derniere).ok_or("assemblage: derniere image incoherente")?;
        if v.l != self.largeur as usize || v.h != self.zone as usize {
            return Err("assemblage: derniere image d'une autre taille".into());
        }
        let rangee = self.largeur as usize * 4;
        let total = self.hauteur as usize;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(total * rangee)
            .map_err(|_| "assemblage: memoire refusee")?;
        let mut reste = total;
        for bande in self.bandes {
            let n = reste.min(LIGNES_PAR_BANDE);
            rgba.extend_from_slice(&bande[..n * rangee]);
            reste -= n;
            if reste == 0 {
                break;
            }
        }
        Ok(Pixels {
            largeur: self.largeur,
            hauteur: total as u32,
            rgba,
        })
    }

    /// Réserve les bandes jusqu'à `lignes` lignes ; faux : mémoire refusée (l'assemblage
    /// est plein).
    fn assurer(&mut self, lignes: usize) -> bool {
        let taille = LIGNES_PAR_BANDE * self.largeur as usize * 4;
        while self.bandes.len() * LIGNES_PAR_BANDE < lignes {
            let mut bande = Vec::new();
            if bande.try_reserve_exact(taille).is_err() {
                self.memoire_refusee = true;
                self.plein = true;
                return false;
            }
            // Réservée entière, remplie à mesure (jamais au-delà : jamais déplacée).
            self.bandes.push(bande);
        }
        true
    }

    /// Écrit les lignes `y0..y0 + n` de `rgba` (une image de la zone) à partir de la ligne
    /// `vers` de l'image assemblée (bandes déjà réservées).
    /// Les lignes déjà posées sont réécrites en place, les suivantes ajoutées au bout de
    /// leur bande (dans sa réserve).
    fn ecrire(&mut self, rgba: &[u8], y0: usize, n: usize, vers: usize) {
        let rangee = self.largeur as usize * 4;
        let mut fait = 0;
        while fait < n {
            let r = vers + fait;
            let (b, i) = (r / LIGNES_PAR_BANDE, r % LIGNES_PAR_BANDE);
            let k = (LIGNES_PAR_BANDE - i).min(n - fait);
            let src = &rgba[(y0 + fait) * rangee..(y0 + fait + k) * rangee];
            let bande = &mut self.bandes[b];
            // Jamais de trou : une ligne n'est posée qu'au plus juste après la dernière.
            if bande.len() < i * rangee {
                bande.resize(i * rangee, 0);
            }
            let deja = (bande.len() - i * rangee).min(src.len());
            bande[i * rangee..i * rangee + deja].copy_from_slice(&src[..deja]);
            bande.extend_from_slice(&src[deja..]);
            fait += k;
        }
    }

    /// La ligne `r` de l'image assemblée.
    fn rangee(&self, r: usize) -> &[u8] {
        let rangee = self.largeur as usize * 4;
        let (b, i) = (r / LIGNES_PAR_BANDE, r % LIGNES_PAR_BANDE);
        &self.bandes[b][i * rangee..(i + 1) * rangee]
    }

    /// Adresse de chaque bande (tests : jamais déplacée par un ajout).
    #[cfg(test)]
    pub(super) fn adresses(&self) -> Vec<(*const u8, usize)> {
        self.bandes
            .iter()
            .map(|b| (b.as_ptr(), b.capacity()))
            .collect()
    }
}
