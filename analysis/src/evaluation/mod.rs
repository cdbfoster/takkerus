use tak::{Color, Resolution, State};

pub use self::util::Evaluation;

pub(crate) use self::util::placement_threat_map;

use self::scoring::*;

mod scoring;
mod util;

pub fn evaluate<const N: usize>(state: &State<N>, start_ply: u16) -> Evaluation {
    use Color::*;

    match state.resolution() {
        None => (),
        Some(Resolution::Road(color)) | Some(Resolution::Flats { color, .. }) => {
            if color == state.to_move() {
                return Evaluation::WIN - state.ply_count as i32;
            } else {
                return Evaluation::LOSE + state.ply_count as i32;
            }
        }
        Some(Resolution::Draw) => return Evaluation::ZERO - state.ply_count as i32,
    }

    let m = &state.metadata;

    let mut p1_eval = Evaluation::ZERO;
    let mut p2_eval = Evaluation::ZERO;

    let all_pieces = m.p1_pieces & m.p2_pieces;

    let road_pieces = m.flatstones | m.capstones;
    let p1_road_pieces = road_pieces & m.p1_pieces;
    let p2_road_pieces = road_pieces & m.p2_pieces;

    let blocking_pieces = m.standing_stones | m.capstones;
    let p1_blocking_pieces = blocking_pieces & m.p1_pieces;
    let p2_blocking_pieces = blocking_pieces & m.p1_pieces;

    // Material
    p1_eval += evaluate_material(m, m.p1_pieces);
    p2_eval += evaluate_material(m, m.p2_pieces);

    // Blocker locations
    p1_eval += evaluate_blocker_locations(p1_blocking_pieces);
    p2_eval += evaluate_blocker_locations(p2_blocking_pieces);

    // Road groups
    p1_eval += evaluate_road_groups(p1_road_pieces);
    p2_eval += evaluate_road_groups(p2_road_pieces);

    // Road slices
    p1_eval += evaluate_road_slices(p1_road_pieces);
    p2_eval += evaluate_road_slices(p2_road_pieces);

    // Captured flats
    p1_eval += evaluate_captured_flats(m.p1_pieces, &m.p1_stacks, &m.p2_stacks);
    p2_eval += evaluate_captured_flats(m.p2_pieces, &m.p2_stacks, &m.p1_stacks);

    // Placement threats
    p1_eval += evaluate_placement_threats(p1_road_pieces, all_pieces & !p1_road_pieces);
    p2_eval += evaluate_placement_threats(p2_road_pieces, all_pieces & !p2_road_pieces);

    // Tempo offset
    match state.to_move() {
        White => p1_eval += tempo_offset::<N>(start_ply, state.ply_count),
        Black => p2_eval += tempo_offset::<N>(start_ply, state.ply_count),
    }

    match state.to_move() {
        White => p1_eval - p2_eval,
        Black => p2_eval - p1_eval,
    }
}
