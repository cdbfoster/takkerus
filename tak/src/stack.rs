use std::fmt;

use crate::piece::{Color, Piece, PieceType};

use Color::*;
use PieceType::*;

#[cfg(not(feature = "deep-stacks"))]
type Bitmap = u32;

#[cfg(feature = "deep-stacks")]
type Bitmap = u128;

const MAX_STACK_HEIGHT: usize = Bitmap::BITS as usize - 4;

/// Representation:
///
///  Leading 1   Piece colors   Piece type
///         | ┌-----┴---------┐ ┌-┴-┐
/// MSB - … 1 x … … … … … … … x t t t - LSB
///
/// The least significant piece color bit represents the top of the stack,
/// and the most significant represents the bottom.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Stack(Bitmap);

impl Default for Stack {
    fn default() -> Self {
        Self(0b1000)
    }
}

impl Stack {
    pub fn from_piece(piece: Piece) -> Self {
        Self(0b10000 | (piece.piece_type() as Bitmap >> 4) | ((piece.color() as Bitmap - 1) << 3))
    }

    pub fn len(&self) -> usize {
        MAX_STACK_HEIGHT - self.0.leading_zeros() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0b1000
    }

    pub fn top(&self) -> Option<Piece> {
        if self.is_empty() {
            None
        } else {
            self.get(0)
        }
    }

    pub fn top_piece_type(&self) -> Option<PieceType> {
        self.top().map(|p| p.piece_type())
    }

    pub fn top_color(&self) -> Option<Color> {
        self.top().map(|p| p.color())
    }

    /// Returns the piece at the given position on the stack, indexed top to bottom.
    pub fn get(&self, index: usize) -> Option<Piece> {
        if index >= self.len() {
            return None;
        }

        let piece_type = if index == 0 {
            (((self.0 & 0x07) as u8) << 4).try_into().unwrap()
        } else {
            Flatstone
        };

        if (self.0 >> (index + 3)) & 0x01 == 0 {
            Some(Piece::new(piece_type, White))
        } else {
            Some(Piece::new(piece_type, Black))
        }
    }

    /// Adds a stack to the top of the stack.
    pub fn add(&mut self, stack: Self) {
        assert!(
            self.len() + stack.len() <= MAX_STACK_HEIGHT,
            "exceeded stack limit, compile with \"deep-stacks\" feature to support this"
        );
        if !stack.is_empty() {
            let new_pieces = stack.0 ^ (0x01 << (stack.len() + 3));
            self.0 = (self.0 & !0x07) << stack.len() | new_pieces;
        }
    }

    /// Adds a piece to the top of the stack.
    pub fn add_piece(&mut self, piece: Piece) {
        self.add(Stack::from_piece(piece))
    }

    /// Takes the top `count` pieces off of the stack.
    pub fn take(&mut self, count: usize) -> Self {
        debug_assert!(count > 0 && count <= self.len());

        let carry = self.0 & !(Bitmap::MAX << (count + 3)) | (0x01 << (count + 3));

        self.0 = (self.0 >> count) & !0x07;

        if !self.is_empty() {
            self.0 |= 0b001;
        }

        Self(carry)
    }

    /// Drops the bottom `count` pieces from the stack.
    pub fn drop(&mut self, count: usize) -> Self {
        debug_assert!(count > 0 && count <= self.len());

        if count < self.len() {
            let remainder = self.take(self.len() - count);
            let drop = *self;
            *self = remainder;
            drop
        } else {
            let drop = *self;
            *self = Self::default();
            drop
        }
    }

    /// Returns an iterator over the pieces in the stack from top to bottom.
    pub fn iter(&self) -> StackIter<'_> {
        StackIter {
            stack: self,
            i_top: 0,
            i_bottom: self.len(),
        }
    }

    /// Returns the positions of each color's pieces in the stack. The first
    /// value is white's piece map, the second is black's. A 1 is a piece of
    /// that color, a 0 could be the opponent's piece or an empty space.
    pub fn get_player_pieces(&self) -> (Bitmap, Bitmap) {
        if !self.is_empty() {
            let mask = !(Bitmap::MAX << self.len());
            let stack_segment = (self.0 >> 3) & mask;
            let p1 = !stack_segment & mask;
            let p2 = stack_segment;
            (p1, p2)
        } else {
            (0, 0)
        }
    }
}

