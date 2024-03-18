use std::fs::{self, File};
use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;
use std::{env, thread};

use rand::{self, seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};

use analysis::evaluation::{AnnEvaluator, AnnModel, Evaluator, GatherFeatures};
use analysis::{analyze, AnalysisConfig, PersistentState};
use ann::linear_algebra::MatrixRowMajor;
use ann::loss::{mse, mse_prime};
use ann::shallow::ShallowAdam;
use tak::{board_mask, generation, Color, PieceType, Ply, Resolution, State, Tps};

const BATCH_SIZE: usize = 128;

const TRAINING_DIR: &str = "training";
const MODEL_DIR: &str = "models";
const CHECKPOINT_DIR: &str = "checkpoints";

#[derive(Clone, Debug, Deserialize)]
struct Config {
    /// Size of the board to play on.
    size: usize,
    /// If set, don't train candidates and just resume this training checkpoint.
    #[serde(default)]
    resume_checkpoint: Option<String>,
    /// Optional string to append to checkpoints and saved models.
    #[serde(default)]
    suffix: Option<String>,
    /// The maximum number of threads to use. The actual number is capped at `batches_per_update`.
    #[serde(default)]
    max_threads: Option<usize>,
    /// Maximum updates count.
    max_updates: usize,
    /// The number of batches to generate at a time from the same network state.
    batches_per_update: usize,
    /// The number of updates between saved checkpoints.
    updates_per_checkpoint: usize,
    /// The number of networks to start, selecting the best to continue training.
    starting_candidates: usize,
    /// The number of updates to train each candidate.
    candidate_updates: usize,
    /// The search depth to use when building the positions the training samples are derived from.
    scaffold_search_depth: u32,
    /// The number of consecutive samples to take from one starting position.
    samples_per_position: usize,
    /// The number of plies to play when calculating the temporal difference of the evaulations.
    td_ply_depth: usize,
    /// The search depth to use when calculating the temporal difference of the evaluations.
    td_search_depth: u32,
    /// Pairs of (batch number, learning rate) to indicate changes in learning rate over time.
    learning_rate_schedule: Vec<(usize, f32)>,
    /// The rate at which a random move is made while building scaffolds
    epsilon: f32,
    discount: f32,
    lambda: f32,
    l2_reg: f32,
}

fn main() {
    fs::create_dir_all(format!("{TRAINING_DIR}/{MODEL_DIR}"))
        .expect("could not create model directory");
    fs::create_dir_all(format!("{TRAINING_DIR}/{CHECKPOINT_DIR}"))
        .expect("could not create checkpoint directory");

    let mut args = env::args();

    let config: Config = if let Some(path) = args.nth(1) {
        let file = File::open(path).expect("could not read config file");
        serde_json::from_reader(file).expect("could not parse config file")
    } else {
        eprintln!("Please pass a path to a config file, i.e. \"training/config/config_6s.json\".");
        return;
    };

    match config.size {
        3 => main_sized::<3>(config),
        4 => main_sized::<4>(config),
        5 => main_sized::<5>(config),
        6 => main_sized::<6>(config),
        7 => main_sized::<7>(config),
        8 => main_sized::<8>(config),
        _ => panic!("invalid size in config"),
    }
}

fn main_sized<const N: usize>(config: Config)
where
    TrainingState<N>: Train<N, State = State<N>>,
{
    let mut training_state = if let Some(path) = &config.resume_checkpoint {
        let file = File::open(path).expect("could not read checkpoint file");
        serde_json::from_reader(file).expect("could not parse checkpoint")
    } else {
        TrainingState::<N>::new()
    };

    if config.max_updates == 0 {
        save_training_state(&config, &training_state, "latest", "latest");
        return;
    }

    let mut best_training_state = load_best(&config).unwrap_or_else(|| {
        println!("Could not load best training state. Cloning current training state.");

        let mut best = training_state.clone();
        best.match_results = None;

        save_training_state(&config, &best, "best", "best");

        best
    });

    if matches!(training_state.stage, Stage::Main { .. })
        && best_training_state.update > training_state.update
    {
        println!("Warning: best is more recent than current!");
    }

    // Run a test if we load in a checkpoint that hasn't been tested.
    if matches!(training_state.stage, Stage::Main)
        && training_state.update % config.updates_per_checkpoint == 0
        && training_state.match_results.is_none()
    {
        do_test(&config, &mut best_training_state, &mut training_state);
    }

    while training_state.update < config.max_updates {
        if let Stage::Candidate { number } = training_state.stage {
            if training_state.update == config.candidate_updates {
                // Test candidates only when they're finished.
                if training_state.match_results.is_none() {
                    do_test(&config, &mut best_training_state, &mut training_state);
                }

                if number < config.starting_candidates {
                    let mut next_candidate = TrainingState::new();
                    next_candidate.stage = Stage::Candidate { number: number + 1 };
                    training_state = next_candidate;
                } else {
                    accept_candidate(&mut training_state, &best_training_state);
                }
            }
        }

        do_update(&config, &mut training_state);

        if matches!(training_state.stage, Stage::Main)
            && training_state.update % config.updates_per_checkpoint == 0
        {
            do_test(&config, &mut best_training_state, &mut training_state);
        }
    }
}

