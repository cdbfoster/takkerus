use crate::bitmap::Bitmap;
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
            hash: 0,
        }
    }
}

impl<const N: usize> Metadata<N> {
    pub(crate) fn set_stack(&mut self, stack: &Stack, x: usize, y: usize) {
        if let Some(piece) = stack.top() {
            if piece.color() == Color::White {
                self.p1_pieces.set(x, y);
                self.p2_pieces.clear(x, y);
            } else {
                self.p1_pieces.clear(x, y);
                self.p2_pieces.set(x, y);
            }

            match piece.piece_type() {
                PieceType::Flatstone => {
                    self.flatstones.set(x, y);
                    self.standing_stones.clear(x, y);
                    self.capstones.clear(x, y);
                }
                PieceType::StandingStone => {
                    self.flatstones.clear(x, y);
                    self.standing_stones.set(x, y);
                    self.capstones.clear(x, y);
                }
                PieceType::Capstone => {
                    self.flatstones.clear(x, y);
                    self.standing_stones.clear(x, y);
                    self.capstones.set(x, y);
                }
            }
        } else {
            self.p1_pieces.clear(x, y);
            self.p2_pieces.clear(x, y);
            self.flatstones.clear(x, y);
            self.standing_stones.clear(x, y);
            self.capstones.clear(x, y);
        }
    }

    pub(crate) fn place_piece(&mut self, piece: Piece, x: usize, y: usize) {
        if piece.color() == Color::White {
            self.p1_pieces.set(x, y);
        } else {
            self.p2_pieces.set(x, y);
        }

        match piece.piece_type() {
            PieceType::Flatstone => {
                self.flatstones.set(x, y);
            }
            PieceType::StandingStone => self.standing_stones.set(x, y),
            PieceType::Capstone => self.capstones.set(x, y),
        }
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
