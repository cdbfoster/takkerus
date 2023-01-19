use std::sync::{Mutex, MutexGuard};

use once_cell::sync::Lazy;
use rand::Rng;

use tak::{Color, Direction, PieceType, Ply, State};

use crate::evaluation::placement_threat_maps;
use crate::rng::JKiss32Rng;

pub(crate) struct PlyGenerator<const N: usize> {
    state: State<N>,
    previous_principal: Option<Ply<N>>,
    tt_ply: Option<Ply<N>>,
    plies: Vec<ScoredPly<N>>,
    operation: Operation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Fallibility {
    Fallible,
    Infallible,
}

impl<const N: usize> PlyGenerator<N> {
    pub(crate) fn new(
        state: &State<N>,
        previous_principal: Option<Ply<N>>,
        tt_ply: Option<Ply<N>>,
    ) -> Self {
        Self {
            state: state.clone(),
            previous_principal,
            tt_ply,
            plies: Vec::new(),
            operation: Operation::PreviousPrincipal,
        }
    }
}

impl<const N: usize> Iterator for PlyGenerator<N> {
    type Item = (Fallibility, Ply<N>);

    fn next(&mut self) -> Option<Self::Item> {
        use Fallibility::*;
        use Operation::*;

        if self.operation == PreviousPrincipal {
            self.operation = self.operation.next();

            if self.previous_principal.is_some() {
                return self.previous_principal.map(|p| (Fallible, p));
            }
        }

        if self.operation == TtPly {
            self.operation = self.operation.next();

            if self.tt_ply.is_some() && self.tt_ply != self.previous_principal {
                return self.tt_ply.map(|p| (Fallible, p));
            }
        }

        if self.operation == GeneratePlies {
            self.operation = self.operation.next();

            generate_plies(&self.state, &mut self.plies);

            // Score plies for sorting ==========

            let m = &self.state.metadata;

            let all_pieces = m.p1_pieces | m.p2_pieces;
            let road_pieces = m.flatstones | m.capstones;

            let player_road_pieces = match self.state.to_move() {
                Color::White => road_pieces & m.p1_pieces,
                Color::Black => road_pieces & m.p2_pieces,
            };

            for scored_ply in &mut self.plies {
                // Search placements before spreads.
                if let ScoredPly {
                    score,
                    ply: Ply::Place { x, y, .. },
                } = scored_ply
                {
                    *score += 1 << 8;

                    // Search placements that would cause a threat first.
                    let mut placed_map = player_road_pieces;
                    placed_map.set(*x as usize, *y as usize);

                    let threat_map = {
                        let (horizontal, vertical) =
                            placement_threat_maps(all_pieces, player_road_pieces);
                        horizontal | vertical
                    };

                    if *threat_map > 0 {
                        *score += 1 << 8;
                    }
                }
            }

            // Shuffle plies randomly so we don't always play the same move if
            // two moves eval the same. Any merit-based scoring is much bigger
            // than this, so this random bonus will only affect order between
            // otherwise equally important plies.
            let mut rng = get_rng();
            for scored_ply in &mut self.plies {
                scored_ply.score += rng.gen::<u8>() as u32;
            }

            self.plies.sort_unstable_by_key(|ply| ply.score);
        }

        if self.operation == AllPlies {
            if let Some(ScoredPly { ply, .. }) = self.plies.pop() {
                if Some(ply) == self.previous_principal {
                    return self.next();
                }

                if Some(ply) == self.tt_ply {
                    return self.next();
                }

                return Some((Infallible, ply));
            } else {
                self.operation = self.operation.next();
            }
        }

        None
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Eq, PartialEq)]
enum Operation {
    PreviousPrincipal = 0u32,
    TtPly,
    GeneratePlies,
    AllPlies,
    Finished,
}

impl Operation {
    fn next(&self) -> Self {
        (*self as u32 + 1).into()
    }
}

impl From<u32> for Operation {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::PreviousPrincipal,
            1 => Self::TtPly,
            2 => Self::GeneratePlies,
            3 => Self::AllPlies,
            _ => Self::Finished,
        }
    }
}

static DROP_COMBOS: Lazy<Vec<Vec<Vec<u8>>>> = Lazy::new(|| generate_drop_combos(8));

/// Generates lists of drop combinations, indexed [stack size][combo][drops]
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

struct ScoredPly<const N: usize> {
    score: u32,
    ply: Ply<N>,
}

impl<const N: usize> From<Ply<N>> for ScoredPly<N> {
    fn from(ply: Ply<N>) -> Self {
        Self { score: 0, ply }
    }
}

