use tak::{board_mask, Color, PieceType, Ply, State};

use crate::util::{placement_threat_map, FixedLifoBuffer};

use super::generation;
use super::Continuation::{self, *};
use super::Fallibility::{self, *};
use super::PlyBuffer;

pub(super) struct GeneratedPly<const N: usize> {
    pub(super) ply: Ply<N>,
    pub(super) fallibility: Fallibility,
    pub(super) continuation: Continuation,
}

// Placement wins ===============================

pub(super) struct PlacementWins<'a, const N: usize> {
    pub(super) state: &'a State<N>,
}

impl<'a, const N: usize> Iterator for PlacementWins<'a, N> {
    type Item = GeneratedPly<N>;

    fn next(&mut self) -> Option<Self::Item> {
        let m = &self.state.metadata;

        let all_pieces = m.p1_pieces | m.p2_pieces;
        let road_pieces = m.flatstones | m.capstones;

        let player_road_pieces = match self.state.to_move() {
            Color::White => road_pieces & m.p1_pieces,
            Color::Black => road_pieces & m.p2_pieces,
        };
        let blocking_pieces = all_pieces & !player_road_pieces;

        let threat_map = placement_threat_map(player_road_pieces, blocking_pieces);

        generation::placements(threat_map, PieceType::Flatstone)
            .map(|ply| GeneratedPly {
                ply,
                fallibility: Infallible,
                continuation: Stop,
            })
            .next()
    }
}

// Transposition table ply =====================

pub(super) struct TtPly<'a, const N: usize> {
    pub(super) used_plies: &'a PlyBuffer<N>,
    pub(super) ply: Option<Ply<N>>,
}

impl<'a, const N: usize> Iterator for TtPly<'a, N> {
    type Item = GeneratedPly<N>;

    fn next(&mut self) -> Option<Self::Item> {
        self.ply
            .filter(|ply| !self.used_plies.borrow().contains(ply))
            .map(|ply| GeneratedPly {
                ply,
                fallibility: Fallible,
                continuation: Continue,
            })
    }
}

// Killer moves =================================

pub(crate) type KillerMoves<const N: usize> = FixedLifoBuffer<2, Ply<N>>;

pub(super) struct Killers<'a, const N: usize> {
    pub(super) used_plies: &'a PlyBuffer<N>,
    pub(super) killer_moves: &'a mut KillerMoves<N>,
}

impl<'a, const N: usize> Iterator for Killers<'a, N> {
    type Item = GeneratedPly<N>;

    fn next(&mut self) -> Option<Self::Item> {
        self.killer_moves
            .pop()
            .filter(|ply| !self.used_plies.borrow().contains(ply))
            .map(|ply| GeneratedPly {
                ply,
                fallibility: Fallible,
                continuation: Continue,
            })
    }
}

// All plies ====================================

pub(super) struct AllPlies<'a, const N: usize> {
    pub(super) used_plies: &'a PlyBuffer<N>,
    pub(super) state: &'a State<N>,
    plies: Option<Vec<ScoredPly<N>>>,
    fetches: usize,
}

impl<'a, const N: usize> AllPlies<'a, N> {
    pub(super) fn new(used_plies: &'a PlyBuffer<N>, state: &'a State<N>) -> Self {
        Self {
            used_plies,
            state,
            plies: None,
            fetches: 0,
        }
    }
}

pub(super) struct ScoredPly<const N: usize> {
    score: u32,
    ply: Ply<N>,
}

impl<'a, const N: usize> Iterator for AllPlies<'a, N> {
    type Item = GeneratedPly<N>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(plies) = &mut self.plies {
            let scored_ply = if self.fetches < 5 {
                self.fetches += 1;

                let best_ply_index = plies
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, scored_ply)| scored_ply.score)
                    .map(|(i, _)| i);

                best_ply_index.map(|i| plies.swap_remove(i))
            } else {
                // If we're more than 5 plies into searching this node, just rely on our weak ordering.
                plies.pop()
            };

            scored_ply.map(|scored_ply| GeneratedPly {
                ply: scored_ply.ply,
                fallibility: Infallible,
                continuation: Continue,
            })
        } else {
            let empty =
                board_mask() ^ self.state.metadata.p1_pieces ^ self.state.metadata.p2_pieces;

            let mut plies = Vec::new();
            let used_plies = self.used_plies.borrow();

            // Generate all available plies, with a weak ordering;
            // since we'll pop off the end, start with standing stones,
            // then spreads, then capstones, then flatstones.

            if self.state.ply_count >= 2 {
                let player_stacks = match self.state.to_move() {
                    Color::White => self.state.metadata.p1_pieces,
                    Color::Black => self.state.metadata.p2_pieces,
                };

                // Standing stones.
                plies.extend(
                    generation::placements(empty, PieceType::StandingStone)
                        .filter(|ply| !used_plies.contains(ply))
                        .map(|ply| ScoredPly { score: 0, ply }),
                );

                // Spreads.
                plies.extend(
                    generation::spreads(self.state, player_stacks)
                        .filter(|ply| !used_plies.contains(ply))
                        .map(|ply| ScoredPly { score: 1, ply }),
                );

                let reserve_capstones = match self.state.to_move() {
                    Color::White => self.state.p1_capstones,
                    Color::Black => self.state.p2_capstones,
                };

                // Capstones.
                if reserve_capstones > 0 {
                    plies.extend(
                        generation::placements(empty, PieceType::Capstone)
                            .filter(|ply| !used_plies.contains(ply))
                            .map(|ply| ScoredPly { score: 2, ply }),
                    );
                }
            }

            // Flatstones.
            plies.extend(
                generation::placements(empty, PieceType::Flatstone)
                    .filter(|ply| !used_plies.contains(ply))
                    .map(|ply| ScoredPly { score: 3, ply }),
            );

            // Add a bonus to any placements that threaten a road.
            // Start at the back of the plies list, since that's where the
            // road-contributing placements are.
            if self.state.ply_count >= 6 {
                let m = &self.state.metadata;

                let all_pieces = m.p1_pieces | m.p2_pieces;
                let road_pieces = m.flatstones | m.capstones;

                let player_road_pieces = match self.state.to_move() {
                    Color::White => road_pieces & m.p1_pieces,
                    Color::Black => road_pieces & m.p2_pieces,
                };
                let blocking_pieces = all_pieces & !player_road_pieces;

                for scored_ply in plies.iter_mut().rev() {
                    if let ScoredPly {
                        score,
                        ply: Ply::Place { x, y, .. },
                    } = scored_ply
                    {
                        let mut placed_map = player_road_pieces;
                        placed_map.set(*x as usize, *y as usize);

                        let threat_map = placement_threat_map(player_road_pieces, blocking_pieces);

                        if threat_map != 0.into() {
                            *score += 10;
                        }
                    } else {
                        break;
                    }
                }
            }

            self.plies = Some(plies);
            self.next()
        }
    }
}
