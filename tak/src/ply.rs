use std::fmt;

use tracing::{instrument, trace};

use crate::piece::PieceType;
use crate::ptn::PtnPly;

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
        drops: [u8; N],
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

                // Drops must begin with a non-zero number, have at least one zero at some point, and all zeros must be at the end.
                // (It's not possible to drop N times on an NxN board.)
                let drop_count = drops
                    .into_iter()
                    .position(|d| d == 0)
                    .ok_or(PlyError::InvalidDrops("Too many drops."))?;
                if !drops[drop_count..].iter().all(|d| *d == 0) {
                    trace!("Misordered drops.");
                    return Err(PlyError::InvalidDrops(
                        "All drops must be at the beginning of the list.",
                    ));
                }
                if drop_count == 0 {
                    trace!("No drops.");
                    return Err(PlyError::InvalidDrops("Must specify at least one drop."));
                }

                // Must not carry more than the size of the board or the size of the stack.
                let carry_total = drops.iter().sum::<u8>() as usize;
                if carry_total > N {
                    trace!("Illegal carry amount.");
                    return Err(PlyError::InvalidDrops("Illegal carry amount."));
                }

                // Must crush with only one stone.
                if crush && drops[drop_count - 1] != 1 {
                    trace!("Invalid crush.");
                    return Err(PlyError::InvalidCrush);
                }

                // The end of the spread must be in bounds.
                let (dx, dy) = direction.to_offset();
                let (tx, ty) = (
                    x as i8 + dx * drop_count as i8,
                    y as i8 + dy * drop_count as i8,
                );
                if tx < 0 || tx as usize >= N || ty < 0 || ty as usize >= N {
                    trace!("End of spread is out of bounds.");
                    return Err(PlyError::OutOfBounds);
                }
            }
        }

        Ok(())
    }
}

impl<const N: usize> fmt::Debug for Ply<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptn: PtnPly = self.into();
        write!(f, "{}", &*ptn)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum PlyError {
    OutOfBounds,
    InvalidDrops(&'static str),
    InvalidCrush,
}
