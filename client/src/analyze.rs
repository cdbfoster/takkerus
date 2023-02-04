use std::{fmt::Write, time::Duration};

use tracing::error;

use analysis::{analyze, AnalysisConfig};
use tak::{PtnGame, PtnHeader, State, Tps};

use crate::args::{Ai, AnalyzeConfig};

pub fn run_analysis(config: AnalyzeConfig) {
    let game = match (&config.file, &config.tps) {
        (Some(filename), None) => match PtnGame::from_file(filename) {
            Ok(game) => game,
            Err(err) => {
                error!(error = ?err, "Invalid PTN file.");
                return;
            }
        },
        (None, Some(tps_string)) => {
            let tps = match tps_string.parse::<Tps>() {
                Ok(tps) => tps,
                Err(err) => {
                    error!(error = ?err, "Invalid TPS string.");
                    return;
                }
            };

            PtnGame {
                headers: vec![PtnHeader::new("TPS", tps)],
                ..Default::default()
            }
        }
        _ => unreachable!(),
    };

    if let Some(size) = game.get_size() {
        match size {
            3 => run_analysis_sized::<3>(config, game),
            4 => run_analysis_sized::<4>(config, game),
            5 => run_analysis_sized::<5>(config, game),
            6 => run_analysis_sized::<6>(config, game),
            7 => run_analysis_sized::<7>(config, game),
            8 => run_analysis_sized::<8>(config, game),
            _ => error!(?size, "Invalid board size."),
        }
    } else {
        error!("Could not determine board size.");
    }
}

fn run_analysis_sized<const N: usize>(config: AnalyzeConfig, game: PtnGame) {
    let state: State<N> = match game.try_into() {
        Ok(state) => state,
        Err(err) => {
            error!(error = ?err, "Could not create state.");
            return;
        }
    };

    let Ai {
        depth_limit,
        time_limit,
        predict_time,
    } = config.ai;

    let analysis_config = AnalysisConfig::<N> {
        depth_limit,
        time_limit,
        predict_time,
        ..Default::default()
    };

    let analysis = analyze(analysis_config, &state);

    let game = {
        let tps: Tps = state.clone().into();

        let mut game = PtnGame {
            headers: vec![PtnHeader::new("TPS", tps)],
            ..Default::default()
        };

        for ply in analysis.principal_variation {
            game.add_ply(ply).expect("could not add pv ply");
        }

        game
    };

    println!("\n--------------------------------------------------");
    println!("\nState:");
    println!("{state}");

    println!("\nTo Move: {:?}", state.to_move());

    println!("\nEvaluation: {}", analysis.evaluation);

    println!("\nPrincipal Variation:");
    for turn in &game.turns {
        println!("  {turn:<7}");
    }
    if let Some(result) = &game.result {
        println!("  {result}");
    }

    println!("\nStatistics:");
    println!("  Depth: {} plies", analysis.depth);
    println!("  Time: {}", format_time(analysis.time));
    println!(
        "  Visited:   {:>13} nodes, {:>10} nodes/s",
        punctuate(analysis.stats.visited),
        punctuate((analysis.stats.visited as f64 / analysis.time.as_secs_f64()) as u64),
    );
    println!(
        "  Evaluated: {:>13} nodes, {:>10} nodes/s",
        punctuate(analysis.stats.evaluated),
        punctuate((analysis.stats.evaluated as f64 / analysis.time.as_secs_f64()) as u64),
    );

    let resulting_state: State<N> = match game.try_into() {
        Ok(state) => state,
        Err(err) => {
            error!(error = ?err, "Could not apply principal variation.");
            return;
        }
    };

    println!("\nResulting State:");
    println!("{resulting_state}");

    println!();
}

fn format_time(time: Duration) -> String {
    let mut buffer = String::new();
    let mut total_secs = time.as_secs();

    let hours = total_secs / 3600;
    total_secs %= 3600;
    if hours > 0 {
        write!(buffer, "{hours} hour{}, ", if hours > 1 { "s" } else { "" }).unwrap();
    }

    let minutes = total_secs / 60;
    total_secs %= 60;
    if hours > 0 || minutes > 0 {
        write!(
            buffer,
            "{minutes} minute{}, ",
            if minutes > 1 { "s" } else { "" }
        )
        .unwrap();
    }

    let secs = total_secs as f64 + time.subsec_millis() as f64 / 1000.0;
    write!(buffer, "{secs:.4} seconds").unwrap();

    buffer
}

fn punctuate(value: u64) -> String {
    let mut buffer = String::new();

    let value = value.to_string();
    for (i, d) in value.chars().enumerate() {
        let x = value.len() - i;

        if x % 3 == 0 && i > 0 && x > 0 {
            write!(buffer, ",").unwrap();
        }
        write!(buffer, "{d}").unwrap();
    }

    buffer
}
