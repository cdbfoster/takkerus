use std::env;

use clap::Parser;

use self::analyze::run_analysis;
use self::args::{Args, Command};
use self::play::run_game;

mod analyze;
mod args;
mod message;
mod play;
mod player;

fn main() {
    let args = Args::parse();

    tracing_subscriber::fmt::init();

    // Limit the number of threads async-std tries to spawn; we don't need that many.
    if env::var("ASYNC_STD_THREAD_COUNT").is_err() {
        env::set_var("ASYNC_STD_THREAD_COUNT", "1");
    }

    match args.command {
        Command::Play(config) => run_game(config),
        Command::Analyze(config) => run_analysis(config),
    }
}
