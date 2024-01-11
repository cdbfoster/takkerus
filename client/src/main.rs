use std::env;

use clap::Parser;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt::format;

use self::analyze::run_analysis;
use self::args::{Args, Command};
use self::play::run_game;
use self::tei::run_tei;

mod analyze;
mod args;
mod play;
mod player;
mod tei;

fn main() {
    let args = Args::parse();

    if matches!(args.command, Command::Analyze(_)) {
        set_default_logging();
    }

    let event_format = format().with_target(false).without_time();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .event_format(event_format)
        .init();

    // Limit the number of threads async-std tries to spawn; we don't need that many.
    if env::var("ASYNC_STD_THREAD_COUNT").is_err() {
        env::set_var("ASYNC_STD_THREAD_COUNT", "1");
    }

    match args.command {
        Command::Play(config) => run_game(config),
        Command::Analyze(config) => run_analysis(config),
        Command::Tei(config) => run_tei(config),
    }
}

fn set_default_logging() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
}
