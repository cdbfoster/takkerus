use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use tracing::info;

use tak::{Color, State};

use crate::analysis::AnalysisConfig;

#[derive(Clone, Copy)]
pub enum TimeControl {
    Simple {
        depth_limit: Option<u32>,
        time_limit: Option<Duration>,
        /// If this is set and the next search depth is predicted to take
        /// longer than the time limit, stop the search early.
        early_stop: bool,
    },
    Clock {
        time: Duration,
        increment: Duration,
        time_limit: Option<Duration>,
        early_stop: bool,
    },
}

impl TimeControl {
    pub(crate) fn set_interrupt<const N: usize>(
        &self,
        config: &AnalysisConfig<N>,
        state: &State<N>,
    ) -> (Option<Duration>, InterruptHandle) {
        let time_limit = match self {
            TimeControl::Simple { time_limit, .. } => *time_limit,
            TimeControl::Clock {
                time,
                increment,
                time_limit,
                ..
            } => {
                let reserves = match state.to_move() {
                    Color::White => state.p1_flatstones as f32,
                    Color::Black => state.p2_flatstones as f32,
                };

                // XXX Come up with a better reserves -> moves estimation function than this.
                let moves_remaining = 3.0 * reserves / 2.0;

                let mut use_time = Duration::from_secs_f32(
                    time.as_secs_f32() / moves_remaining + 4.0 * increment.as_secs_f32() / 5.0,
                );

                // Never use more than half of the remaining time.
                use_time = use_time.min(*time / 2);

                if let Some(maximum) = time_limit {
                    use_time = use_time.min(*maximum);
                }

                Some(use_time)
            }
        };

        (
            time_limit,
            InterruptHandle(
                time_limit.map(|time_limit| spawn_interrupt_thread(config, time_limit)),
            ),
        )
    }

    pub fn maximum_time(&self) -> Option<Duration> {
        match self {
            TimeControl::Simple { time_limit, .. } => *time_limit,
            TimeControl::Clock { time_limit, .. } => *time_limit,
        }
    }

    pub fn early_stop(&self) -> bool {
        match self {
            TimeControl::Simple { early_stop, .. } => *early_stop,
            TimeControl::Clock { early_stop, .. } => *early_stop,
        }
    }
}

impl Default for TimeControl {
    fn default() -> Self {
        Self::Simple {
            depth_limit: Default::default(),
            time_limit: Default::default(),
            early_stop: Default::default(),
        }
    }
}

impl fmt::Display for TimeControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeControl::Simple {
                depth_limit,
                time_limit,
                early_stop,
            } => {
                write!(
                    f,
                    "depth_limit: {}, time_limit: {}, early_stop: {:?}",
                    if let Some(depth_limit) = depth_limit {
                        depth_limit.to_string()
                    } else {
                        "none".to_owned()
                    },
                    if let Some(time_limit) = time_limit {
                        format!("{:.2}s", time_limit.as_secs_f32())
                    } else {
                        "none".to_owned()
                    },
                    early_stop,
                )?;
            }
            TimeControl::Clock {
                time,
                increment,
                time_limit: maximum,
                early_stop,
            } => {
                let minutes = time.as_secs() / 60;
                let seconds = time.as_secs_f32() - (60 * minutes) as f32;

                write!(
                    f,
                    "clock: {minutes:02}:{seconds:04.2}, inc: +{:.2}s",
                    increment.as_secs_f32(),
                )?;

                if let Some(maximum) = maximum {
                    write!(f, ", maximum: {:.2}s", maximum.as_secs_f32())?;
                }

                write!(f, ", early_stop: {early_stop:?}")?;
            }
        }

        Ok(())
    }
}

fn spawn_interrupt_thread<const N: usize>(
    config: &AnalysisConfig<N>,
    time_limit: Duration,
) -> Arc<AtomicBool> {
    let cancel = Arc::new(AtomicBool::new(false));
    let start_time = Instant::now();

    {
        let cancel = cancel.clone();
        let interrupted = config.interrupted.clone();
        thread::spawn(move || loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }

            let remaining_time = time_limit.saturating_sub(start_time.elapsed());
            if !remaining_time.is_zero() {
                // Check for cancels at least every 10th of a second.
                let sleep_time = remaining_time.div_f64(2.0).min(Duration::from_millis(100));
                thread::sleep(sleep_time);
            } else {
                info!("Time limit reached. Stopping.");
                interrupted.store(true, Ordering::Relaxed);
                break;
            }
        });
    }

    cancel
}

pub(crate) struct InterruptHandle(Option<Arc<AtomicBool>>);

impl InterruptHandle {
    pub fn cancel(&self) {
        if let Some(cancel) = &self.0 {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}
