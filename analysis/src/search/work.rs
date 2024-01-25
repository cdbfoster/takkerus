use std::iter::Enumerate;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use crate::evaluation::Evaluation;
use crate::plies::PlyGenerator;
use crate::util::bag::Bag;

use super::{BranchResult, Node};

#[derive(Default)]
pub(super) struct WorkNodes<const N: usize> {
    nodes: Mutex<Bag<WorkNode<N>>>,
    pub queue: Condvar,
    pub shutdown: AtomicBool,
}

impl<const N: usize> WorkNodes<N> {
    #[allow(dead_code)]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Mutex::new(Bag::with_capacity(capacity)),
            queue: Condvar::new(),
            shutdown: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, Bag<WorkNode<N>>> {
        self.nodes.lock().unwrap()
    }
}

pub(super) struct WorkNode<const N: usize> {
    pub ply_generator: Enumerate<PlyGenerator<N>>,
    pub node: Node<N>,
    pub variables: WorkNodeVariables,
    pub results: Arc<WorkNodeResults<N>>,
}

#[derive(Clone, Copy)]
pub(super) struct WorkNodeVariables {
    pub status: WorkNodeStatus,
    pub alpha: Evaluation,
    pub beta: Evaluation,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum WorkNodeStatus {
    Wait,
    Active,
    Pruned,
}

#[derive(Default)]
pub(super) struct WorkNodeResults<const N: usize> {
    buffer: Mutex<WorkNodeResultsInner<N>>,
    pub queue: Condvar,
}

impl<const N: usize> WorkNodeResults<N> {
    pub fn lock(&self) -> MutexGuard<'_, WorkNodeResultsInner<N>> {
        self.buffer.lock().unwrap()
    }
}

pub(super) struct WorkNodeResultsInner<const N: usize> {
    buffer: Vec<(usize, BranchResult<N>)>,
    /// The number of workers actively working on this result set.
    workers: usize,
}

impl<const N: usize> Default for WorkNodeResultsInner<N> {
    fn default() -> Self {
        Self {
            buffer: Vec::new(),
            workers: 1,
        }
    }
}

impl<const N: usize> WorkNodeResultsInner<N> {
    pub fn workers(&self) -> usize {
        self.workers
    }

    pub fn inc_workers(&mut self) {
        self.workers += 1;
    }

    pub fn dec_workers(&mut self) {
        assert!(self.workers >= 1, "cannot decrease workers below zero");
        self.workers -= 1;
    }
}

impl<const N: usize> Deref for WorkNodeResultsInner<N> {
    type Target = Vec<(usize, BranchResult<N>)>;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<const N: usize> DerefMut for WorkNodeResultsInner<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}
