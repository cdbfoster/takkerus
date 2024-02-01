use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::thread;
use std::time::Instant;

use rand::{self, seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};

use analysis::evaluation::{AnnEvaluator, AnnModel, Evaluator, GatherFeatures};
use analysis::{analyze, AnalysisConfig, PersistentState, TimeControl};
use ann::linear_algebra::MatrixRowMajor;
use ann::loss::{mse, mse_prime};
use ann::shallow::ShallowAdam;
use tak::{board_mask, generation, Color, PieceType, Ply, Resolution, State};

const BATCH_SIZE: usize = 128;
const BATCHES_PER_UPDATE: usize = 8;
const CHECKPOINT_BATCHES: usize = 1000;

// Capped at the number of batches per update.
const GATHER_THREADS: usize = 4;
/// The search depth to use when building the positions the training samples are derived from.
const SCAFFOLD_SEARCH_DEPTH: u32 = 3;
/// The number of consecutive samples to take from one starting position.
const SAMPLES_PER_POSITION: usize = 10;
/// The number of plies to play when calculating the temporal difference of the evaulations.
const TD_PLY_DEPTH: usize = 10;
/// The search depth to use when calculating the temporal difference of the evaluations.
const TD_SEARCH_DEPTH: u32 = 4;

const LEARNING_RATE_SCHEDULE: [(usize, f32); 3] = [(0, 0.001), (64, 0.0001), (75_000, 0.00001)];

const TRAINING_DIR: &'static str = "training";
const MODEL_DIR: &'static str = "models";
const CHECKPOINT_DIR: &'static str = "checkpoints";

fn main() {
    fs::create_dir_all(format!("{TRAINING_DIR}/{MODEL_DIR}"))
        .expect("could not create model directory");
    fs::create_dir_all(format!("{TRAINING_DIR}/{CHECKPOINT_DIR}"))
        .expect("could not create checkpoint directory");

    let mut args = env::args();

    let (size, checkpoint) = if let Some(argument) = args.nth(1) {
        if argument == "--size" {
            let size = args
                .next()
                .expect("must pass a size value")
                .parse::<usize>()
                .expect("invalid size");
            (size, None)
        } else {
            let file = File::open(argument).expect("could not read checkpoint file");
            let (size, checkpoint): (usize, String) =
                serde_json::from_reader(file).expect("could not parse checkpoint file");

            (size, Some(checkpoint))
        }
    } else {
        eprintln!("To train a new model, specify a board size N by passing '--size N'.");
        eprintln!("To continue from a previous checkpoint, pass the name of a checkpoint file.");
        return;
    };

    let max_batches = args.next().map(|s| {
        s.parse::<usize>()
            .expect("could not parse maximum number of batches")
    });

    match size {
        3 => main_sized::<3>(checkpoint, max_batches),
        4 => main_sized::<4>(checkpoint, max_batches),
        5 => main_sized::<5>(checkpoint, max_batches),
        6 => main_sized::<6>(checkpoint, max_batches),
        7 => main_sized::<7>(checkpoint, max_batches),
        8 => main_sized::<8>(checkpoint, max_batches),
        _ => panic!("invalid size"),
    }
}

fn main_sized<const N: usize>(checkpoint: Option<String>, max_batches: Option<usize>)
where
    TrainingState<N>: Train<N, State = State<N>>,
{
    let mut rng = rand::thread_rng();

    let mut training_state = if let Some(checkpoint) = checkpoint {
        serde_json::from_str(&checkpoint).expect("could not parse checkpoint")
    } else {
        TrainingState::<N>::new()
    };

    if max_batches == Some(0) {
        save_training_state(
            &training_state,
            format!("{TRAINING_DIR}/{MODEL_DIR}/latest.json"),
            format!("{TRAINING_DIR}/{CHECKPOINT_DIR}/latest.json"),
        );
        return;
    }

    let mut iteration = training_state.batch / BATCHES_PER_UPDATE;
    let mut checkpoint_error =
        training_state.error * (training_state.batch % CHECKPOINT_BATCHES) as f32;

    while max_batches.is_none() || training_state.batch < max_batches.unwrap() {
        iteration += 1;

        print!("Iteration {iteration}... ");
        std::io::stdout().flush().ok();

        let start_time = Instant::now();

        let scaffolds = build_scaffold_positions(&training_state);
        let mut batch_samples = generate_batch_samples(&training_state, scaffolds);
        batch_samples.shuffle(&mut rng);

        let start_batch = training_state.batch;
        let average_error = train_batches(
            &mut training_state,
            &batch_samples,
            max_batches,
            &mut checkpoint_error,
        );
        let batch_count = training_state.batch - start_batch;

        let elapsed = start_time.elapsed().as_secs_f32();
        println!(
            "b: {}, t: {:.2}s, avg b t: {:.2}s/b, lr: {}, avg b err: {:.3}, avg chkpt err: {:.3}",
            training_state.batch,
            elapsed,
            elapsed / batch_count as f32,
            training_state.learning_rate,
            average_error,
            training_state.error,
        );
    }
}

