/// Interruption periodique du systeme, pour eprouver une hypothese.
///
/// Le firmware tient son calendrier en logiciel : rien n'avance a la seconde
/// dans sa memoire tant qu'une source de temps ne le reveille pas. Cet outil
/// leve une interruption choisie a une cadence choisie, de quoi verifier laquelle
/// fait repartir l'horloge sans coder un peripherique entier.
///
/// Il est inerte tant que SONIX_TIC n'est pas defini, sous la forme
/// `numero:periode_en_cycles`.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TicSysteme {
    pub irq: u32,
    pub periode: u64,
    cycles: u64,
}

impl Default for TicSysteme {
    fn default() -> Self {
        let (irq, periode) = std::env::var("SONIX_TIC")
            .ok()
            .and_then(|v| {
                let (a, b) = v.split_once(':')?;
                Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
            })
            .unwrap_or((0, 0));
        Self { irq, periode, cycles: 0 }
    }
}

impl TicSysteme {
    /// Rend le numero d'interruption a lever, quand la periode est ecoulee.
    pub fn tick(&mut self, cycles: u32) -> Option<u32> {
        if self.periode == 0 {
            return None;
        }
        self.cycles += cycles as u64;
        if self.cycles < self.periode {
            return None;
        }
        self.cycles -= self.periode;
        Some(self.irq)
    }
}
