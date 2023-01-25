use std::fmt;

use crate::piece::{Color, Piece, PieceType};

#[cfg(not(feature = "deep-stacks"))]
type Bitmap = u32;

#[cfg(feature = "deep-stacks")]
type Bitmap = u128;

const MAX_STACK_HEIGHT: usize = Bitmap::BITS as usize - 4;

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Stack(Bitmap);

impl Default for Stack {
    fn default() -> Self {
        Self(0b1000)
    }
}

impl Stack {
    pub fn from_piece(piece: Piece) -> Self {
        Self(0b10000 | (piece.piece_type() as Bitmap >> 3) | (piece.color() as Bitmap - 1))
    }

    pub fn len(&self) -> usize {
        MAX_STACK_HEIGHT - self.0.leading_zeros() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn last(&self) -> Option<Piece> {
        if self.is_empty() {
            None
        } else {
            self.get(self.len() - 1)
        }
    }

    pub fn last_piece_type(&self) -> Option<PieceType> {
        self.last().map(|p| p.piece_type())
    }

    pub fn last_color(&self) -> Option<Color> {
        self.last().map(|p| p.color())
    }

    pub fn get(&self, index: usize) -> Option<Piece> {
        if index >= self.len() {
            return None;
        }

        let piece_type = if index == self.len() - 1 {
            (((self.0 >> self.len()) as u8 & 0x07) << 4)
                .try_into()
                .unwrap()
        } else {
            PieceType::Flatstone
        };

        if ((self.0 >> index) & 0x01) == 0 {
            Some(Piece::new(piece_type, Color::White))
        } else {
            Some(Piece::new(piece_type, Color::Black))
        }
    }

    pub fn add(&mut self, stack: Self) {
        assert!(
            self.len() + stack.len() <= MAX_STACK_HEIGHT,
            "exceeded stack limit, compile with \"deep-stacks\" feature to support this"
        );
        if !stack.is_empty() {
            let old_len = self.len();
            let mask = Bitmap::MAX << old_len;
            self.0 &= !mask;
            self.0 |= stack.0 << old_len;
        }
    }

    pub fn add_piece(&mut self, piece: Piece) {
        self.add(Stack::from_piece(piece))
    }

    pub fn take(&mut self, count: usize) -> Self {
        assert!(count > 0 && count <= self.len());

        let remaining = self.len() - count;
        let carry_stack = Self(self.0 >> remaining);

        if remaining > 0 {
            let mask = Bitmap::MAX << remaining;
            self.0 &= !mask;
            self.0 |= (0b1001 as Bitmap) << remaining;
        } else {
            *self = Self::default();
        }

        carry_stack
    }

    pub fn drop(&mut self, count: usize) -> Self {
        assert!(count > 0 && count <= self.len());

        if count < self.len() {
            let mask = Bitmap::MAX << count;
            let drop_stack = Self((self.0 & !mask) | ((0b1001 as Bitmap) << count));
            self.0 >>= count;
            drop_stack
        } else {
            let drop_stack = *self;
            *self = Self::default();
            drop_stack
        }
    }

    /// Returns an iterator over the pieces in the stack from top to bottom.
    pub fn iter(&self) -> StackIter<'_> {
        StackIter {
            stack: self,
            i_top: self.len(),
            i_bottom: 0,
        }
    }

    /// Returns the positions of the top 8 pieces of the stack for each color.
    /// The first byte is white's piece map, the second is black's.
    /// A 1 is a piece of that color, a 0 could be the opponent's piece or an
    /// empty space (if the stack is less than 8 pieces tall).
    pub(crate) fn get_player_pieces(&self) -> (u8, u8) {
        if !self.is_empty() {
            let mask = 0xFFu8 >> (8 - self.len().min(8));
            let stack_segment = (self.0 >> (self.len().max(8) - 8)) as u8 & mask;
            let p1 = !stack_segment & mask;
            let p2 = stack_segment;
            (p1, p2)
        } else {
            (0x00, 0x00)
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
            self.i_top -= 1;
            self.stack.get(self.i_top)
        }
    }
}

impl<'a> DoubleEndedIterator for StackIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.i_bottom == self.i_top {
            None
        } else {
            self.i_bottom += 1;
            self.stack.get(self.i_bottom - 1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_player_pieces() {
        let stack = Stack(0b1001011010);
        assert_eq!(stack.get_player_pieces(), (0b100101, 0b11010));

        let stack = Stack(0b100101101001101);
        assert_eq!(stack.get_player_pieces(), (0b10010110, 0b1101001));

        let stack = Stack(0b1000);
        assert_eq!(stack.get_player_pieces(), (0, 0));
    }
}
