use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use analysis::evaluate;
use tak::State;

criterion_main!(benches);
criterion_group!(
    benches,
    evaluate_empty_6s,
    evaluate_alion_1,
    evaluate_alion_2,
    evaluate_alion_3,
    evaluate_alion_4,
    evaluate_alion_5,
    evaluate_test_5s,
    evaluate_test_7s,
    evaluate_topaz_1,
    evaluate_opening_1,
    evaluate_opening_2,
    evaluate_start_1,
    evaluate_start_2,
    evaluate_start_3,
    evaluate_start_4,
    evaluate_midgame_1,
    evaluate_midgame_2,
    evaluate_midgame_3,
    evaluate_endgame_1,
    evaluate_endgame_2,
    evaluate_endgame_3
);

fn bench_position(c: &mut Criterion, size: usize, key: &str, tps: &str) {
    c.benchmark_group("benches")
        .measurement_time(Duration::from_secs(30))
        .sample_size(2000)
        .bench_function(&format!("evaluate_{key}"), |b| {
            macro_rules! iter {
                ($size:literal) => {{
                    let state: State<$size> = tps.parse().unwrap();
                    b.iter(|| evaluate(black_box(&state)))
                }};
            }

            match size {
                3 => iter!(3),
                4 => iter!(4),
                5 => iter!(5),
                6 => iter!(6),
                7 => iter!(7),
                8 => iter!(8),
                _ => unreachable!(),
            }
        });
}

pub fn evaluate_empty_6s(c: &mut Criterion) {
    bench_position(c, 6, "empty_6s", "x6/x6/x6/x6/x6/x6 1 1");
}

pub fn evaluate_alion_1(c: &mut Criterion) {
    bench_position(c, 6, "alion_1", "2,1221122,1,1,1,2S/1,1,1,x,1C,1111212/x2,2,212,2C,11/2,2,x2,1,1/x3,1,1,x/x2,2,21,x,112S 2 32");
}

pub fn evaluate_alion_2(c: &mut Criterion) {
    bench_position(c, 6, "alion_2", "2,212221C,2,2,2C,1/1,2,1,1,2,1/12,x,1S,2S,2,1/2,2,2,x2,1/1,2212121S,2,12,1,1S/x,2,2,2,x,1 1 30");
}

pub fn evaluate_alion_3(c: &mut Criterion) {
    bench_position(
        c,
        6,
        "alion_3",
        "x2,1,21,2,2/1,2,21,1,21,2/1S,2,2,2C,2,2/21S,1,121C,x,1,12/2,2,121,1,1,1/2,2,x3,22S 1 27",
    );
}

pub fn evaluate_alion_4(c: &mut Criterion) {
    bench_position(
        c,
        6,
        "alion_4",
        "x,1,x4/2,2,1,1,1,1/2221,x,1,21C,x2/2,2,2C,1,2,x/2,2,1,1,1,2/2,x2,2,x,1 2 18",
    );
}

pub fn evaluate_alion_5(c: &mut Criterion) {
    bench_position(
        c,
        6,
        "alion_5",
        "2,x4,11/x5,221/x,2,2,2,x,221/2,1,12C,1,21C,2/2,x,2,x2,2/x,2,2,2,x,121 1 25",
    );
}

pub fn evaluate_test_5s(c: &mut Criterion) {
    bench_position(
        c,
        5,
        "test_5s",
        "2,2,x2,1/2,2,x,1,1/1221S,1,122221C,x,1/1,12,x,2C,2/1S,2,2,x2 1 20",
    );
}

pub fn evaluate_test_7s(c: &mut Criterion) {
    bench_position(c, 7, "test_7s", "2,2,21S,2,1,1,1/2,1,x,2,1,x,1/2,2,2,2,21112C,121S,x/x2,1112C,2,1,1112S,x/121,22211C,1S,1,1,121,1221C/x,2,2,2,1,12,2/2,x3,1,122,x 2 50");
}