impl fmt::Debug for Stack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, " ")?
        } else {
            for (i, piece) in self.iter().rev().enumerate() {
                if i > 0 {
                    write!(f, " ")?;
                }
                write!(f, "{piece:?}")?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for Stack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

pub struct StackIter<'a> {
    stack: &'a Stack,
    i_top: usize,
    i_bottom: usize,
}

impl<'a> Iterator for StackIter<'a> {
    type Item = Piece;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i_top == self.i_bottom {
            None
        } else {
            self.i_top += 1;
            self.stack.get(self.i_top - 1)
        }
    }
}

impl<'a> DoubleEndedIterator for StackIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.i_bottom == self.i_top {
            None
        } else {
            self.i_bottom -= 1;
            self.stack.get(self.i_bottom)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get() {
        let stack = Stack(0b1010110010);
        assert_eq!(stack.get(0), Some(Piece::new(StandingStone, White)));
        assert_eq!(stack.get(1), Some(Piece::new(Flatstone, Black)));
        assert_eq!(stack.get(2), Some(Piece::new(Flatstone, Black)));
        assert_eq!(stack.get(3), Some(Piece::new(Flatstone, White)));
        assert_eq!(stack.get(4), Some(Piece::new(Flatstone, Black)));
        assert_eq!(stack.get(5), Some(Piece::new(Flatstone, White)));
    }

    #[test]
    fn add() {
        let mut a = Stack(0b1010001);
        let b = Stack(0b1110010);
        a.add(b);
        assert_eq!(a, Stack(0b1010110010));
    }

    #[test]
    fn add_piece() {
        let mut a = Stack(0b1010001);
        a.add_piece(Piece::new(Capstone, Black));
        assert_eq!(a, Stack(0b10101100));
    }

    #[test]
    fn take() {
        let mut a = Stack(0b1010110010);
        let b = a.take(3);
        assert_eq!(a, Stack(0b1010001));
        assert_eq!(b, Stack(0b1110010));

        let mut a = Stack(0b1010110010);
        let b = a.take(6);
        assert_eq!(a, Stack::default());
        assert_eq!(b, Stack(0b1010110010));
    }

    #[test]
    fn drop() {
        let mut a = Stack(0b1010110010);
        let b = a.drop(3);
        assert_eq!(a, Stack(0b1110010));
        assert_eq!(b, Stack(0b1010001));
    }

    #[test]
    fn iter() {
        let stack = Stack(0b1010110010);
        let mut iter = stack.iter();
        assert_eq!(iter.next(), Some(Piece::new(StandingStone, White)));
        assert_eq!(iter.next(), Some(Piece::new(Flatstone, Black)));
        assert_eq!(iter.next(), Some(Piece::new(Flatstone, Black)));
        assert_eq!(iter.next(), Some(Piece::new(Flatstone, White)));
        assert_eq!(iter.next(), Some(Piece::new(Flatstone, Black)));
        assert_eq!(iter.next(), Some(Piece::new(Flatstone, White)));

        let mut iter = stack.iter().rev();
        assert_eq!(iter.next(), Some(Piece::new(Flatstone, White)));
        assert_eq!(iter.next(), Some(Piece::new(Flatstone, Black)));
        assert_eq!(iter.next(), Some(Piece::new(Flatstone, White)));
        assert_eq!(iter.next(), Some(Piece::new(Flatstone, Black)));
        assert_eq!(iter.next(), Some(Piece::new(Flatstone, Black)));
        assert_eq!(iter.next(), Some(Piece::new(StandingStone, White)));
    }

    #[test]
    fn get_player_pieces() {
        let stack = Stack(0b1011010001);
        assert_eq!(stack.get_player_pieces(), (0b100101, 0b011010));

        let stack = Stack(0b101101001101001);
        assert_eq!(stack.get_player_pieces(), (0b10010110010, 0b01101001101));

        let stack = Stack(0b1000);
        assert_eq!(stack.get_player_pieces(), (0, 0));
    }
}
