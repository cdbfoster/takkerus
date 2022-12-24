use std::env;

use clap::Parser;

use self::args::{Args, Command};
use self::player::human;

mod args;
mod game;
mod message;
mod player;

fn main() {
    let args = Args::parse();

    tracing_subscriber::fmt::init();

    // Limit the number of threads async-std tries to spawn; we don't need that many.
    if env::var("ASYNC_STD_THREAD_COUNT").is_err() {
        env::set_var("ASYNC_STD_THREAD_COUNT", "1");
    }

    match args.command {
        Command::Play { ref game, .. } => match game.size {
            3 => run_game::<3>(args),
            4 => run_game::<4>(args),
            5 => run_game::<5>(args),
            6 => run_game::<6>(args),
            7 => run_game::<7>(args),
            8 => run_game::<8>(args),
            _ => panic!("invalide game size"),
        },
        Command::Analyze { .. } => (),
    }

    std::thread::sleep(std::time::Duration::from_secs(10));
}

fn run_game<const N: usize>(args: Args) {
    game::run(
        human::initialize,
        human::initialize,
        tak::State::<N>::default(),
    );
}
