use std::fmt;
use std::time::Duration;

use tak::{Color, State};

#[derive(Clone, Copy, Debug, Default)]
pub struct TimeControl {
    pub time: Duration,
    pub increment: Duration,
}

impl TimeControl {
    pub(crate) fn get_use_time<const N: usize>(&self, state: &State<N>) -> Duration {
        let reserves = match state.to_move() {
            Color::White => (state.p1_flatstones + state.p1_capstones) as f32,
            Color::Black => (state.p2_flatstones + state.p2_capstones) as f32,
        };

        // XXX Come up with a better reserves -> moves estimation function than this.
        let moves_remaining = 3.0 * reserves / 2.0;

        let mut use_time = Duration::from_secs_f32(
            self.time.as_secs_f32() / moves_remaining + 4.0 * self.increment.as_secs_f32() / 5.0,
        );

        // Never use more than half of the remaining time.
        use_time = use_time.min(self.time / 2);

        use_time
    }
}

impl fmt::Display for TimeControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let minutes = self.time.as_secs() / 60;
        let seconds = self.time.as_secs_f32() - (60 * minutes) as f32;

        write!(
            f,
            "{minutes:02}:{seconds:04.2}, inc: +{:.2}s",
            self.increment.as_secs_f32(),
        )
    }
}