fn do_update<const N: usize>(config: &Config, training_state: &mut TrainingState<N>)
where
    TrainingState<N>: Train<N, State = State<N>>,
{
    let mut rng = rand::thread_rng();

    print!(
        "{}Iteration {}... ",
        match training_state.stage {
            Stage::Candidate { number } => format!("Candidate {number} - "),
            _ => String::new(),
        },
        training_state.update + 1,
    );
    std::io::stdout().flush().ok();

    let start_time = Instant::now();

    let scaffolds = build_scaffold_positions(config, training_state);
    let mut batch_samples = generate_batch_samples(config, training_state, scaffolds);
    batch_samples.shuffle(&mut rng);

    let average_error = train_batches(config, training_state, &batch_samples);

    let elapsed = start_time.elapsed().as_secs_f32();
    println!(
        "b: {}, t: {:6.2}s, avg b t: {:5.2}s/b, lr: {}, avg b err: {:.3}, avg chkpt err: {:.3}",
        config.batches_per_update * training_state.update,
        elapsed,
        elapsed / config.batches_per_update as f32,
        training_state.learning_rate,
        average_error,
        training_state.error,
    );

    if training_state.update % config.updates_per_checkpoint == 0 {
        println!("  Saving checkpoint.");
    }
}

fn do_test<const N: usize>(
    config: &Config,
    best_training_state: &mut TrainingState<N>,
    training_state: &mut TrainingState<N>,
) where
    TrainingState<N>: Train<N, State = State<N>>,
{
    println!("Running test...");
    let results = match test_match(config, best_training_state, training_state) {
        TestOutcome::Accepted(results) => {
            *best_training_state = training_state.clone();
            best_training_state.match_results = None;

            println!("Accepted");

            // Save the best checkpoint/model.
            save_training_state(config, best_training_state, "best", "best");

            results
        }
        TestOutcome::Rejected(results) => {
            println!("Rejected");

            results
        }
    };

    training_state.match_results = Some(results);

    let stage = match training_state.stage {
        Stage::Candidate { number } => format!("cand{number:02}_"),
        Stage::Main => String::new(),
    };

    save_checkpoint(
        config,
        training_state,
        format!("checkpoint_{N}s_{stage}{:05}", training_state.update),
    );
}

fn accept_candidate<const N: usize>(
    training_state: &mut TrainingState<N>,
    best_training_state: &TrainingState<N>,
) where
    TrainingState<N>: Train<N, State = State<N>>,
{
    let number = match best_training_state.stage {
        Stage::Candidate { number } => number,
        _ => panic!("best is not a candidate"),
    };

    println!("  Accepting candidate {number}.");

    *training_state = best_training_state.clone();
    training_state.stage = Stage::Main;

    // Copy all models and checkpoints that belonged to this candidate.
    let candidate_name = format!("cand{number:02}_");

    let models = fs::read_dir(format!("{TRAINING_DIR}/{MODEL_DIR}"))
        .expect("cannot read model directory")
        .flat_map(|f| f.ok())
        .map(|f| f.path().to_string_lossy().into_owned())
        .filter(|f| f.contains(&candidate_name));

    for model in models {
        let new_name = model.replace(&candidate_name, "");
        fs::copy(model, new_name).expect("cannot copy model file");
    }

    let checkpoints = fs::read_dir(format!("{TRAINING_DIR}/{CHECKPOINT_DIR}"))
        .expect("cannot read checkpoint directory")
        .flat_map(|f| f.ok())
        .map(|f| f.path().to_string_lossy().into_owned())
        .filter(|f| f.contains(&candidate_name));

    for checkpoint in checkpoints {
        let new_name = checkpoint.replace(&candidate_name, "");
        fs::copy(checkpoint, new_name).expect("cannot copy checkpoint file");
    }
}

