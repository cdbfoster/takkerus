use once_cell::sync::Lazy;

use tak::{Bitmap, Direction, Drops, PieceType, Ply, State};

pub fn placements<const N: usize>(
    locations: Bitmap<N>,
    piece_type: PieceType,
) -> impl Iterator<Item = Ply<N>> {
    locations
        .bits()
        .map(|b| b.coordinates())
        .map(move |(x, y)| Ply::Place {
            x: x as u8,
            y: y as u8,
            piece_type,
        })
}

pub fn spreads<const N: usize>(
    state: &State<N>,
    locations: Bitmap<N>,
) -> impl Iterator<Item = Ply<N>> + '_ {
    use PieceType::*;

    locations
        .bits()
        .map(|b| b.coordinates())
        .flat_map(move |(x, y)| {
            let stack = &state.board[x][y];
            let top_piece = stack.top().unwrap();

            [
                Direction::North,
                Direction::East,
                Direction::South,
                Direction::West,
            ]
            .into_iter()
            .flat_map(move |direction| {
                let (dx, dy) = direction.to_offset();
                let (mut tx, mut ty) = (x as i8, y as i8);
                let mut distance = 0;

                let pickup_size = N.min(stack.len());

                // Cast until the edge of the board or until (and including) a blocking piece.
                for _ in 0..pickup_size {
                    tx += dx;
                    ty += dy;
                    if tx < 0 || tx >= N as i8 || ty < 0 || ty >= N as i8 {
                        break;
                    }

                    distance += 1;
                    let target_type = state.board[tx as usize][ty as usize].top_piece_type();

                    if matches!(target_type, Some(StandingStone | Capstone)) {
                        break;
                    }
                }

                DROP_COMBOS[..=pickup_size]
                    .iter()
                    .flatten()
                    .filter(move |combo| combo.len() <= distance)
                    .filter_map(move |combo| {
                        let tx = x as i8 + combo.len() as i8 * dx;
                        let ty = y as i8 + combo.len() as i8 * dy;
                        let target_type = state.board[tx as usize][ty as usize].top_piece_type();

                        // Allow this drop combo if the target is a flatstone or empty.
                        let unblocked = target_type.is_none() || target_type == Some(Flatstone);

                        // Allow this drop combo if the target is a standing stone, and we're
                        // dropping a capstone by itself onto it.
                        let crush = target_type == Some(StandingStone)
                            && top_piece.piece_type() == Capstone
                            && *combo.last().unwrap() == 1;

                        (unblocked || crush).then_some((combo, crush))
                    })
                    .map(move |(combo, crush)| {
                        let drops = Drops::new::<N>(&combo).unwrap();

                        Ply::Spread {
                            x: x as u8,
                            y: y as u8,
                            direction,
                            drops,
                            crush,
                        }
                    })
            })
        })
}

static DROP_COMBOS: Lazy<Vec<Vec<Vec<u8>>>> = Lazy::new(|| generate_drop_combos(8));

/// Generates lists of drop combinations, indexed by [stack size][which combo][drops per tile].
fn generate_drop_combos(max_size: usize) -> Vec<Vec<Vec<u8>>> {
    let mut combos_for_size = Vec::with_capacity(max_size + 1);

    // 0 stones, 0 drops.
    combos_for_size.push(Vec::new());

    for current_size in 1..=max_size {
        // For any stack size, there's the option of dropping everything on the first square.
        let full_drop = std::iter::once(vec![current_size as u8]);

        // Iterate over every previous drop combo, subtracting the total from this stack size.
        let other_combos = combos_for_size[..current_size]
            .iter()
            .flat_map(|stack_combos| stack_combos.iter())
            .map(|combo: &Vec<u8>| {
                let mut new_combo = Vec::with_capacity(combo.len() + 1);
                new_combo.push(current_size as u8 - combo.iter().sum::<u8>());
                new_combo.extend_from_slice(combo);
                new_combo
            });

        combos_for_size.push(full_drop.chain(other_combos).collect());
    }

    combos_for_size
}
