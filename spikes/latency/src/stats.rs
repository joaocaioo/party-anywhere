//! Estatística pura do spike: amostras com percentis e a estimativa da diferença entre relógios.

use std::collections::VecDeque;

use serde::Serialize;

/// Acima disto, para de guardar (uma sessão longa ainda cabe com folga).
const MAX_SAMPLES: usize = 200_000;
/// Quantas trocas SYNC recentes valem para escolher o offset.
const CLOCK_WINDOW: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Summary {
    pub n: usize,
    pub min: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
    pub mean: f64,
}

#[derive(Debug, Default, Clone)]
pub struct Samples {
    v: Vec<f64>,
}

impl Samples {
    pub fn push(&mut self, x: f64) {
        if self.v.len() < MAX_SAMPLES {
            self.v.push(x);
        }
    }

    pub fn len(&self) -> usize {
        self.v.len()
    }

    /// Percentis pelo método do posto mais próximo. `None` sem amostras.
    pub fn summary(&self) -> Option<Summary> {
        if self.v.is_empty() {
            return None;
        }
        let mut s = self.v.clone();
        s.sort_by(f64::total_cmp);
        let rank = |p: f64| s[((p / 100.0 * s.len() as f64).ceil() as usize).clamp(1, s.len()) - 1];
        Some(Summary {
            n: s.len(),
            min: s[0],
            p50: rank(50.0),
            p95: rank(95.0),
            p99: rank(99.0),
            max: s[s.len() - 1],
            mean: s.iter().sum::<f64>() / s.len() as f64,
        })
    }
}

/// Diferença entre o relógio do servidor e o do celular (servidor ≈ celular + offset), como no NTP:
/// das trocas recentes, vale a de menor ida e volta, que é a de menor erro possível (≤ rtt / 2).
#[derive(Debug, Default)]
pub struct ClockSync {
    recent: VecDeque<(f64, f64)>,
}

impl ClockSync {
    pub fn add(&mut self, rtt_ms: f64, offset_ms: f64) {
        if self.recent.len() == CLOCK_WINDOW {
            self.recent.pop_front();
        }
        self.recent.push_back((rtt_ms, offset_ms));
    }

    fn best(&self) -> Option<(f64, f64)> {
        self.recent.iter().copied().min_by(|a, b| a.0.total_cmp(&b.0))
    }

    pub fn offset(&self) -> Option<f64> {
        self.best().map(|b| b.1)
    }

    /// Ida e volta da troca usada: o erro da medição é no máximo metade disto.
    pub fn best_rtt(&self) -> Option<f64> {
        self.best().map(|b| b.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_nearest_rank() {
        let mut s = Samples::default();
        for i in 1..=100 {
            s.push(i as f64);
        }
        let r = s.summary().unwrap();
        assert_eq!((r.n, r.min, r.p50, r.p95, r.p99, r.max), (100, 1.0, 50.0, 95.0, 99.0, 100.0));
        assert_eq!(r.mean, 50.5);
    }

    #[test]
    fn single_sample_and_empty() {
        assert!(Samples::default().summary().is_none());
        let mut s = Samples::default();
        s.push(7.0);
        let r = s.summary().unwrap();
        assert_eq!((r.p50, r.p99), (7.0, 7.0));
    }

    #[test]
    fn clock_uses_shortest_round_trip() {
        let mut c = ClockSync::default();
        assert_eq!(c.offset(), None);
        c.add(20.0, 1000.0);
        c.add(4.0, 990.0);
        c.add(9.0, 1003.0);
        assert_eq!((c.offset(), c.best_rtt()), (Some(990.0), Some(4.0)));
    }

    #[test]
    fn clock_window_forgets_old_exchanges() {
        let mut c = ClockSync::default();
        c.add(1.0, 5.0);
        for _ in 0..CLOCK_WINDOW {
            c.add(10.0, 7.0);
        }
        assert_eq!(c.offset(), Some(7.0));
    }
}
