use std::ops::{Add, AddAssign};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Debug, Default)]
pub struct Statistics {
    pub visited: u64,
    pub evaluated: u64,
    pub terminal: u64,
    pub scouted: u64,
    pub re_searched: u64,
    pub beta_cutoff: u64,
    pub null_cutoff: u64,
    pub tt_stores: u64,
    pub tt_store_fails: u64,
    pub tt_hits: u64,
    pub tt_saves: u64,
    /// Best-ply ordering of PV-Nodes (exact bound nodes).
    pub pv_ply_order: [u64; 6],
    /// Best-ply ordering of All-Nodes (fail-low nodes).
    pub all_ply_order: [u64; 6],
}

impl Add for &Statistics {
    type Output = Statistics;

    fn add(self, other: Self) -> Self::Output {
        let mut pv_ply_order = self.pv_ply_order;
        for (a, b) in pv_ply_order.iter_mut().zip(other.pv_ply_order) {
            *a += b;
        }

        let mut all_ply_order = self.all_ply_order;
        for (a, b) in all_ply_order.iter_mut().zip(other.all_ply_order) {
            *a += b;
        }

        Self::Output {
            visited: self.visited + other.visited,
            evaluated: self.evaluated + other.evaluated,
            terminal: self.terminal + other.terminal,
            scouted: self.scouted + other.scouted,
            re_searched: self.re_searched + other.re_searched,
            beta_cutoff: self.beta_cutoff + other.beta_cutoff,
            null_cutoff: self.null_cutoff + other.null_cutoff,
            tt_stores: self.tt_stores + other.tt_stores,
            tt_store_fails: self.tt_store_fails + other.tt_store_fails,
            tt_hits: self.tt_hits + other.tt_hits,
            tt_saves: self.tt_saves + other.tt_saves,
            pv_ply_order,
            all_ply_order,
        }
    }
}

impl AddAssign<&Statistics> for Statistics {
    fn add_assign(&mut self, other: &Self) {
        *self = self.add(other)
    }
}

#[derive(Debug, Default)]
pub(crate) struct AtomicStatistics {
    pub visited: AtomicU64,
    pub evaluated: AtomicU64,
    pub terminal: AtomicU64,
    pub scouted: AtomicU64,
    pub re_searched: AtomicU64,
    pub beta_cutoff: AtomicU64,
    pub null_cutoff: AtomicU64,
    pub tt_stores: AtomicU64,
    pub tt_store_fails: AtomicU64,
    pub tt_hits: AtomicU64,
    pub tt_saves: AtomicU64,
    /// Best-ply ordering of PV-Nodes (exact bound nodes).
    pub pv_ply_order: [AtomicU64; 6],
    /// Best-ply ordering of All-Nodes (fail-low nodes).
    pub all_ply_order: [AtomicU64; 6],
}

impl AtomicStatistics {
    pub fn load(&self) -> Statistics {
        fn load_ply_order(values: &[AtomicU64; 6]) -> [u64; 6] {
            let mut buffer = [0; 6];
            for (a, b) in buffer.iter_mut().zip(values) {
                *a = b.load(Ordering::Relaxed);
            }
            buffer
        }

        Statistics {
            visited: self.visited.load(Ordering::Relaxed),
            evaluated: self.evaluated.load(Ordering::Relaxed),
            terminal: self.terminal.load(Ordering::Relaxed),
            scouted: self.scouted.load(Ordering::Relaxed),
            re_searched: self.re_searched.load(Ordering::Relaxed),
            beta_cutoff: self.beta_cutoff.load(Ordering::Relaxed),
            null_cutoff: self.null_cutoff.load(Ordering::Relaxed),
            tt_stores: self.tt_stores.load(Ordering::Relaxed),
            tt_store_fails: self.tt_store_fails.load(Ordering::Relaxed),
            tt_hits: self.tt_hits.load(Ordering::Relaxed),
            tt_saves: self.tt_saves.load(Ordering::Relaxed),
            pv_ply_order: load_ply_order(&self.pv_ply_order),
            all_ply_order: load_ply_order(&self.all_ply_order),
        }
    }
}
