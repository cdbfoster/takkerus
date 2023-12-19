use crate::bitmap::{edge_masks, Bitmap};
use crate::piece::{Color, Piece, PieceType};
use crate::stack::Stack;
use crate::zobrist::ZobristHash;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Metadata<const N: usize> {
    pub p1_pieces: Bitmap<N>,
    pub p2_pieces: Bitmap<N>,
    pub flatstones: Bitmap<N>,
    pub standing_stones: Bitmap<N>,
    pub capstones: Bitmap<N>,
    pub p1_edge_groups: [Bitmap<N>; 4],
    pub p2_edge_groups: [Bitmap<N>; 4],
    pub hash: ZobristHash,
}

impl<const N: usize> Default for Metadata<N> {
    fn default() -> Self {
        Self {
            p1_pieces: Bitmap::empty(),
            p2_pieces: Bitmap::empty(),
            flatstones: Bitmap::empty(),
            standing_stones: Bitmap::empty(),
            capstones: Bitmap::empty(),
            p1_edge_groups: [Bitmap::empty(); 4],
            p2_edge_groups: [Bitmap::empty(); 4],
            hash: 0,
        }
    }
}

impl<const N: usize> Metadata<N> {
    pub(crate) fn get_modifier(&mut self) -> MetadataModifier<'_, N> {
        let p1_expanded_edge_groups = expand_edge_groups(&self.p1_edge_groups);
        let p2_expanded_edge_groups = expand_edge_groups(&self.p2_edge_groups);

        MetadataModifier {
            metadata: self,
            p1_expanded_edge_groups,
            p2_expanded_edge_groups,
        }
    }
}

pub(crate) struct MetadataModifier<'a, const N: usize> {
    pub(crate) metadata: &'a mut Metadata<N>,
    pub(crate) p1_expanded_edge_groups: [Bitmap<N>; 4],
    pub(crate) p2_expanded_edge_groups: [Bitmap<N>; 4],
}

impl<'a, const N: usize> MetadataModifier<'a, N> {
    pub(crate) fn set_stack(&mut self, stack: Stack, x: usize, y: usize) {
        let m = &mut self.metadata;

        if let Some(piece) = stack.top() {
            if piece.color() == Color::White {
                m.p1_pieces.set(x, y);
                m.p2_pieces.clear(x, y);
                set_on_edges(&mut m.p1_edge_groups, &self.p1_expanded_edge_groups, x, y);
                clear_from_edges(&mut m.p2_edge_groups, x, y);
            } else {
                m.p1_pieces.clear(x, y);
                m.p2_pieces.set(x, y);
                set_on_edges(&mut m.p2_edge_groups, &self.p2_expanded_edge_groups, x, y);
                clear_from_edges(&mut m.p1_edge_groups, x, y);
            }

            match piece.piece_type() {
                PieceType::Flatstone => {
                    m.flatstones.set(x, y);
                    m.standing_stones.clear(x, y);
                    m.capstones.clear(x, y);
                }
                PieceType::StandingStone => {
                    m.flatstones.clear(x, y);
                    m.standing_stones.set(x, y);
                    m.capstones.clear(x, y);
                }
                PieceType::Capstone => {
                    m.flatstones.clear(x, y);
                    m.standing_stones.clear(x, y);
                    m.capstones.set(x, y);
                }
            }
        } else {
            m.p1_pieces.clear(x, y);
            m.p2_pieces.clear(x, y);
            m.flatstones.clear(x, y);
            m.standing_stones.clear(x, y);
            m.capstones.clear(x, y);
            clear_from_edges(&mut self.p1_expanded_edge_groups, x, y);
            clear_from_edges(&mut self.p2_expanded_edge_groups, x, y);
        }
    }

    pub(crate) fn place_piece(&mut self, piece: Piece, x: usize, y: usize) {
        let m = &mut self.metadata;

        let (pieces, edge_groups, expanded_edge_groups) = match piece.color() {
            Color::White => (
                &mut m.p1_pieces,
                &mut m.p1_edge_groups,
                &self.p1_expanded_edge_groups,
            ),
            Color::Black => (
                &mut m.p2_pieces,
                &mut m.p2_edge_groups,
                &self.p2_expanded_edge_groups,
            ),
        };

        pieces.set(x, y);
        set_on_edges(edge_groups, expanded_edge_groups, x, y);

        if piece.color() == Color::White {
            m.p1_pieces.set(x, y);
        } else {
            m.p2_pieces.set(x, y);
        }

        match piece.piece_type() {
            PieceType::Flatstone => {
                m.flatstones.set(x, y);
            }
            PieceType::StandingStone => m.standing_stones.set(x, y),
            PieceType::Capstone => m.capstones.set(x, y),
        }
    }
}

pub(crate) fn expand_edge_groups<const N: usize>(edge_groups: &[Bitmap<N>; 4]) -> [Bitmap<N>; 4] {
    let mut expanded_groups = edge_groups.map(|group| group.dilate());
    for (group, edge) in expanded_groups.iter_mut().zip(edge_masks::<N>()) {
        *group |= edge;
    }
    expanded_groups
}

fn set_on_edges<const N: usize>(
    edge_groups: &mut [Bitmap<N>; 4],
    expanded_edge_groups: &[Bitmap<N>; 4],
    x: usize,
    y: usize,
) {
    for (connected_to_edge, edge_map) in expanded_edge_groups.iter().zip(edge_groups) {
        if connected_to_edge.get(x, y) {
            edge_map.set(x, y);
        }
    }
}

fn clear_from_edges<const N: usize>(edge_groups: &mut [Bitmap<N>; 4], x: usize, y: usize) {
    for group in edge_groups {
        group.clear(x, y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_size() {
        for n in 3..=8 {
            let (size, alignment) = match n {
                3 => (
                    std::mem::size_of::<Metadata<3>>(),
                    std::mem::align_of::<Metadata<3>>(),
                ),
                4 => (
                    std::mem::size_of::<Metadata<4>>(),
                    std::mem::align_of::<Metadata<4>>(),
                ),
                5 => (
                    std::mem::size_of::<Metadata<5>>(),
                    std::mem::align_of::<Metadata<5>>(),
                ),
                6 => (
                    std::mem::size_of::<Metadata<6>>(),
                    std::mem::align_of::<Metadata<6>>(),
                ),
                7 => (
                    std::mem::size_of::<Metadata<7>>(),
                    std::mem::align_of::<Metadata<7>>(),
                ),
                8 => (
                    std::mem::size_of::<Metadata<8>>(),
                    std::mem::align_of::<Metadata<8>>(),
                ),
                _ => unreachable!(),
            };

            println!("Metadata<{n}>: {size} bytes, {alignment} byte alignment");
        }
    }
}
