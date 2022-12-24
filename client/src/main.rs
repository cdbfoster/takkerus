use std::env;

use clap::Parser;

use self::args::{Args, Command, Player};
use self::game::PlayerInitializer;
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

    match &args.command {
        Command::Play { game, .. } => match game.size {
            3 => run_game::<3>(args),
            4 => run_game::<4>(args),
            5 => run_game::<5>(args),
            6 => run_game::<6>(args),
            7 => run_game::<7>(args),
            8 => run_game::<8>(args),
            _ => panic!("invalid game size"),
        },
        Command::Analyze { .. } => (),
    }
}

fn run_game<const N: usize>(args: Args) {
    let (p1, p2, _game) = match &args.command {
        Command::Play { p1, p2, game } => (p1, p2, game),
        _ => panic!("invalid command"),
    };

    let p1_initialize = initialize_player(p1);
    let p2_initialize = initialize_player(p2);

    let state = tak::State::<N>::default();

    game::run(p1_initialize, p2_initialize, state);
}

fn initialize_player<const N: usize>(player: &Player) -> impl PlayerInitializer<N> + '_ {
    match player {
        Player::Human(player) => |to_game| human::initialize(Some(player.name.clone()), to_game),
        Player::Ai(_) => unimplemented!(),
    }
}
