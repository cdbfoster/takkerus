use std::convert::TryFrom;
use std::ops::Deref;

use crate::piece::PieceType;
use crate::ply::{Direction, Ply};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PtnPly(String);

impl PtnPly {
    pub fn new(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl<const N: usize> TryFrom<PtnPly> for Ply<N> {
    type Error = PtnError;

    fn try_from(value: PtnPly) -> Result<Self, Self::Error> {
        let mut grab = None;
        let mut piece_type = None;

        let mut chars = value.0.chars();

        let mut next = chars.next().ok_or(PtnError::InputTooShort)?;
        match next {
            'F' => {
                piece_type = Some(PieceType::Flatstone);
                next = chars.next().ok_or(PtnError::InputTooShort)?;
            }
            'S' => {
                piece_type = Some(PieceType::StandingStone);
                next = chars.next().ok_or(PtnError::InputTooShort)?;
            }
            'C' => {
                piece_type = Some(PieceType::Capstone);
                next = chars.next().ok_or(PtnError::InputTooShort)?;
            }
            c => {
                if let Some(number) = c.to_digit(10) {
                    if number == 0 || number > N as u32 {
                        return Err(PtnError::InvalidValue("Invalid carry amount."));
                    }
                    grab = Some(number as u8);
                    next = chars.next().ok_or(PtnError::InputTooShort)?;
                }
            }
        }

        let column = next
            .to_digit(10 + N as u32)
            .filter(|n| *n >= 10)
            .map(|n| n - 10)
            .ok_or(PtnError::InvalidValue("Invalid file letter."))? as usize;

        next = chars.next().ok_or(PtnError::InputTooShort)?;
        let row = next
            .to_digit(10)
            .filter(|n| *n > 0 && *n <= N as u32)
            .map(|n| n - 1)
            .ok_or(PtnError::InvalidValue("Invalid rank number."))? as usize;

        let direction = match chars.next() {
            Some('+') => Some(Direction::North),
            Some('>') => Some(Direction::East),
            Some('-') => Some(Direction::South),
            Some('<') => Some(Direction::West),
            Some(_) => return Err(PtnError::InvalidValue("Expected a direction.")),
            None => {
                if grab.is_some() {
                    return Err(PtnError::InvalidSlide(
                        "Carry amount specified without direction.",
                    ));
                }
                return Ok(Ply::Place {
                    x: column as u8,
                    y: row as u8,
                    piece_type: piece_type.unwrap_or(PieceType::Flatstone),
                });
            }
        };

        let mut drop_amounts = Vec::new();
        let mut crush = false;

        for next in chars {
            if let Some(amount) = next.to_digit(10).filter(|n| *n > 0) {
                drop_amounts.push(amount as u8);
            } else if next == '*' {
                crush = true;
            } else if next == '?' || next == '!' {
                // Ignore commentary markings.
                continue;
            } else {
                return Err(PtnError::InvalidValue("Invalid drop amount."));
            }
        }

        if drop_amounts.is_empty() {
            drop_amounts.push(grab.unwrap_or(1));
        }

        if crush && *drop_amounts.last().unwrap() != 1 {
            return Err(PtnError::InvalidSlide(
                "Cannot crush with more than one stone.",
            ));
        }

        let drop_squares = drop_amounts.len();
        if match direction.unwrap() {
            Direction::North => row + drop_squares >= N,
            Direction::East => column + drop_squares >= N,
            Direction::South => drop_squares > row,
            Direction::West => drop_squares > column,
        } {
            return Err(PtnError::InvalidSlide("Cannot slide out of bounds."));
        }

        if drop_amounts.iter().sum::<u8>() != grab.unwrap_or(1) {
            return Err(PtnError::InvalidValue(
                "Carry and drop amounts don't match.",
            ));
        }

        let mut drops = [0; N];
        drops[..drop_amounts.len()].copy_from_slice(&drop_amounts);

        Ok(Ply::Slide {
            x: column as u8,
            y: row as u8,
            direction: direction.unwrap(),
            drops,
            crush,
        })
    }
}

impl<const N: usize> TryFrom<&str> for Ply<N> {
    type Error = PtnError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        PtnPly::new(value).try_into()
    }
}

impl Deref for PtnPly {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum PtnError {
    InputTooShort,
    InputTooLong,
    InvalidValue(&'static str),
    InvalidSlide(&'static str),
}

impl<const N: usize> From<Ply<N>> for PtnPly {
    fn from(ply: Ply<N>) -> Self {
        let mut buffer = String::new();

        match ply {
            Ply::Place { x, y, piece_type } => {
                match piece_type {
                    PieceType::Flatstone => (),
                    PieceType::StandingStone => buffer.push('S'),
                    PieceType::Capstone => buffer.push('C'),
                }

                buffer.push(char::from_digit(x as u32 + 10, 10 + N as u32).unwrap());
                buffer.push(char::from_digit(y as u32 + 1, 10).unwrap());
            }
            Ply::Slide {
                x,
                y,
                direction,
                drops,
                crush,
            } => {
                let count = drops.into_iter().sum::<u8>() as u32;
                buffer.push(char::from_digit(count, 10).unwrap());

                buffer.push(char::from_digit(x as u32 + 10, 10 + N as u32).unwrap());
                buffer.push(char::from_digit(y as u32 + 1, 10).unwrap());

                buffer.push(match direction {
                    Direction::North => '+',
                    Direction::East => '>',
                    Direction::South => '-',
                    Direction::West => '<',
                });

                if drops.into_iter().filter(|d| *d != 0).count() > 1 {
                    for drop in drops.into_iter().filter(|d| *d != 0) {
                        buffer.push(char::from_digit(drop as u32, 10).unwrap());
                    }
                }

                if crush {
                    buffer.push('*');
                }
            }
        }

        Self(buffer)
    }
}

impl<const N: usize> From<&Ply<N>> for PtnPly {
    fn from(ply: &Ply<N>) -> Self {
        Self::from(*ply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;

    use PieceType::*;

    #[test]
    fn coordinates_are_in_bounds() {
        assert_eq!(
            "a1".try_into(),
            Ok(Ply::<3>::Place {
                x: 0,
                y: 0,
                piece_type: Flatstone
            })
        );

        assert_eq!(
            "c3".try_into(),
            Ok(Ply::<3>::Place {
                x: 2,
                y: 2,
                piece_type: Flatstone
            })
        );

        assert_eq!(
            <&str as TryInto<Ply<3>>>::try_into("d1"),
            Err(PtnError::InvalidValue("Invalid file letter.")),
        );

        assert_eq!(
            <&str as TryInto<Ply<3>>>::try_into("a4"),
            Err(PtnError::InvalidValue("Invalid rank number.")),
        );
    }

    #[test]
    fn place_piece_types() {
        assert_eq!(
            "a1".try_into(),
            Ok(Ply::<3>::Place {
                x: 0,
                y: 0,
                piece_type: Flatstone
            })
        );

        assert_eq!(
            "Fa1".try_into(),
            Ok(Ply::<3>::Place {
                x: 0,
                y: 0,
                piece_type: Flatstone
            })
        );

        assert_eq!(
            "Sa1".try_into(),
            Ok(Ply::<3>::Place {
                x: 0,
                y: 0,
                piece_type: StandingStone
            })
        );

        assert_eq!(
            "Ca1".try_into(),
            Ok(Ply::<3>::Place {
                x: 0,
                y: 0,
                piece_type: Capstone
            })
        );
    }

    #[test]
    fn slide_directions() {
        assert_eq!(
            "c3+".try_into(),
            Ok(Ply::<5>::Slide {
                x: 2,
                y: 2,
                direction: Direction::North,
                drops: [1, 0, 0, 0, 0],
                crush: false,
            }),
        );

        assert_eq!(
            "c3>".try_into(),
            Ok(Ply::<5>::Slide {
                x: 2,
                y: 2,
                direction: Direction::East,
                drops: [1, 0, 0, 0, 0],
                crush: false,
            }),
        );

        assert_eq!(
            "c3-".try_into(),
            Ok(Ply::<5>::Slide {
                x: 2,
                y: 2,
                direction: Direction::South,
                drops: [1, 0, 0, 0, 0],
                crush: false,
            }),
        );

        assert_eq!(
            "c3<".try_into(),
            Ok(Ply::<5>::Slide {
                x: 2,
                y: 2,
                direction: Direction::West,
                drops: [1, 0, 0, 0, 0],
                crush: false,
            }),
        );
    }

    #[test]
    fn slide_amounts() {
        assert_eq!(
            <&str as TryInto<Ply<5>>>::try_into("a3*"),
            Err(PtnError::InvalidValue("Expected a direction.")),
        );

        assert_eq!(
            <&str as TryInto<Ply<3>>>::try_into("4a1+"),
            Err(PtnError::InvalidValue("Invalid carry amount.")),
        );

        assert_eq!(
            "a3>".try_into(),
            Ok(Ply::<5>::Slide {
                x: 0,
                y: 2,
                direction: Direction::East,
                drops: [1, 0, 0, 0, 0],
                crush: false,
            }),
        );

        assert_eq!(
            "a3>1".try_into(),
            Ok(Ply::<5>::Slide {
                x: 0,
                y: 2,
                direction: Direction::East,
                drops: [1, 0, 0, 0, 0],
                crush: false,
            }),
        );

        assert_eq!(
            "3a3>12".try_into(),
            Ok(Ply::<5>::Slide {
                x: 0,
                y: 2,
                direction: Direction::East,
                drops: [1, 2, 0, 0, 0],
                crush: false,
            }),
        );

        assert_eq!(
            <&str as TryInto<Ply<5>>>::try_into("3a3>22"),
            Err(PtnError::InvalidValue(
                "Carry and drop amounts don't match."
            )),
        );
    }

    #[test]
    fn slide_bounds() {
        assert_eq!(
            <&str as TryInto<Ply<3>>>::try_into("a3+"),
            Err(PtnError::InvalidSlide("Cannot slide out of bounds.")),
        );

        assert_eq!(
            <&str as TryInto<Ply<3>>>::try_into("3a1>111"),
            Err(PtnError::InvalidSlide("Cannot slide out of bounds.")),
        );

        assert_eq!(
            <&str as TryInto<Ply<3>>>::try_into("a1-"),
            Err(PtnError::InvalidSlide("Cannot slide out of bounds.")),
        );

        assert_eq!(
            <&str as TryInto<Ply<3>>>::try_into("2b1<11"),
            Err(PtnError::InvalidSlide("Cannot slide out of bounds.")),
        );
    }

    #[test]
    fn crushes() {
        assert_eq!(
            "3a3>21*".try_into(),
            Ok(Ply::<5>::Slide {
                x: 0,
                y: 2,
                direction: Direction::East,
                drops: [2, 1, 0, 0, 0],
                crush: true,
            }),
        );

        assert_eq!(
            <&str as TryInto<Ply<5>>>::try_into("3a3>12*"),
            Err(PtnError::InvalidSlide(
                "Cannot crush with more than one stone."
            )),
        );
    }
}
