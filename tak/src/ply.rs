use std::fmt;

use once_cell::sync::Lazy;
use tracing::{instrument, trace};

use crate::bitmap::Bitmap;
use crate::piece::PieceType;
use crate::ptn::PtnPly;
use crate::state::State;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    North = 0,
    East,
    South,
    West,
}

impl Direction {
    pub fn to_offset(self) -> (i8, i8) {
        match self {
            Direction::North => (0, 1),
            Direction::East => (1, 0),
            Direction::South => (0, -1),
            Direction::West => (-1, 0),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Drops(u8);

impl Drops {
    pub fn new<const N: usize>(drops: &[u8]) -> Result<Self, PlyError> {
        if drops.len() >= N {
            return Err(PlyError::InvalidDrops("Too many drops."));
        } else if drops.len() == 0 {
            return Err(PlyError::InvalidDrops("Must specify at least one drop."));
        }

        if drops.iter().any(|d| *d == 0) {
            return Err(PlyError::InvalidDrops("Invalid drop amount."));
        }

        if drops.iter().sum::<u8>() as usize > N {
            return Err(PlyError::InvalidDrops("Illegal carry amount."));
        }

        let mut map = 0;
        for drop in drops.iter().rev() {
            map <<= 1;
            map |= 1;
            map <<= drop - 1;
        }

        Ok(Self(map))
    }

    pub fn id(&self) -> usize {
        self.0 as usize
    }

    pub fn iter(&self) -> impl Iterator<Item = u8> {
        struct DropIterator(u8);

        impl Iterator for DropIterator {
            type Item = u8;

            fn next(&mut self) -> Option<Self::Item> {
                if self.0 > 0 {
                    let drop = self.0.trailing_zeros() as u8 + 1;
                    self.0 >>= drop;
                    Some(drop)
                } else {
                    None
                }
            }
        }

        DropIterator(self.0)
    }

    pub fn last(&self) -> usize {
        let len = self.len();
        if len > 1 {
            (self.0 << (self.0.leading_zeros() + 1)).leading_zeros() as usize + 1
        } else {
            self.carry()
        }
    }

    pub fn len(&self) -> usize {
        self.0.count_ones() as usize
    }

    pub fn carry(&self) -> usize {
        (8 - self.0.leading_zeros()) as usize
    }
}

impl fmt::Debug for Drops {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let drops = self.iter().collect::<Vec<_>>();
        f.debug_tuple("Drops").field(&drops).finish()
    }
}

impl From<Drops> for u8 {
    fn from(drops: Drops) -> Self {
        drops.0
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Ply<const N: usize> {
    Place {
        x: u8,
        y: u8,
        piece_type: PieceType,
    },
    Spread {
        x: u8,
        y: u8,
        direction: Direction,
        drops: Drops,
        crush: bool,
    },
}

impl<const N: usize> Ply<N> {
    #[instrument(level = "trace")]
    pub fn validate(self) -> Result<(), PlyError> {
        match self {
            Ply::Place { x, y, .. } => {
                if x as usize >= N || y as usize >= N {
                    trace!("Out of bounds.");
                    return Err(PlyError::OutOfBounds);
                }
            }
            Ply::Spread {
                x,
                y,
                direction,
                drops,
                crush,
            } => {
                if x as usize >= N || y as usize >= N {
                    trace!("Out of bounds.");
                    return Err(PlyError::OutOfBounds);
                }

                // The end of the spread must be in bounds.
                let (dx, dy) = direction.to_offset();
                let (tx, ty) = (
                    x as i8 + dx * drops.len() as i8,
                    y as i8 + dy * drops.len() as i8,
                );
                if tx < 0 || tx as usize >= N || ty < 0 || ty as usize >= N {
                    trace!("End of spread is out of bounds.");
                    return Err(PlyError::OutOfBounds);
                }

                // Must crush with only one stone.
                if crush && drops.iter().last() != Some(1) {
                    trace!("Invalid crush.");
                    return Err(PlyError::InvalidCrush);
                }
            }
        }

        Ok(())
    }
}

impl<const N: usize> fmt::Debug for Ply<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptn = PtnPly::from(*self);
        write!(f, "{ptn}")
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum PlyError {
    OutOfBounds,
    InvalidDrops(&'static str),
    InvalidCrush,
}

pub mod generation {
    use super::*;

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

                    (1..1 << pickup_size)
                        .map(|value| Drops::new::<N>(value as u8).expect("invalid drops"))
                        .filter(move |drops| drops.len() <= distance)
                        .filter_map(move |drops| {
                            let tx = x as i8 + drops.len() as i8 * dx;
                            let ty = y as i8 + drops.len() as i8 * dy;
                            let target_type =
                                state.board[tx as usize][ty as usize].top_piece_type();

                            // Allow this drop combo if the target is a flatstone or empty.
                            let unblocked = target_type.is_none() || target_type == Some(Flatstone);

                            // Allow this drop combo if the target is a standing stone, and we're
                            // dropping a capstone by itself onto it.
                            let crush = target_type == Some(StandingStone)
                                && top_piece.piece_type() == Capstone
                                && drops.last() == 1;

                            (unblocked || crush).then_some((drops, crush))
                        })
                        .map(move |(drops, crush)| Ply::Spread {
                            x: x as u8,
                            y: y as u8,
                            direction,
                            drops,
                            crush,
                        })
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use generation::spread_map;

    #[test]
    fn drops() {
        let drops = Drops::from_drop_counts::<6>(&[3, 2, 1]).unwrap();
        let mut d = drops.iter();

        assert_eq!(d.next(), Some(3));
        assert_eq!(d.next(), Some(2));
        assert_eq!(d.next(), Some(1));
        assert_eq!(d.next(), None);
    }

    #[test]
    fn drops_invalid_carry() {
        assert!(Drops::from_drop_counts::<6>(&[3, 3, 1]).is_err());
    }

    #[test]
    fn drops_invalid_drop() {
        assert!(Drops::from_drop_counts::<6>(&[3, 2, 0, 1]).is_err());
        assert!(Drops::from_drop_counts::<6>(&[3, 2, 1, 0]).is_err());
        assert!(Drops::from_drop_counts::<6>(&[0, 3, 2, 1]).is_err());
    }

    #[test]
    fn drops_last() {
        assert_eq!(Drops::from_drop_counts::<6>(&[3, 2, 1]).unwrap().last(), 1);
        assert_eq!(Drops::from_drop_counts::<6>(&[1, 2, 3]).unwrap().last(), 3);
        assert_eq!(Drops::from_drop_counts::<6>(&[3]).unwrap().last(), 3);
        assert_eq!(Drops::from_drop_counts::<6>(&[1]).unwrap().last(), 1);
    }

}