fn build_scaffold_positions<const N: usize>(
    config: &Config,
    training_state: &TrainingState<N>,
) -> Vec<State<N>>
where
    TrainingState<N>: Train<N, State = State<N>>,
{
    let mut rng = rand::thread_rng();

    let mut states = Vec::new();
    let persistent_state = PersistentState::default();
    let evaluator = training_state.model_as_evaluator();

    let mut state = State::default();

    while state.resolution().is_none() && state.ply_count < 300 {
        states.push(state.clone());

        // If the game has just started, or if a random number is below epsilon, make a random move.
        // Otherwise, use the principal variation from a search.
        if state.ply_count < 2 || rng.gen::<f32>() < config.epsilon {
            let ply = *generate_plies(&state).choose(&mut rng).unwrap();
            state.execute_ply(ply).expect("error executing random ply");
        } else {
            let config = AnalysisConfig::<N> {
                depth_limit: Some(config.scaffold_search_depth),
                time_limit: None,
                early_stop: false,
                persistent_state: Some(&persistent_state),
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
    config: &Config,
    training_state: &TrainingState<N>,
    scaffolds: Vec<State<N>>,
) -> Vec<TrainingSample<State<N>>>
where
    TrainingState<N>: Train<N, State = State<N>>,
{
    let scaffolds = Mutex::new(scaffolds);
    let training_samples = Mutex::new(Vec::new());

    let max_threads = config.max_threads.unwrap_or(
        thread::available_parallelism()
            .expect("could not determine available parallelism")
            .into(),
    );

    thread::scope(|scope| {
        for _ in 0..max_threads.min(config.batches_per_update) {
            scope.spawn(|| {
                let mut rng = rand::thread_rng();

                let persistent_state = PersistentState::default();
                let evaluator = training_state.model_as_evaluator();

                'gather: loop {
                    let mut state = {
                        let mut guard = scaffolds.lock().unwrap();

                        let mut state = guard.choose(&mut rng).cloned().unwrap();
                        let ply = *generate_plies(&state)
                            .choose(&mut rng)
                            .unwrap_or_else(|| panic!("no plies: {}", Tps::from(state.clone())));
                        state.execute_ply(ply).expect("error executing random ply");

                        guard.push(state.clone());
                        state
                    };

                    // The state at time t.
                    let mut s_t = Vec::new();
                    // The reward for the player to move at time t.
                    let mut r_t = Vec::new();

                    for _ in 0..config.samples_per_position + config.td_ply_depth {
                        let count = training_samples.lock().unwrap().len();
                        if count >= BATCH_SIZE * config.batches_per_update {
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
                            depth_limit: Some(config.td_search_depth),
                            time_limit: None,
                            early_stop: false,
                            persistent_state: Some(&persistent_state),
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
                            } else if analysis.evaluation > 0.0.into() {
                                1.0
                            } else {
                                -1.0
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

                    let g_t = calculate_n_step_returns(&r_t, config.discount);
                    let g_l_t = calculate_lambda_returns(&g_t, config.lambda);

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
    config: &Config,
    training_state: &mut TrainingState<N>,
    mut batch_samples: &[TrainingSample<State<N>>],
) -> f32
where
    TrainingState<N>: Train<N, State = State<N>>,
{
    let mut error_sum = 0.0;

    assert!(config.batches_per_update * BATCH_SIZE <= batch_samples.len());

    for _ in 0..config.batches_per_update {
        let (batch, remaining) = batch_samples.split_at(BATCH_SIZE);

        let (_, learning_rate) = config
            .learning_rate_schedule
            .iter()
            .rev()
            .find(|(u, _)| *u <= training_state.update)
            .unwrap();
        training_state.learning_rate = *learning_rate;

        let error = training_state.train_batch(config, batch);
        error_sum += error;
        training_state.checkpoint_error_acc += error;

        batch_samples = remaining;
    }

    training_state.update += 1;

    if training_state.update % config.updates_per_checkpoint == 0 {
        training_state.error = training_state.checkpoint_error_acc
            / (config.batches_per_update * config.updates_per_checkpoint) as f32;

        let stage = match training_state.stage {
            Stage::Candidate { number } => format!("cand{number:02}_"),
            Stage::Main => String::new(),
        };

        training_state.match_results = None;

        save_training_state(
            config,
            training_state,
            format!("model_{N}s_{stage}{:05}", training_state.update),
            format!("checkpoint_{N}s_{stage}{:05}", training_state.update),
        );

        training_state.checkpoint_error_acc = 0.0;
    } else {
        training_state.error = training_state.checkpoint_error_acc
            / (config.batches_per_update * (training_state.update % config.updates_per_checkpoint))
                as f32;
    }

    // Save the latest checkpoint/model too.
    save_training_state(config, training_state, "latest", "latest");

    error_sum / config.batches_per_update as f32
}

struct TrainingSample<T> {
    input: T,
    label: f32,
}

trait Train<const N: usize> {
    type State;
    type Evaluator: Evaluator<N>;
    type Model: Clone + for<'a> Deserialize<'a> + Serialize + Send + Sync;
    type GradientDescent: Clone + for<'a> Deserialize<'a> + Serialize + Send + Sync;

    fn new() -> Self;
    fn model_as_evaluator<const M: usize>(&self) -> Box<dyn Evaluator<M>>;
    fn train_batch(&mut self, config: &Config, samples: &[TrainingSample<Self::State>]) -> f32;
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(tag = "name", rename_all = "snake_case")]
enum Stage {
    Candidate { number: usize },
    Main,
}

#[derive(Clone, Deserialize, Serialize)]
struct TrainingState<const N: usize>
where
    Self: Train<N>,
{
    batch: usize,
    update: usize,
    stage: Stage,
    model: <Self as Train<N>>::Model,
    gradient_descent: <Self as Train<N>>::GradientDescent,
    learning_rate: f32,
    error: f32,
    checkpoint_error_acc: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    match_results: Option<Results>,
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
                    update: 0,
                    stage: Stage::Candidate { number: 1 },
                    model: Self::Model::random(&mut rand::thread_rng()),
                    gradient_descent: Self::GradientDescent::default(),
                    learning_rate: 0.001,
                    error: 0.0,
                    checkpoint_error_acc: 0.0,
                    match_results: None,
                }
            }

            fn model_as_evaluator<const N: usize>(&self) -> Box<dyn Evaluator<N>> {
                let evaluator: Box<Self::Evaluator> = Box::new(self.model.clone().into());
                unsafe { std::mem::transmute(evaluator as Box<dyn Evaluator<$s>>) }
            }

            fn train_batch(
                &mut self,
                config: &Config,
                samples: &[TrainingSample<Self::State>],
            ) -> f32 {
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
                    config.l2_reg,
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

fn save_checkpoint<const N: usize>(
    config: &Config,
    training_state: &TrainingState<N>,
    checkpoint_name: impl AsRef<str>,
) where
    TrainingState<N>: Train<N>,
{
    let suffix = match &config.suffix {
        Some(suffix) => format!("-{suffix}"),
        None => String::new(),
    };

    let path = format!(
        "{TRAINING_DIR}/{CHECKPOINT_DIR}/{}{suffix}.json",
        checkpoint_name.as_ref()
    );
    let file = File::create(path).expect("could not create checkpoint file");
    serde_json::to_writer(file, &training_state).expect("could not write to checkpoint file");
}

fn save_model<const N: usize>(
    config: &Config,
    training_state: &TrainingState<N>,
    model_name: impl AsRef<str>,
) where
    TrainingState<N>: Train<N>,
{
    let suffix = match &config.suffix {
        Some(suffix) => format!("-{suffix}"),
        None => String::new(),
    };

    let path = format!(
        "{TRAINING_DIR}/{MODEL_DIR}/{}{suffix}.json",
        model_name.as_ref()
    );
    let file = File::create(path).expect("could not create model file");
    serde_json::to_writer(file, &training_state.model).expect("could not write to model file");
}

fn save_training_state<const N: usize>(
    config: &Config,
    training_state: &TrainingState<N>,
    model_name: impl AsRef<str>,
    checkpoint_name: impl AsRef<str>,
) where
    TrainingState<N>: Train<N>,
{
    save_model(config, training_state, model_name);
    save_checkpoint(config, training_state, checkpoint_name);
}

fn load_best<const N: usize>(config: &Config) -> Option<TrainingState<N>>
where
    TrainingState<N>: Train<N>,
{
    let suffix = match &config.suffix {
        Some(suffix) => format!("-{suffix}"),
        None => String::new(),
    };

    let path = format!("{TRAINING_DIR}/{CHECKPOINT_DIR}/best{suffix}.json");

    let file = File::open(path).ok()?;
    serde_json::from_reader(file).ok()
}

enum TestOutcome {
    Accepted(Results),
    Rejected(Results),
}

fn test_match<const N: usize>(
    config: &Config,
    best: &mut TrainingState<N>,
    candidate: &mut TrainingState<N>,
) -> TestOutcome
where
    TrainingState<N>: Train<N>,
{
    // A 2σ result out of 425 matches is a win rate of roughly 55%.
    const MATCHES: usize = 425;
    const SIGNIFICANCE_THRESHOLD: f32 = 2.0;

    let results = Mutex::new(Results {
        matches_remaining: MATCHES,
        a_update: candidate.update,
        b_update: best.update,
        a_wins: 0,
        b_wins: 0,
        draws: 0,
    });

    let max_threads = config.max_threads.unwrap_or(
        thread::available_parallelism()
            .expect("could not determine available parallelism")
            .into(),
    );

    let start = Instant::now();

    thread::scope(|scope| {
        for _ in 0..max_threads {
            scope.spawn(|| {
                let mut rng = rand::thread_rng();

                let a_evaluator = candidate.model_as_evaluator();
                let b_evaluator = best.model_as_evaluator();

                'run_matches: loop {
                    let game_number = {
                        let mut results = results.lock().unwrap();

                        if results.matches_remaining >= 1 {
                            results.matches_remaining -= 1;
                        } else {
                            break 'run_matches;
                        }

                        MATCHES - results.matches_remaining + 1
                    };

                    let (p1, p2) = if game_number % 2 == 0 {
                        (&a_evaluator, &b_evaluator)
                    } else {
                        (&b_evaluator, &a_evaluator)
                    };

                    let p1_persistent_state = PersistentState::default();
                    let p2_persistent_state = PersistentState::default();

                    // Execute two random plies on a new board to start the game.
                    let mut state = State::default();
                    let ply = *generate_plies(&state).choose(&mut rng).unwrap();
                    state.execute_ply(ply).expect("error executing random ply");
                    let ply = *generate_plies(&state).choose(&mut rng).unwrap();
                    state.execute_ply(ply).expect("error executing random ply");

                    while state.resolution().is_none() && state.ply_count < 300 {
                        let (player, persistent_state) = if state.ply_count % 2 == 0 {
                            (&**p1, &p1_persistent_state)
                        } else {
                            (&**p2, &p2_persistent_state)
                        };

                        let config = AnalysisConfig::<N> {
                            depth_limit: Some(config.td_search_depth),
                            time_limit: None,
                            early_stop: false,
                            persistent_state: Some(persistent_state),
                            evaluator: Some(player),
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

                    let mut results = results.lock().unwrap();
                    match state.resolution() {
                        Some(Resolution::Road(color)) | Some(Resolution::Flats { color, .. }) => {
                            match color {
                                Color::White => {
                                    if game_number % 2 == 0 {
                                        results.a_wins += 1;
                                    } else {
                                        results.b_wins += 1;
                                    }
                                }
                                Color::Black => {
                                    if game_number % 2 == 0 {
                                        results.b_wins += 1;
                                    } else {
                                        results.a_wins += 1;
                                    }
                                }
                            }
                        }
                        Some(Resolution::Draw) | None => {
                            results.draws += 1;
                        }
                    }

                    println!(
                        "  {:3}/{}  +{}-{}={}{}",
                        results.a_wins + results.b_wins + results.draws,
                        MATCHES,
                        results.a_wins,
                        results.b_wins,
                        results.draws,
                        if let Some(sigma) = results.z_test() {
                            format!(" {sigma:.2}σ")
                        } else {
                            String::new()
                        },
                    );
                }
            });
        }
    });

    let elapsed = start.elapsed();
    println!(
        "  Finished in {:.2}s, {:.2}s/match",
        elapsed.as_secs_f32(),
        elapsed.as_secs_f32() / MATCHES as f32
    );

    let results = results.lock().unwrap();

    if results.a_wins > results.b_wins
        && results.z_test().expect("no result significance") >= SIGNIFICANCE_THRESHOLD
    {
        TestOutcome::Accepted(*results)
    } else {
        TestOutcome::Rejected(*results)
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
struct Results {
    #[serde(skip)]
    matches_remaining: usize,
    a_update: usize,
    b_update: usize,
    a_wins: usize,
    b_wins: usize,
    draws: usize,
}

impl Results {
    fn z_test(&self) -> Option<f32> {
        let total = self.a_wins + self.b_wins + 2 * self.draws;
        if total < 20 {
            return None;
        }

        let numerator = (self.a_wins + self.draws) as f32 - total as f32 / 2.0;
        if numerator == 0.0 {
            return Some(0.0);
        }

        let z = ((numerator + numerator.signum() * 0.5) / (total as f32 * 0.5 * 0.5).sqrt()).abs();

        Some(z)
    }
}
