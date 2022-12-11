use std::fmt;

use crate::piece::{Color, Piece, PieceType};

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct Stack {
    bitmap: u64,
    height: u8,
    top_piece: Option<Piece>,
}

impl Stack {
    pub fn from_piece(piece: Piece) -> Self {
        Self {
            bitmap: match piece.color() {
                Color::White => 0,
                Color::Black => 1,
            },
            height: 1,
            top_piece: Some(piece),
        }
    }

    pub fn len(&self) -> usize {
        self.height as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn last(&self) -> Option<Piece> {
        self.top_piece
    }

    pub fn last_piece_type(&self) -> Option<PieceType> {
        self.last().map(|p| p.piece_type())
    }

    pub fn last_color(&self) -> Option<Color> {
        self.last().map(|p| p.color())
    }

    pub fn get(&self, index: usize) -> Option<Piece> {
        if index >= self.len() {
            None
        } else if index == self.len() - 1 {
            self.top_piece
        } else if ((self.bitmap >> index) & 0x01) == 0 {
            Some(Piece::new(PieceType::Flatstone, Color::White))
        } else {
            Some(Piece::new(PieceType::Flatstone, Color::Black))
        }
    }

    pub fn add(&mut self, stack: Self) {
        assert!(self.height + stack.height <= 64, "exceeded stack limit");
        if stack.height > 0 {
            self.bitmap |= stack.bitmap << self.height;
            self.height += stack.height;
            self.top_piece = stack.top_piece;
        }
    }

    pub fn add_piece(&mut self, piece: Piece) {
        self.add(Stack::from_piece(piece))
    }

    pub fn take(&mut self, count: usize) -> Self {
        assert!(count > 0 && count <= self.height as usize);

        let remaining = self.height as usize - count;
        let carry_stack = Self {
            bitmap: self.bitmap >> remaining,
            height: count as u8,
            top_piece: self.top_piece,
        };

        let mask = 0xFFFFFFFFFFFFFFFFu64 << remaining;
        self.bitmap &= !mask;
        self.height = remaining as u8;
        self.top_piece = (remaining > 0).then(|| {
            Piece::new(
                PieceType::Flatstone,
                if self.bitmap >> (remaining - 1) == 0 {
                    Color::White
                } else {
                    Color::Black
                },
            )
        });

        carry_stack
    }

    pub fn drop(&mut self, count: usize) -> Self {
        assert!(count > 0 && count <= self.height as usize);

        let mask = 0xFFFFFFFFFFFFFFFFu64 << count;
        let drop_stack = Self {
            bitmap: self.bitmap & !mask,
            height: count as u8,
            top_piece: if count == self.height as usize {
                self.top_piece
            } else {
                Some(Piece::new(
                    PieceType::Flatstone,
                    if ((self.bitmap >> (count - 1)) & 0x01) == 0 {
                        Color::White
                    } else {
                        Color::Black
                    },
                ))
            },
        };

        self.bitmap >>= count;
        self.height -= count as u8;
        self.top_piece = self.top_piece.filter(|_| self.height > 0);

        drop_stack
    }

    /// Returns an iterator over the pieces in the stack from top to bottom.
    pub fn iter(&self) -> StackIter<'_> {
        StackIter {
            stack: self,
            i_top: self.len(),
            i_bottom: 0,
        }
    }
}

impl fmt::Debug for Stack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, " ")?
        } else {
            for piece in self.iter().rev() {
                write!(f, "{:?}", piece)?
            }
        }
        Ok(())
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