fn build_scaffold_positions<const N: usize>(training_state: &TrainingState<N>) -> Vec<State<N>>
where
    TrainingState<N>: Train<N, State = State<N>>,
{
    let mut rng = rand::thread_rng();

    let mut states = Vec::new();
    let mut persistent_state = PersistentState::default();
    let evaluator = training_state.model_as_evaluator();

    let mut state = State::default();

    while state.resolution().is_none() && state.ply_count < 300 {
        states.push(state.clone());

        // If the game has just started, or if a random number is below epsilon, make a random move.
        // Otherwise, use the principal variation from a search.
        if state.ply_count < 2 || rng.gen::<f32>() < training_state.epsilon {
            let ply = *generate_plies(&state).choose(&mut rng).unwrap();
            state.execute_ply(ply).expect("error executing random ply");
        } else {
            let config = AnalysisConfig::<N> {
                time_control: TimeControl::Simple {
                    depth_limit: Some(SCAFFOLD_SEARCH_DEPTH),
                    time_limit: None,
                    early_stop: false,
                },
                persistent_state: Some(&mut persistent_state),
                evaluator: Some(&*evaluator),
                exact_eval: true,
                ..Default::default()
            };

            let analysis = analyze(config, &state);

            state
                .execute_ply(
                    *analysis
                        .principal_variation
                        .first()
                        .expect("no principal variation"),
                )
                .expect("error executing principal variation ply");
        }
    }

    states
}

fn generate_batch_samples<const N: usize>(
    training_state: &TrainingState<N>,
    scaffolds: Vec<State<N>>,
) -> Vec<TrainingSample<State<N>>>
where
    TrainingState<N>: Train<N, State = State<N>>,
{
    let scaffolds = Mutex::new(scaffolds);
    let training_samples = Mutex::new(Vec::new());

    thread::scope(|scope| {
        for _ in 0..GATHER_THREADS.min(BATCHES_PER_UPDATE) {
            scope.spawn(|| {
                let mut rng = rand::thread_rng();

                let mut persistent_state = PersistentState::default();
                let evaluator = training_state.model_as_evaluator();

                'gather: loop {
                    let mut state = {
                        let mut guard = scaffolds.lock().unwrap();

                        let mut state = guard.choose(&mut rng).cloned().unwrap();
                        let ply = *generate_plies(&state).choose(&mut rng).unwrap();
                        state.execute_ply(ply).expect("error executing random ply");

                        guard.push(state.clone());
                        state
                    };

                    // The state at time t.
                    let mut s_t = Vec::new();
                    // The reward for the player to move at time t.
                    let mut r_t = Vec::new();

                    for _ in 0..SAMPLES_PER_POSITION + TD_PLY_DEPTH {
                        let count = training_samples.lock().unwrap().len();
                        if count >= BATCH_SIZE * BATCHES_PER_UPDATE {
                            break 'gather;
                        }

                        // If the state is terminal, reward the last two moves and break.
                        if let Some(resolution) = state.resolution() {
                            let reward = if let Some(color) = resolution.color() {
                                if color == state.to_move() {
                                    1.0
                                } else {
                                    -1.0
                                }
                            } else {
                                0.0
                            };

                            // Give the opposite reward to the opponent's last move.
                            if let Some(last) = r_t.last_mut() {
                                *last = -reward;
                            }

                            s_t.push(state.clone());
                            r_t.push(reward);

                            break;
                        }

                        // Otherwise, perform a search from the state.
                        let config = AnalysisConfig::<N> {
                            time_control: TimeControl::Simple {
                                depth_limit: Some(TD_SEARCH_DEPTH),
                                time_limit: None,
                                early_stop: false,
                            },
                            persistent_state: Some(&mut persistent_state),
                            evaluator: Some(&*evaluator),
                            exact_eval: true,
                            ..Default::default()
                        };

                        let analysis = analyze(config, &state);

                        s_t.push(state.clone());

                        // Grant a static reward for Tinuë, or use the network's output.
                        if analysis.evaluation.is_terminal() {
                            let eval = if matches!(
                                analysis.final_state.resolution(),
                                Some(Resolution::Draw { .. })
                            ) {
                                0.0
                            } else {
                                if analysis.evaluation > 0.0.into() {
                                    1.0
                                } else {
                                    -1.0
                                }
                            };

                            r_t.push(eval);
                        } else {
                            r_t.push(analysis.evaluation.into());
                        }

                        state
                            .execute_ply(
                                *analysis
                                    .principal_variation
                                    .first()
                                    .expect("no principal variation"),
                            )
                            .expect("error executing principal variation ply");
                    }

                    let g_t = calculate_n_step_returns(&r_t, training_state.discount);
                    let g_l_t = calculate_lambda_returns(&g_t, training_state.lambda);

                    let new_samples = s_t
                        .into_iter()
                        .zip(g_l_t)
                        .map(|(input, label)| TrainingSample { input, label });

                    let mut training_samples = training_samples.lock().unwrap();
                    training_samples.extend(new_samples);
                }
            });
        }
    });

    training_samples.into_inner().unwrap()
}

