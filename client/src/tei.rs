//! A limited TEI implementation, suitable for running analysis via (RaceTrack)[https://github.com/MortenLohne/racetrack].

use std::fmt::Write;
use std::io;
use std::sync::Mutex;
use std::time::Duration;

use async_std::channel::{self, Sender};
use async_std::io::{prelude::BufReadExt, stdin, BufReader};
use async_std::prelude::*;
use async_std::task;
use once_cell::sync::Lazy;
use tracing::error;

use analysis::{
    analyze, version, Analysis, AnalysisConfig, PersistentState, Sender as SenderTrait, TimeControl,
};
use tak::{Color, Komi, PtnGame, PtnPly, State, Tps};

use crate::analyze::load_model;
use crate::args::{Ai, TeiConfig};

pub fn run_tei(config: TeiConfig) {
    let ai = config.ai;

    task::block_on(listen_spawner(ai));
}

async fn listen_spawner(ai: Ai) {
    let mut size = 6;
    let mut komi = Komi::default();

    let mut game;
    macro_rules! new_game {
        () => {{
            game = PtnGame::default();
            game.add_header("Komi", komi);
        }};
    }
    new_game!();

    let mut input = BufReader::new(stdin()).lines();

    while let Some(message) = input.next().await {
        let message = message.expect("could not read from stdin");

        let mut parts = message.split_whitespace();

        match parts.next().expect("no input") {
            "tei" => {
                println!("id name Takkerus {}", version());
                println!("id author Christopher Foster");
                println!("option name HalfKomi type spin default 0 min -10 max 10");
                println!("teiok")
            }
            "isready" => println!("readyok"),
            "setoption" => {
                assert_eq!(parts.next().unwrap(), "name");

                match parts.next().expect("no option name") {
                    "HalfKomi" => {
                        assert_eq!(parts.next().unwrap(), "value");

                        let half_komi = parts
                            .next()
                            .expect("no half-komi value")
                            .parse::<i8>()
                            .expect("invalid half-komi value");

                        komi = Komi::from_half_komi(half_komi);
                    }
                    x => error!(option = ?x, "Unknown option."),
                }
            }
            "teinewgame" => {
                size = parts
                    .next()
                    .expect("no size parameter")
                    .parse::<usize>()
                    .expect("invalid size parameter");

                if !(3..=8).contains(&size) {
                    panic!("invalid size parameter");
                }

                clear_persistent_state(size);
            }
            "position" => {
                match parts.next().expect("no position") {
                    "startpos" => new_game!(),
                    "tps" => {
                        let tps = parts
                            .next()
                            .expect("no tps string")
                            .parse::<Tps>()
                            .expect("invalid tps string");

                        new_game!();
                        game.add_header("TPS", tps);
                    }
                    _ => panic!("invalid position"),
                }

                assert_eq!(parts.next().expect("no moves"), "moves");

                for ply in parts {
                    add_ply(size, &mut game, ply);
                }
            }
            "go" => {
                let mut time_controls = (TimeControl::default(), TimeControl::default());

                while let Some(part) = parts.next() {
                    let time = match part {
                        "wtime" => &mut time_controls.0.time,
                        "btime" => &mut time_controls.1.time,
                        "winc" => &mut time_controls.0.increment,
                        "binc" => &mut time_controls.1.increment,
                        _ => panic!("unexpected value"),
                    };

                    *time = Duration::from_millis(
                        parts
                            .next()
                            .expect("no time value")
                            .parse()
                            .expect("invalid time value"),
                    );
                }

                begin_analysis(size, &game, ai.clone(), time_controls).await;
            }
            "quit" => break,
            x => error!(input = ?x, "Unexpected input."),
        }
    }
}

static PERSISTENT_STATE_3S: Lazy<Mutex<PersistentState<3>>> = Lazy::new(Default::default);
static PERSISTENT_STATE_4S: Lazy<Mutex<PersistentState<4>>> = Lazy::new(Default::default);
static PERSISTENT_STATE_5S: Lazy<Mutex<PersistentState<5>>> = Lazy::new(Default::default);
static PERSISTENT_STATE_6S: Lazy<Mutex<PersistentState<6>>> = Lazy::new(Default::default);
static PERSISTENT_STATE_7S: Lazy<Mutex<PersistentState<7>>> = Lazy::new(Default::default);
static PERSISTENT_STATE_8S: Lazy<Mutex<PersistentState<8>>> = Lazy::new(Default::default);

fn clear_persistent_state(size: usize) {
    fn sized<const N: usize>(persistent_state: &'static Mutex<PersistentState<N>>) {
        let mut guard = persistent_state.lock().unwrap();
        *guard = PersistentState::default();
    }

    match size {
        3 => sized(&PERSISTENT_STATE_3S),
        4 => sized(&PERSISTENT_STATE_4S),
        5 => sized(&PERSISTENT_STATE_5S),
        6 => sized(&PERSISTENT_STATE_6S),
        7 => sized(&PERSISTENT_STATE_7S),
        8 => sized(&PERSISTENT_STATE_8S),
        _ => unreachable!(),
    }
}

