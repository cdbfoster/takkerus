use tak::State;

pub use self::types::Evaluation;

mod features;
mod model;
mod types;

use model::{evaluate_model, Model};

pub fn evaluate<const N: usize>(state: &State<N>) -> Evaluation {
    macro_rules! evaluate_sized {
        ($n:expr) => {{
            let state = downcast_size::<N, $n>(state);
            evaluate_model(state, <State<$n> as Model>::static_model())
        }};
    }

    match N {
        3 => evaluate_sized!(3),
        4 => evaluate_sized!(4),
        5 => evaluate_sized!(5),
        6 => evaluate_sized!(6),
        7 => evaluate_sized!(7),
        8 => evaluate_sized!(8),
        _ => unreachable!(),
    }
}

fn downcast_size<const N: usize, const M: usize>(state: &State<N>) -> &State<M> {
    debug_assert_eq!(N, M);
    unsafe { std::mem::transmute(state) }
}
