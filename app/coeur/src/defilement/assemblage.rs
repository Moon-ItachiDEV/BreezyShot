//! L'image en cours d'assemblage (contrat 0.6, §13.2, D11 et D13).
//!
//! Ce qui est gardé : le « flux » (l'en-tête de la première image, puis les lignes de la
//! page, sans le pied), en bandes de `LIGNES_PAR_BANDE` lignes réservées une fois chacune
//! (jamais de réallocation de ce qui est déjà là), et une copie de la dernière image
//! gardée (pour les bandes fixes et le pied de l'aperçu). `finir` fait la seule copie de
//! toute l'image : le flux, puis le pied de la dernière image.

use super::{fixes, retrecir, Ajout, Fixes, Limite, Mesure, Vue, TOLERANCE};
use crate::systeme::gel::Pixels;

/// Lignes par bande du flux.
const LIGNES_PAR_BANDE: usize = 1024;

/// L'image en cours d'assemblage : bandes de 1 024 lignes, jamais une réallocation de
/// toute l'image ; une seule copie à `finir`.
#[derive(Debug)]
pub struct Assemblage {
    largeur: u32,
    /// Hauteur des images de la zone.
    zone: u32,
    /// Lignes du flux (en-tête et page, sans le pied).
    flux: u32,
    /// Le flux, par bandes de `LIGNES_PAR_BANDE` lignes ; chaque bande a sa capacité
    /// entière dès sa création.
    bandes: Vec<Vec<u8>>,
    fixes: Fixes,
    /// Les bandes sont connues (première `Avance` vue).
    fixes_connues: bool,
    /// Hauteur finale maximale (D13), pied compris.
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
            flux: 0,
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
        if a.pousser(&premiere.rgba, 0, v.h) < v.h {
            return Err("assemblage: memoire refusee".into());
        }
        Ok(a)
    }

    /// Ajoute les lignes nouvelles de `image` selon `mesure` (faite contre la dernière
    /// image gardée). Seule `Avance` ajoute ; l'image devient alors la dernière gardée. À
    /// la première `Avance`, les bandes fixes sont reconnues (D11) et le pied de la
    /// première image quitte le flux ; ensuite elles ne font que rétrécir. Les lignes
    /// nouvelles viennent toujours de `image` (D12). La limite (D13) coupe le dernier ajout
    /// et tout ce qui suit rend `Limite(0)` ; une mémoire refusée finit de même
    /// (`memoire_refusee`).
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
        let ancien_bas = if self.fixes_connues {
            let bas = self.fixes.bas;
            let r = retrecir(&self.precedente, image, d, self.fixes, &TOLERANCE);
            self.fixes = Fixes {
                haut: r.haut.min(self.fixes.haut),
                bas: r.bas.min(bas),
            };
            bas
        } else {
            self.fixes = fixes(&self.precedente, image, d, &TOLERANCE);
            self.fixes_connues = true;
            // Le pied de la première image n'est pas de la page : il quitte le flux et
            // reviendra une fois, en bas, à `finir`.
            self.tronquer(self.flux.saturating_sub(self.fixes.bas));
            self.fixes.bas
        };
        // Le flux s'arrête à la ligne `h - ancien_bas` de l'image d'avant, soit la ligne
        // `h - ancien_bas - d` de celle-ci.
        let debut = h.saturating_sub(ancien_bas + d);
        let fin = h - self.fixes.bas;
        let voulues = fin.saturating_sub(debut);
        let place = self
            .hauteur_max
            .saturating_sub(self.fixes.bas)
            .saturating_sub(self.flux);
        let prises = voulues.min(place);
        let poussees = self.pousser(&image.rgba, debut as usize, (debut + prises) as usize) as u32;
        self.precedente.rgba.copy_from_slice(&image.rgba);
        if poussees < voulues {
            self.plein = true;
            return Ajout::Limite(poussees);
        }
        Ajout::Ajoutees(poussees)
    }

    /// Les bandes fixes reconnues.
    pub fn fixes(&self) -> Fixes {
        self.fixes
    }

    /// La hauteur de l'image si elle finissait maintenant (pied compris), en pixels.
    pub fn hauteur(&self) -> u32 {
        self.flux + self.fixes.bas
    }

    /// Une mémoire refusée a arrêté l'assemblage (D19 : `defilante impossible: memoire`).
    pub fn memoire_refusee(&self) -> bool {
        self.memoire_refusee
    }

    /// L'aperçu en direct : le bas de l'image assemblée (pied de la dernière image gardée
    /// compris), réduit à `largeur` au plus (jamais agrandi) et au plus `hauteur_max`
    /// lignes. Chaque pixel est la moyenne des pixels qu'il couvre.
    pub fn apercu(&self, largeur: u32, hauteur_max: u32) -> Pixels {
        let total = self.hauteur() as usize;
        let l = self.largeur as usize;
        if largeur == 0 || hauteur_max == 0 || total == 0 {
            return Pixels {
                largeur: 0,
                hauteur: 0,
                rgba: Vec::new(),
            };
        }
        let nl = (largeur as usize).min(l);
        // Lignes d'arrivée : la hauteur réduite, bornée.
        let nh = ((total * nl) / l).clamp(1, hauteur_max as usize);
        // Lignes de départ prises en bas, pour garder la proportion.
        let source = ((nh * l).div_ceil(nl)).min(total);
        let depart = total - source;
        // Chaque colonne de départ va dans une colonne d'arrivée.
        let colonne: Vec<usize> = (0..l).map(|x| x * nl / l).collect();
        let mut rgba = vec![0u8; nl * nh * 4];
        let mut somme = vec![0u32; nl * 4];
        let mut nombre = vec![0u32; nl];
        for y in 0..nh {
            let (y0, y1) = (
                y * source / nh,
                ((y + 1) * source).div_ceil(nh).max(y * source / nh + 1),
            );
            somme.iter_mut().for_each(|s| *s = 0);
            nombre.iter_mut().for_each(|n| *n = 0);
            for sy in y0..y1.min(source) {
                let ligne = self.rangee(depart + sy);
                for (x, p) in ligne.chunks_exact(4).enumerate() {
                    let c = colonne[x];
                    for k in 0..4 {
                        somme[c * 4 + k] += u32::from(p[k]);
                    }
                    nombre[c] += 1;
                }
            }
            let sortie = &mut rgba[y * nl * 4..(y + 1) * nl * 4];
            for (c, n) in nombre.iter().enumerate() {
                let n = (*n).max(1);
                for k in 0..4 {
                    sortie[c * 4 + k] = ((somme[c * 4 + k] + n / 2) / n) as u8;
                }
            }
        }
        Pixels {
            largeur: nl as u32,
            hauteur: nh as u32,
            rgba,
        }
    }

    /// L'image finale : le flux, puis le pied de `derniere` (la dernière image gardée, ou
    /// une image prise après elle à la même taille : le pied ne défile pas). Une seule
    /// copie ; les bandes sont rendues à mesure. Erreur : `derniere` d'une autre taille ou
    /// mémoire refusée.
    pub fn finir(self, derniere: &Pixels) -> Result<Pixels, String> {
        let v = Vue::de(derniere).ok_or("assemblage: derniere image incoherente")?;
        if v.l != self.largeur as usize || v.h != self.zone as usize {
            return Err("assemblage: derniere image d'une autre taille".into());
        }
        let rangee = self.largeur as usize * 4;
        let total = self.hauteur() as usize;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(total * rangee)
            .map_err(|_| "assemblage: memoire refusee")?;
        for bande in self.bandes {
            rgba.extend_from_slice(&bande);
        }
        let h = self.zone as usize;
        let bas = self.fixes.bas as usize;
        rgba.extend_from_slice(&derniere.rgba[(h - bas) * rangee..h * rangee]);
        Ok(Pixels {
            largeur: self.largeur,
            hauteur: total as u32,
            rgba,
        })
    }

    /// Ajoute au flux les lignes `y0..y1` de `rgba` (une image de la zone). Rend le nombre
    /// de lignes ajoutées ; moins que demandé : mémoire refusée (l'assemblage est plein).
    fn pousser(&mut self, rgba: &[u8], y0: usize, y1: usize) -> usize {
        let rangee = self.largeur as usize * 4;
        let mut y = y0;
        while y < y1 {
            let numero = self.flux as usize / LIGNES_PAR_BANDE;
            if numero == self.bandes.len() {
                let mut bande = Vec::new();
                if bande.try_reserve_exact(LIGNES_PAR_BANDE * rangee).is_err() {
                    self.memoire_refusee = true;
                    self.plein = true;
                    break;
                }
                self.bandes.push(bande);
            }
            let libres = LIGNES_PAR_BANDE - self.flux as usize % LIGNES_PAR_BANDE;
            let n = libres.min(y1 - y);
            self.bandes[numero].extend_from_slice(&rgba[y * rangee..(y + n) * rangee]);
            self.flux += n as u32;
            y += n;
        }
        y - y0
    }

    /// Raccourcit le flux à `lignes` (le pied de la première image).
    fn tronquer(&mut self, lignes: u32) {
        let rangee = self.largeur as usize * 4;
        let lignes = lignes.min(self.flux) as usize;
        let garder = lignes.div_ceil(LIGNES_PAR_BANDE);
        self.bandes.truncate(garder.max(1).min(self.bandes.len()));
        if let Some(derniere) = self.bandes.last_mut() {
            let dans = lignes - (garder.max(1) - 1) * LIGNES_PAR_BANDE;
            derniere.truncate(dans * rangee);
        }
        self.flux = lignes as u32;
    }

    /// La ligne `r` de l'image si elle finissait maintenant.
    fn rangee(&self, r: usize) -> &[u8] {
        let rangee = self.largeur as usize * 4;
        if r < self.flux as usize {
            let (b, i) = (r / LIGNES_PAR_BANDE, r % LIGNES_PAR_BANDE);
            &self.bandes[b][i * rangee..(i + 1) * rangee]
        } else {
            let y = (self.zone - self.fixes.bas) as usize + (r - self.flux as usize);
            &self.precedente.rgba[y * rangee..(y + 1) * rangee]
        }
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
