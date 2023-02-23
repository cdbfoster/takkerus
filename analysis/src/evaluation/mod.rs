use tak::{Resolution, State};

pub use self::features::GatherFeatures;
pub use self::model::{Evaluator, Model};
pub use self::types::Evaluation;

mod features;
mod model;
mod types;

pub fn evaluate<const N: usize>(state: &State<N>) -> Evaluation {
    match state.resolution() {
        None => (),
        Some(Resolution::Road(color)) | Some(Resolution::Flats { color, .. }) => {
            if color == state.to_move() {
                return Evaluation::WIN - state.ply_count as i32;
            } else {
                return Evaluation::LOSE + state.ply_count as i32;
            }
        }
        Some(Resolution::Draw) => return Evaluation::ZERO - state.ply_count as i32,
    }

    macro_rules! evaluate_sized {
        ($n:expr) => {{
            let state = downcast_size::<N, $n>(state);
            Model::<$n>::evaluate_model(Model::<$n>::static_model(), state)
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