fn generate_plies<const N: usize>(state: &State<N>, ply_buffer: &mut Vec<ScoredPly<N>>) {
    use Color::*;
    use PieceType::*;

    let next_color = if state.ply_count % 2 == 0 {
        White
    } else {
        Black
    };

    if state.ply_count >= 2 {
        let next_capstones = match next_color {
            White => state.p1_capstones,
            Black => state.p2_capstones,
        };

        for x in 0..N {
            for y in 0..N {
                if state.board[x][y].is_empty() {
                    ply_buffer.push(
                        Ply::Place {
                            x: x as u8,
                            y: y as u8,
                            piece_type: Flatstone,
                        }
                        .into(),
                    );
                    ply_buffer.push(
                        Ply::Place {
                            x: x as u8,
                            y: y as u8,
                            piece_type: StandingStone,
                        }
                        .into(),
                    );
                    if next_capstones > 0 {
                        ply_buffer.push(
                            Ply::Place {
                                x: x as u8,
                                y: y as u8,
                                piece_type: Capstone,
                            }
                            .into(),
                        );
                    }
                } else {
                    let stack = &state.board[x][y];
                    let top_piece = stack.last().unwrap();

                    if top_piece.color() == next_color {
                        for direction in [
                            Direction::North,
                            Direction::East,
                            Direction::South,
                            Direction::West,
                        ] {
                            let (dx, dy) = direction.to_offset();
                            let (mut tx, mut ty) = (x as i8, y as i8);
                            let mut distance = 0;

                            // Cast until the edge of the board or until (and including) a blocking piece.
                            loop {
                                tx += dx;
                                ty += dy;
                                if tx < 0 || tx >= N as i8 || ty < 0 || ty >= N as i8 {
                                    break;
                                }

                                distance += 1;
                                let target_type =
                                    state.board[tx as usize][ty as usize].last_piece_type();

                                if matches!(target_type, Some(StandingStone | Capstone)) {
                                    break;
                                }
                            }

                            let pickup_size = N.min(stack.len());
                            let drop_combos = DROP_COMBOS[..=pickup_size]
                                .iter()
                                .flatten()
                                .filter(|combo| combo.len() <= distance)
                                .filter_map(|combo| {
                                    let tx = x as i8 + combo.len() as i8 * dx;
                                    let ty = y as i8 + combo.len() as i8 * dy;
                                    let target_type =
                                        state.board[tx as usize][ty as usize].last_piece_type();

                                    // Allow this drop combo if the target is a flatstone or empty.
                                    let unblocked =
                                        target_type.is_none() || target_type == Some(Flatstone);

                                    // Allow this drop combo if the target is a standing stone, and we're
                                    // dropping a capstone by itself onto it.
                                    let crush = target_type == Some(StandingStone)
                                        && top_piece.piece_type() == Capstone
                                        && *combo.last().unwrap() == 1;

                                    (unblocked || crush).then_some((combo, crush))
                                });

                            for (drop_combo, crush) in drop_combos {
                                let mut drops = [0; N];
                                drops[..drop_combo.len()].copy_from_slice(drop_combo);

                                ply_buffer.push(
                                    Ply::Spread {
                                        x: x as u8,
                                        y: y as u8,
                                        direction,
                                        drops,
                                        crush,
                                    }
                                    .into(),
                                );
                            }
                        }
                    }
                }
            }
        }
    } else {
        for x in 0..N {
            for y in 0..N {
                if state.board[x][y].is_empty() {
                    ply_buffer.push(
                        Ply::Place {
                            x: x as u8,
                            y: y as u8,
                            piece_type: Flatstone,
                        }
                        .into(),
                    );
                }
            }
        }
    }
}

fn get_rng() -> MutexGuard<'static, JKiss32Rng> {
    static RNG: Lazy<Mutex<JKiss32Rng>> = Lazy::new(|| Default::default());
    RNG.lock().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_plies() {
        let state: State<5> = State::default();

        let mut plies = Vec::new();
        generate_plies(&state, &mut plies);

        for ScoredPly { ply, .. } in plies {
            let validated_ply = state.validate_ply(ply);
            assert_eq!(Ok(ply), validated_ply);
        }

        let state: State<5> = "x5/x,1S,x2,1C/x4,1/x,2,2C,x,2/x5 1 4".parse().unwrap();

        let mut plies = Vec::new();
        generate_plies(&state, &mut plies);

        for ScoredPly { ply, .. } in plies {
            let validated_ply = state.validate_ply(ply);
            assert_eq!(Ok(ply), validated_ply);
        }
    }
}