pub fn evaluate_topaz_1(c: &mut Criterion) {
    bench_position(c, 6, "topaz_1", "x2,1,x,1212,x/x,221,2212221211C,2S,x2/x,221,1,2,2,x/221,2,12C,1,2,2/22221S,221S,1,1,2,x/12,x,12,1,1,x 1 44");
}

pub fn evaluate_opening_1(c: &mut Criterion) {
    bench_position(
        c,
        6,
        "opening_1",
        "x2,2,x3/x,2,2,x3/x,1,1,2,2,1/1C,2,12C,1,1,x/x,2,x2,1,1/2,x4,1 1 11",
    );
}

pub fn evaluate_opening_2(c: &mut Criterion) {
    bench_position(
        c,
        6,
        "opening_2",
        "2,x5/x3,1,2,x/2,2,221C,12C,1,2/x,2,x,1,1,x/x2,2,1,1,x/x2,2,x,1,1 1 13",
    );
}

pub fn evaluate_start_1(c: &mut Criterion) {
    bench_position(c, 6, "start_1", "x6/x2,2,2,x2/x6/x6/x6/1,x3,1,x 1 3");
}

pub fn evaluate_start_2(c: &mut Criterion) {
    bench_position(c, 6, "start_2", "2,x5/x6/x6/x6/x2,1,x3/1,x5 2 2");
}

pub fn evaluate_start_3(c: &mut Criterion) {
    bench_position(
        c,
        6,
        "start_3",
        "x6/x4,2,1/x2,2,2C,1,2/x2,2,x,1,1/x5,1/x6 1 6",
    );
}

pub fn evaluate_start_4(c: &mut Criterion) {
    bench_position(
        c,
        6,
        "start_4",
        "2,x4,1/x4,1,1/x2,2,21C,12C,x/1,1,1,2,1,1/x2,2,2,2,2/x6 2 11",
    );
}

pub fn evaluate_midgame_1(c: &mut Criterion) {
    bench_position(c, 6, "midgame_1", "2,2,2222221C,x3/2,2,2S,12121S,x,2/2,2,1,1,1,1/x,1S,111112C,1,1,x/1,12112S,x4/x,2,x3,1 1 31");
}

pub fn evaluate_midgame_2(c: &mut Criterion) {
    bench_position(c, 6, "midgame_2", "x4,1,1/1,12S,2,2,1,1/1,1221S,1,21C,1,x/1,21112C,2,1,22221S,2/2,2,2,2S,1,2/x2,21,21,x,2 1 32");
}

pub fn evaluate_midgame_3(c: &mut Criterion) {
    bench_position(
        c,
        6,
        "midgame_3",
        "2,1,1,1,x2/x,2,2,1,x2/x,1,2,1C,1,1/x2,2,1112C,12S,2/x,2,2,1,x,1/2,2,x2,1,1 1 17",
    );
}

pub fn evaluate_endgame_1(c: &mut Criterion) {
    bench_position(c, 6, "endgame_1", "1,2,1,1,1S,212212/112,22,x,1,1S,2/1,2,212C,2,1112S,x/x,2,1,1,12221C,2/1,1S,1S,12,x,22121S/1,12,1,2,x,2 1 46");
}

pub fn evaluate_endgame_2(c: &mut Criterion) {
    bench_position(c, 6, "endgame_2", "2,x,2,2,1,1/1,2,2,1,12,1/1,2112S,x,1,2,1/21,2,2221S,2,2112C,2/2,121,1,2S,11221C,1/12,222221S,12,1,1,1 1 43");
}

pub fn evaluate_endgame_3(c: &mut Criterion) {
    bench_position(c, 6, "endgame_3", "x2,21,122,1121S,112S/1S,x,1112,x,2S,x/112C,2S,x,1222221C,2,x/2,x2,1,2121S,x/112,1112111112S,x3,221S/2,2,x2,21,2 1 56");
}