fn calculate_n_step_returns(r_t: &[f32], discount: f32) -> Vec<f32> {
    let mut g_t = Vec::new();

    let end_t = r_t.len();
    for t in 0..end_t {
        let g = r_t[t]
            + (1..end_t - t)
                .map(|n| {
                    let sign = if n % 2 != 0 { -1.0 } else { 1.0 };
                    let delta = sign * (r_t[t + n] + r_t[t + n - 1]);
                    discount.powi(n as i32) * delta
                })
                .sum::<f32>();

        g_t.push(g);
    }

    g_t
}

fn calculate_lambda_returns(g_t: &[f32], lambda: f32) -> Vec<f32> {
    // λ-return for time t.
    let mut g_l_t = Vec::new();

    let end_t = g_t.len();
    for t in 0..end_t {
        let sign = if (end_t - t - 1) % 2 != 0 { -1.0 } else { 1.0 };
        let g = (1.0 - lambda)
            * (0..end_t - t - 1)
                .map(|n| {
                    let sign = if n % 2 != 0 { -1.0 } else { 1.0 };
                    sign * lambda.powi(n as i32) * g_t[t + n]
                })
                .sum::<f32>()
            + sign * lambda.powi((end_t - t - 1) as i32) * g_t[end_t - 1];

        g_l_t.push(g);
    }

    g_l_t
}

fn train_batches<const N: usize>(
    training_state: &mut TrainingState<N>,
    mut batch_samples: &[TrainingSample<State<N>>],
    max_batches: Option<usize>,
    checkpoint_error: &mut f32,
) -> f32
where
    TrainingState<N>: Train<N, State = State<N>>,
{
    let mut error_sum = 0.0;
    let batch_count =
        BATCHES_PER_UPDATE.min(max_batches.unwrap_or(usize::MAX) - training_state.batch);

    assert!(batch_count * BATCH_SIZE <= batch_samples.len());

    for _ in 0..batch_count {
        let (batch, remaining) = batch_samples.split_at(BATCH_SIZE);

        let (_, learning_rate) = LEARNING_RATE_SCHEDULE
            .iter()
            .rev()
            .find(|(b, _)| *b <= training_state.batch)
            .unwrap();
        training_state.learning_rate = *learning_rate;

        let error = training_state.train_batch(batch);
        error_sum += error;
        *checkpoint_error += error;

        if training_state.batch % CHECKPOINT_BATCHES == 0 {
            training_state.error = *checkpoint_error / CHECKPOINT_BATCHES as f32;

            save_training_state(
                &training_state,
                format!(
                    "{TRAINING_DIR}/{MODEL_DIR}/model_{N}s_{:06}.json",
                    training_state.batch
                ),
                format!(
                    "{TRAINING_DIR}/{CHECKPOINT_DIR}/checkpoint_{N}s_{:06}.json",
                    training_state.batch
                ),
            );

            *checkpoint_error = 0.0;
        }

        batch_samples = remaining;
    }

    // Save the latest checkpoint/model too.
    if training_state.batch % CHECKPOINT_BATCHES != 0 {
        training_state.error =
            *checkpoint_error / (training_state.batch % CHECKPOINT_BATCHES) as f32;
    }
    save_training_state(
        &training_state,
        format!("{TRAINING_DIR}/{MODEL_DIR}/latest.json"),
        format!("{TRAINING_DIR}/{CHECKPOINT_DIR}/latest.json"),
    );

    error_sum / batch_count as f32
}

struct TrainingSample<T> {
    input: T,
    label: f32,
}

trait Train<const N: usize> {
    type State;
    type Evaluator: Evaluator<N>;
    type Model: for<'a> Deserialize<'a> + Serialize + Send + Sync;
    type GradientDescent: for<'a> Deserialize<'a> + Serialize + Send + Sync;