async fn begin_analysis(
    size: usize,
    game: &PtnGame,
    ai: Ai,
    time_controls: (TimeControl, TimeControl),
) {
    async fn sized<const N: usize>(
        game: &PtnGame,
        ai: Ai,
        time_controls: (TimeControl, TimeControl),
        persistent_state: &'static Mutex<PersistentState<N>>,
    ) {
        let Ai {
            depth_limit,
            time_limit,
            early_stop,
            exact_eval,
            threads,
            model_file,
        } = ai;

        let state: State<N> = game.clone().try_into().expect("could not create state");

        struct AnalysisSender<const M: usize>(Sender<Analysis<M>>);

        impl<const M: usize> SenderTrait<Analysis<M>> for AnalysisSender<M> {
            fn send(&self, value: Analysis<M>) -> Result<(), io::Error> {
                self.0
                    .try_send(value)
                    .map_err(|_| io::Error::new(io::ErrorKind::Other, "could not send analysis"))
            }
        }

        let (sender, receiver) = {
            let (s, r) = channel::unbounded();
            (AnalysisSender::<N>(s), r)
        };

        task::spawn_blocking(move || {
            let guard = persistent_state.lock().unwrap();

            let evaluator = model_file.as_deref().map(load_model);

            let analysis_config = AnalysisConfig {
                depth_limit,
                time_limit,
                early_stop,
                time_control: Some(match state.to_move() {
                    Color::White => time_controls.0,
                    Color::Black => time_controls.1,
                }),
                persistent_state: Some(&*guard),
                exact_eval,
                evaluator: evaluator.as_deref(),
                interim_analysis_sender: Some(Box::new(sender)),
                threads,
                ..Default::default()
            };

            analyze(analysis_config, &state);
        });

        task::spawn(async move {
            let mut last_analysis = None;

            let mut iteration = 0;

            while let Ok(analysis) = receiver.recv().await {
                iteration += 1;
                // Or something.
                let centiflats = (f32::from(analysis.evaluation) * 1000.0) as i32;
                let nps = (analysis.stats.visited as f64 / analysis.time.as_secs_f64()) as u64;
                let mut state = analysis.state.clone();

                let mut info = String::from("info");
                write!(info, " depth {iteration}").unwrap();
                write!(info, " seldepth {}", analysis.depth).unwrap();
                write!(info, " score cp {centiflats}").unwrap();
                write!(info, " time {}", analysis.time.as_millis()).unwrap();
                write!(info, " nodes {}", analysis.stats.visited).unwrap();
                write!(info, " nps {nps}",).unwrap();
                write!(info, " pv").unwrap();
                for &ply in &analysis.principal_variation {
                    let validation = state.execute_ply(ply).expect("invalid ply in pv");
                    let ptn: PtnPly = (ply, validation).into();
                    write!(info, " {ptn}").unwrap();
                }
                println!("{info}");

                last_analysis = Some(analysis);
            }

            if let Some(analysis) = last_analysis {
                if let Some(&ply) = analysis.principal_variation.first() {
                    let validation = analysis
                        .state
                        .validate_ply(ply)
                        .expect("invalid ply bestmove");
                    let ptn: PtnPly = (ply, validation).into();
                    println!("bestmove {ptn}");
                } else {
                    error!(?analysis, "No PV returned from search.");
                }
            }
        });
    }

    match size {
        3 => sized(game, ai, time_controls, &PERSISTENT_STATE_3S).await,
        4 => sized(game, ai, time_controls, &PERSISTENT_STATE_4S).await,
        5 => sized(game, ai, time_controls, &PERSISTENT_STATE_5S).await,
        6 => sized(game, ai, time_controls, &PERSISTENT_STATE_6S).await,
        7 => sized(game, ai, time_controls, &PERSISTENT_STATE_7S).await,
        8 => sized(game, ai, time_controls, &PERSISTENT_STATE_8S).await,
        _ => unreachable!(),
    }
}

fn add_ply(size: usize, game: &mut PtnGame, ply: &str) {
    match size {
        3 => game.add_ply::<3>(ply.parse().expect("invalid ply string")),
        4 => game.add_ply::<4>(ply.parse().expect("invalid ply string")),
        5 => game.add_ply::<5>(ply.parse().expect("invalid ply string")),
        6 => game.add_ply::<6>(ply.parse().expect("invalid ply string")),
        7 => game.add_ply::<7>(ply.parse().expect("invalid ply string")),
        8 => game.add_ply::<8>(ply.parse().expect("invalid ply string")),
        _ => unreachable!(),
    }
    .expect("could not add ply");
}
