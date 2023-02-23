use std::env;
use std::fs::File;

use analysis::evaluation::{evaluate, Evaluation};
use tak::{State, Tps};

fn main() {
    let input =
        File::open(env::args().nth(1).expect("expected input file")).expect("could not open file");

    let tps_strings: Vec<String> = serde_json::from_reader(input).expect("could not read file");

    let tps = tps_strings
        .into_iter()
        .map(|s| s.parse::<Tps>().expect("could not parse tps"))
        .collect::<Vec<_>>();

    let evaluations: Vec<i32> = tps
        .into_iter()
        .map(|tps| evaluate_tps(tps).into())
        .collect::<Vec<_>>();

    println!("{}", serde_json::to_string_pretty(&evaluations).unwrap());
}

fn evaluate_tps(tps: Tps) -> Evaluation {
    match tps.size() {
        3 => evaluate_tps_sized::<3>(tps),
        4 => evaluate_tps_sized::<4>(tps),
        5 => evaluate_tps_sized::<5>(tps),
        6 => evaluate_tps_sized::<6>(tps),
        7 => evaluate_tps_sized::<7>(tps),
        8 => evaluate_tps_sized::<8>(tps),
        _ => unreachable!(),
    }
}

fn evaluate_tps_sized<const N: usize>(tps: Tps) -> Evaluation {
    let state: State<N> = tps.try_into().expect("could not create state from tps");
    evaluate(&state)
}
