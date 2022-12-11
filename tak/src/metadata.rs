use crate::bitmap::Bitmap;
use crate::piece::{Color, Piece, PieceType};
use crate::stack::Stack;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Metadata<const N: usize> {
    pub p1_pieces: Bitmap<N>,
    pub p2_pieces: Bitmap<N>,
    pub flatstones: Bitmap<N>,
    pub standing_stones: Bitmap<N>,
    pub capstones: Bitmap<N>,
}

impl<const N: usize> Metadata<N> {
    pub(crate) fn set_stack(&mut self, stack: &Stack, x: usize, y: usize) {
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
        match piece.color() {
            Color::White => self.p1_pieces.set(x, y),
            Color::Black => self.p2_pieces.set(x, y),
        }

        match piece.piece_type() {
            PieceType::Flatstone => self.flatstones.set(x, y),
            PieceType::StandingStone => self.standing_stones.set(x, y),
            PieceType::Capstone => self.capstones.set(x, y),
        }
    }
}
