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
    pub p1_flat_count: u8,
    pub p2_flat_count: u8,
    pub p1_stacks: [[u8; N]; N],
    pub p2_stacks: [[u8; N]; N],
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
            p1_flat_count: 0,
            p2_flat_count: 0,
            p1_stacks: [[0; N]; N],
            p2_stacks: [[0; N]; N],
            hash: 0,
        }
    }
}

impl<const N: usize> Metadata<N> {
    pub(crate) fn set_stack(&mut self, stack: &Stack, x: usize, y: usize) {
        if (self.flatstones & self.p1_pieces).get(x, y) {
            self.p1_flat_count -= 1;
        } else if (self.flatstones & self.p2_pieces).get(x, y) {
            self.p2_flat_count -= 1;
        }

        if let Some(piece) = stack.last() {
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
                    match piece.color() {
                        Color::White => self.p1_flat_count += 1,
                        Color::Black => self.p2_flat_count += 1,
                    }
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

        let (p1_stack, p2_stack) = stack.get_player_pieces();
        self.p1_stacks[x][y] = p1_stack;
        self.p2_stacks[x][y] = p2_stack;
    }

    pub(crate) fn place_piece(&mut self, piece: Piece, x: usize, y: usize) {
        if piece.color() == Color::White {
            self.p1_pieces.set(x, y);
            self.p1_stacks[x][y] = 1;
        } else {
            self.p2_pieces.set(x, y);
            self.p2_stacks[x][y] = 1;
        }

        match piece.piece_type() {
            PieceType::Flatstone => {
                self.flatstones.set(x, y);
                match piece.color() {
                    Color::White => self.p1_flat_count += 1,
                    Color::Black => self.p2_flat_count += 1,
                }
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
