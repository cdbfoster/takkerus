use std::env;

use tak::{PtnGame, State, Tps};

fn main() {
    let games = env::args()
        .skip(1)
        .map(|path| PtnGame::from_file(path).expect("cannot read ptn file"))
        .collect::<Vec<_>>();

    if games.is_empty() {
        println!("[]");
        return;
    }

    let max_length = games.iter().map(|g| g.get_ply_len()).max().unwrap();

    let mut tps = Vec::new();

    for ply in 1..=max_length {
        let states = games.iter().filter_map(|game| get_tps_at_ply(game, ply));

        for state in states {
            if !tps.contains(&state) {
                tps.push(state);
            }
        }
    }

    println!("{}", serde_json::to_string_pretty(&tps).unwrap());
}

fn get_tps_at_ply(game: &PtnGame, ply: usize) -> Option<String> {
    let size = game.get_size().expect("cannot determine game size");

    match size {
        3 => get_tps_at_ply_sized::<3>(game, ply),
        4 => get_tps_at_ply_sized::<4>(game, ply),
        5 => get_tps_at_ply_sized::<5>(game, ply),
        6 => get_tps_at_ply_sized::<6>(game, ply),
        7 => get_tps_at_ply_sized::<7>(game, ply),
        8 => get_tps_at_ply_sized::<8>(game, ply),
        _ => unreachable!(),
    }
}

fn get_tps_at_ply_sized<const N: usize>(game: &PtnGame, ply: usize) -> Option<String> {
    if ply > game.get_ply_len() {
        return None;
    }

    let state: State<N> = game.get_state_at_ply(ply).expect("could not get state");
    let tps: Tps = state.into();

    Some(tps.to_string())
}