    fn new() -> Self;
    fn model_as_evaluator<const M: usize>(&self) -> Box<dyn Evaluator<M>>;
    fn train_batch(&mut self, samples: &[TrainingSample<Self::State>]) -> f32;
}

#[derive(Deserialize, Serialize)]
struct TrainingState<const N: usize>
where
    Self: Train<N>,
{
    batch: usize,
    model: <Self as Train<N>>::Model,
    gradient_descent: <Self as Train<N>>::GradientDescent,
    epsilon: f32,
    discount: f32,
    lambda: f32,
    learning_rate: f32,
    l2_reg: f32,
    error: f32,
}

macro_rules! train_impl {
    ($s:expr) => {
        impl Train<$s> for TrainingState<$s> {
            type State = State<$s>;
            type Evaluator = <AnnModel<$s> as AnnEvaluator<$s>>::Evaluator;
            type Model = <AnnModel<$s> as AnnEvaluator<$s>>::Model;
            type GradientDescent = ShallowAdam<
                { AnnModel::<$s>::INPUTS },
                { AnnModel::<$s>::HIDDEN },
                { AnnModel::<$s>::OUTPUTS },
            >;

            fn new() -> Self {
                Self {
                    batch: 0,
                    model: Self::Model::random(&mut rand::thread_rng()),
                    gradient_descent: Self::GradientDescent::default(),
                    epsilon: 0.05,
                    discount: 0.85,
                    lambda: 0.8,
                    learning_rate: 0.001,
                    l2_reg: 0.0001,
                    error: 0.0,
                }
            }

            fn model_as_evaluator<const N: usize>(&self) -> Box<dyn Evaluator<N>> {
                let evaluator: Box<Self::Evaluator> = Box::new(self.model.clone().into());
                unsafe { std::mem::transmute(evaluator as Box<dyn Evaluator<$s>>) }
            }

            fn train_batch(&mut self, samples: &[TrainingSample<Self::State>]) -> f32 {
                self.batch += 1;

                let mut inputs = MatrixRowMajor::zeros();
                let mut labels = MatrixRowMajor::zeros();

                for (i, sample) in samples.iter().enumerate().take(BATCH_SIZE) {
                    inputs[i] = *sample.input.gather_features().as_vector();
                    labels[i][0] = sample.label;
                }

                let outputs = self.model.propagate_forward(&inputs);
                let error = mse(&outputs, &labels)[0];

                self.model.train_batch::<BATCH_SIZE>(
                    self.batch,
                    &inputs,
                    &labels,
                    mse_prime,
                    &mut self.gradient_descent,
                    self.learning_rate,
                    self.l2_reg,
                );

                error
            }
        }
    };
}

train_impl!(3);
train_impl!(4);
train_impl!(5);
train_impl!(6);
train_impl!(7);
train_impl!(8);

fn generate_plies<const N: usize>(state: &State<N>) -> Vec<Ply<N>> {
    let mut plies = Vec::new();

    let empty_spaces = board_mask() ^ state.metadata.p1_pieces ^ state.metadata.p2_pieces;

    let reserve_flatstones = match state.to_move() {
        Color::White => state.p1_flatstones,
        Color::Black => state.p2_flatstones,
    };

    if reserve_flatstones > 0 {
        plies.extend(generation::placements(empty_spaces, PieceType::Flatstone));
    }

    if state.ply_count >= 2 {
        if reserve_flatstones > 0 {
            plies.extend(generation::placements(
                empty_spaces,
                PieceType::StandingStone,
            ));
        }

        let capstone_reserve = match state.to_move() {
            Color::White => state.p1_capstones,
            Color::Black => state.p2_capstones,
        };

        if capstone_reserve > 0 {
            plies.extend(generation::placements(empty_spaces, PieceType::Capstone));
        }

        match state.to_move() {
            Color::White => plies.extend(generation::spreads(state, state.metadata.p1_pieces)),
            Color::Black => plies.extend(generation::spreads(state, state.metadata.p2_pieces)),
        }
    }

    plies
}

fn save_training_state<const N: usize>(
    training_state: &TrainingState<N>,
    model: impl AsRef<Path>,
    checkpoint: impl AsRef<Path>,
) where
    TrainingState<N>: Train<N>,
{
    let file = File::create(model).expect("could not create model file");
    serde_json::to_writer(file, &training_state.model).expect("could not write to model file");

    let file = File::create(checkpoint).expect("could not create checkpoint file");
    serde_json::to_writer(file, &(N, serde_json::to_string(&training_state).unwrap()))
        .expect("could not write to checkpoint file");
}
