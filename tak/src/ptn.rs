use std::convert::TryFrom;
use std::fmt::{self, Write};
use std::fs::File;
use std::io::{Error as IoError, Read};
use std::path::Path;
use std::str::FromStr;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::piece::PieceType;
use crate::ply::{Direction, Ply};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PtnGame {
    pub headers: Vec<PtnHeader>,
    pub opening_comments: Vec<String>,
    pub turns: Vec<PtnTurn>,
    pub result: Option<String>,
    pub closing_comments: Vec<String>,
}

impl PtnGame {
    pub fn from_file(filename: impl AsRef<Path>) -> Result<Self, PtnError> {
        let mut contents = String::new();
        File::open(filename.as_ref())?.read_to_string(&mut contents)?;

        contents.parse()
    }
}

impl FromStr for PtnGame {
    type Err = PtnError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut text = s.trim();

        // Parse headers.
        let mut headers = Vec::new();
        if let Some(header_section) = HEADER_SECTION.find(text) {
            for line in header_section.as_str().lines() {
                headers.push(line.trim().parse()?);
            }
            text = &text[header_section.end()..];
        }

        // Parse opening comments.
        let mut opening_comments = Vec::new();
        while let Some(comment) = BODY_COMMENT.captures(text) {
            opening_comments.push(comment.name("comment").unwrap().as_str().to_owned());
            text = &text[comment.get(0).unwrap().end()..];
        }

        // Parse turns.
        let mut turns = Vec::new();
        while let Some(turn) = TURN.find(text) {
            turns.push(turn.as_str().parse()?);
            text = &text[turn.end()..];
        }

        // Parse result.
        let mut result = None;
        if let Some(r) = RESULT.find(text) {
            result = Some(r.as_str().to_owned());
            text = &text[r.end()..];
        }

        // Parse closing comments.
        let mut closing_comments = Vec::new();
        while let Some(comment) = BODY_COMMENT.captures(text) {
            closing_comments.push(comment.name("comment").unwrap().as_str().to_owned());
            text = &text[comment.get(0).unwrap().end()..];
        }

        Ok(Self {
            headers,
            opening_comments,
            turns,
            result,
            closing_comments,
        })
    }
}

impl fmt::Display for PtnGame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = String::new();

        if !self.headers.is_empty() {
            for header in &self.headers {
                writeln!(buffer, "{header}")?;
            }
            writeln!(buffer)?;
        }

        if !self.opening_comments.is_empty() {
            for comment in &self.opening_comments {
                writeln!(buffer, "{{{comment}}}")?;
            }
            writeln!(buffer)?;
        }

        if !self.turns.is_empty() {
            for turn in &self.turns {
                writeln!(buffer, "{turn}")?;
            }

            if let Some(result) = &self.result {
                writeln!(buffer, "{result}")?;
            }

            writeln!(buffer)?;
        }

        for comment in &self.closing_comments {
            writeln!(buffer, "{{{comment}}}")?;
        }

        write!(f, "{}", buffer.trim())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PtnHeader {
    pub key: String,
    pub value: String,
}

impl FromStr for PtnHeader {
    type Err = PtnError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || PtnError::InvalidHeader(s.to_owned());

        let c = HEADER.captures(s).ok_or_else(invalid)?;

        Ok(Self {
            key: c.name("key").unwrap().as_str().to_owned(),
            value: c.name("value").unwrap().as_str().to_owned(),
        })
    }
}

impl fmt::Display for PtnHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { key, value } = self;
        write!(f, "[{key} \"{value}\"]")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PtnTurn {
    pub number: u32,
    pub p1_move: PtnMove,
    pub p2_move: PtnMove,
}

impl FromStr for PtnTurn {
    type Err = PtnError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || PtnError::InvalidHeader(s.to_owned());

        let c = TURN.captures(s).ok_or_else(invalid)?;

        // p1's move can have comments without a ply.
        let p1_move = {
            let ply = c.name("p1_move").map(|m| m.as_str().parse()).transpose()?;
            let comments = c
                .name("p1_move_comments")
                .map(|m| COMMENT.captures_iter(m.as_str()))
                .map(|i| {
                    i.map(|x| x.name("comment").unwrap().as_str().to_owned())
                        .collect()
                })
                .unwrap_or_else(|| Vec::new());
            PtnMove { ply, comments }
        };

        // p2's move must have a ply to have comments.
        let p2_move = if let Some(m) = c.name("p2_move") {
            PtnMove {
                ply: Some(m.as_str().parse()?),
                comments: c
                    .name("p2_move_comments")
                    .map(|m| COMMENT.captures_iter(m.as_str()))
                    .map(|i| {
                        i.map(|x| x.name("comment").unwrap().as_str().to_owned())
                            .collect()
                    })
                    .unwrap_or_else(|| Vec::new()),
            }
        } else {
            PtnMove::default()
        };

        Ok(Self {
            number: c
                .name("turn_number")
                .ok_or_else(invalid)?
                .as_str()
                .parse()
                .unwrap(),
            p1_move,
            p2_move,
        })
    }
}

impl fmt::Display for PtnTurn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            number,
            p1_move,
            p2_move,
        } = self;
        write!(f, "{number}. {p1_move}")?;

        if p2_move.ply.is_some() {
            write!(f, " {p2_move}")?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PtnMove {
    pub ply: Option<PtnPly>,
    pub comments: Vec<String>,
}

impl fmt::Display for PtnMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ply) = &self.ply {
            write!(f, "{ply}")?;
        } else {
            write!(f, "--")?;
        }

        for comment in &self.comments {
            write!(f, " {{{comment}}}")?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PtnPly {
    Place {
        x: u8,
        y: u8,
        piece_type: PieceType,
        annotations: Option<String>,
    },
    Spread {
        x: u8,
        y: u8,
        direction: Direction,
        drops: Vec<u8>,
        annotations: Option<String>,
    },
}

impl FromStr for PtnPly {
    type Err = PtnError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || PtnError::InvalidPly(s.to_owned());

        let c = PLY.captures(s).ok_or_else(invalid)?;

        let file_letter = c
            .name("place_file")
            .or_else(|| c.name("spread_file"))
            .and_then(|s| s.as_str().chars().next())
            .unwrap();
        let file_number = file_letter.to_digit(18).unwrap() as u8 - 10;

        let rank_number = c
            .name("place_rank")
            .or_else(|| c.name("spread_rank"))
            .and_then(|s| s.as_str().parse::<u8>().ok())
            .unwrap()
            - 1;

        let annotations = c.name("annotations").map(|s| s.as_str().to_owned());

        if let Some(direction) = c.name("direction") {
            let direction = match direction.as_str() {
                "+" => Direction::North,
                ">" => Direction::East,
                "-" => Direction::South,
                "<" => Direction::West,
                _ => unreachable!(),
            };

            let carry = c
                .name("carry")
                .map_or(1, |n| n.as_str().parse::<u8>().unwrap());

            let drops = c.name("drops").map_or_else(
                || vec![carry],
                |drops| {
                    drops
                        .as_str()
                        .chars()
                        .map(|n| n.to_digit(10).unwrap() as u8)
                        .collect()
                },
            );

            if drops.iter().sum::<u8>() != carry {
                return Err(invalid());
            }

            Ok(PtnPly::Spread {
                x: file_number,
                y: rank_number,
                direction,
                drops,
                annotations,
            })
        } else {
            let piece_type = match c.name("place_type").map(|s| s.as_str()).unwrap_or("F") {
                "F" => PieceType::Flatstone,
                "S" => PieceType::StandingStone,
                "C" => PieceType::Capstone,
                _ => unreachable!(),
            };

            Ok(PtnPly::Place {
                x: file_number,
                y: rank_number,
                piece_type,
                annotations,
            })
        }
    }
}

impl<const N: usize> TryFrom<PtnPly> for Ply<N> {
    type Error = PtnError;

    fn try_from(value: PtnPly) -> Result<Self, Self::Error> {
        // Validate x and y.
        let (x, y) = match &value {
            PtnPly::Place { x, y, .. } => (x, y),
            PtnPly::Spread { x, y, .. } => (x, y),
        };
        if *x as usize >= N || *y as usize >= N {
            return Err(PtnError::InvalidPly(format!(
                "Board is size {N} and position is ({}, {}).",
                x + 1,
                y + 1
            )));
        }

        match value {
            PtnPly::Place {
                x, y, piece_type, ..
            } => Ok(Self::Place { x, y, piece_type }),
            PtnPly::Spread {
                x,
                y,
                direction,
                drops,
                annotations,
            } => {
                let carry = drops.iter().sum::<u8>();
                if carry as usize > N {
                    return Err(PtnError::InvalidPly(format!(
                        "Cannot carry {carry} stones on a board of size {N}."
                    )));
                }

                let drop_squares = drops.len() as u8;
                let out_of_bounds = match direction {
                    Direction::North => y + drop_squares >= N as u8,
                    Direction::East => x + drop_squares >= N as u8,
                    Direction::South => drop_squares > y,
                    Direction::West => drop_squares > x,
                };
                if out_of_bounds {
                    return Err(PtnError::InvalidPly(
                        "Cannot spread out of bounds.".to_owned(),
                    ));
                }

                let mut drops_array = [0; N];
                drops_array[..drops.len()].copy_from_slice(&drops);

                let crush = annotations
                    .map(|a| a.contains(|c| c == '*'))
                    .unwrap_or_default();

                if crush && *drops.last().unwrap() > 1 {
                    return Err(PtnError::InvalidPly(
                        "Cannot crush with more than one stone.".to_owned(),
                    ));
                }

                Ok(Self::Spread {
                    x,
                    y,
                    direction,
                    drops: drops_array,
                    crush,
                })
            }
        }
    }
}

impl<const N: usize> FromStr for Ply<N> {
    type Err = PtnError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse::<PtnPly>().and_then(|p| p.try_into())
    }
}

impl<const N: usize> From<Ply<N>> for PtnPly {
    fn from(ply: Ply<N>) -> Self {
        match ply {
            Ply::Place { x, y, piece_type } => Self::Place {
                x,
                y,
                piece_type,
                annotations: None,
            },
            Ply::Spread {
                x,
                y,
                direction,
                drops,
                crush,
            } => Self::Spread {
                x,
                y,
                direction,
                drops: drops.into_iter().collect(),
                annotations: crush.then(|| "*".to_owned()),
            },
        }
    }
}

impl fmt::Display for PtnPly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Place {
                x,
                y,
                piece_type,
                annotations,
            } => {
                match piece_type {
                    PieceType::Flatstone => (),
                    PieceType::StandingStone => f.write_char('S')?,
                    PieceType::Capstone => f.write_char('C')?,
                }

                f.write_char(char::from_digit(*x as u32 + 10, 18).unwrap())?;
                f.write_char(char::from_digit(*y as u32 + 1, 10).unwrap())?;

                if let Some(annotations) = annotations {
                    f.write_str(annotations)?;
                }

                Ok(())
            }
            Self::Spread {
                x,
                y,
                direction,
                drops,
                annotations,
            } => {
                let count = drops.iter().sum::<u8>() as u32;
                if count > 1 {
                    f.write_char(char::from_digit(count, 10).unwrap())?;
                }

                f.write_char(char::from_digit(*x as u32 + 10, 18).unwrap())?;
                f.write_char(char::from_digit(*y as u32 + 1, 10).unwrap())?;

                f.write_char(match direction {
                    Direction::North => '+',
                    Direction::East => '>',
                    Direction::South => '-',
                    Direction::West => '<',
                })?;

                if drops.len() > 1 {
                    for &drop in drops {
                        f.write_char(char::from_digit(drop as u32, 10).unwrap())?;
                    }
                }

                if let Some(annotations) = annotations {
                    f.write_str(annotations)?;
                }

                Ok(())
            }
        }
    }
}

#[derive(Debug)]
pub enum PtnError {
    InputTooShort,
    InputTooLong,
    InvalidValue(&'static str),
    InvalidSpread(&'static str),
    IoError(String),
    MissingHeaders,
    InvalidHeader(String),
    InvalidPly(String),
    OutOfBounds(String),
    IllegalCarry(String),
}

impl From<IoError> for PtnError {
    fn from(error: IoError) -> Self {
        PtnError::IoError(error.to_string())
    }
}

static HEADER_SECTION: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(?:\[[^\]\n]+\]\n)+").unwrap());

static HEADER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\[(?P<key>[^\s]+) "(?P<value>.+)"\]$"#).unwrap());

static COMMENT_PATTERN: &str = r"\s*\{(?P<comment>[^}]*)\}";
static COMMENTS_PATTERN: &str = r"(?:\s*\{[^}]*\})+";

static BODY_COMMENT: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!(r"^{COMMENT_PATTERN}")).unwrap());

static COMMENT: Lazy<Regex> = Lazy::new(|| Regex::new(COMMENT_PATTERN).unwrap());

static TURN: Lazy<Regex> = Lazy::new(|| {
    let turn_number = r"^\s*(?P<turn_number>\d+)\.";
    // p1's move can have comments even if there is no ply.
    let p1_move = format!(
        r"(?:\.\.|\s+--|\s+(?P<p1_move>[FSCa-h1-8<>+\-?!*]+))(?P<p1_move_comments>{COMMENTS_PATTERN})?"
    );
    // p2's move must have a ply in order for there to be comments.
    let p2_move = format!(
        r"(?:\s+(?:--|(?P<p2_move>[FSCa-h1-8<>+\-?!*]+))(?P<p2_move_comments>{COMMENTS_PATTERN})?)?"
    );
    let end = r"\s+";

    Regex::new(&format!(r"{turn_number}{p1_move}{p2_move}{end}")).unwrap()
});

static PLY: Lazy<Regex> = Lazy::new(|| {
    let place = r"(?P<place_type>[FSC])?(?P<place_file>[a-h])(?P<place_rank>[1-8])";
    let spread = r"(?P<carry>\d)?(?P<spread_file>[a-h])(?P<spread_rank>[1-8])(?P<direction>[><+-])(?P<drops>\d+)?(?P<spread_type>[FSC])?";
    let annotations = r"(?P<annotations>[?!*]+)?";

    Regex::new(&format!("^(?:{place}|{spread}){annotations}$")).unwrap()
});

static RESULT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?:R-0|0-R|F-0|0-F|1-0|0-1|1/2-1/2)").unwrap());

#[cfg(test)]
mod tests {
    use super::*;

    use PieceType::*;

    #[test]
    fn coordinates_are_in_bounds() {
        assert_eq!(
            "a1".parse::<Ply::<3>>().unwrap(),
            Ply::<3>::Place {
                x: 0,
                y: 0,
                piece_type: Flatstone
            }
        );

        assert_eq!(
            "c3".parse::<Ply::<3>>().unwrap(),
            Ply::<3>::Place {
                x: 2,
                y: 2,
                piece_type: Flatstone
            }
        );

        assert!("d1".parse::<Ply<3>>().is_err());

        assert!("a4".parse::<Ply<3>>().is_err());
    }

    #[test]
    fn place_piece_types() {
        assert_eq!(
            "a1".parse::<Ply::<3>>().unwrap(),
            Ply::<3>::Place {
                x: 0,
                y: 0,
                piece_type: Flatstone
            }
        );

        assert_eq!(
            "Fa1".parse::<Ply::<3>>().unwrap(),
            Ply::<3>::Place {
                x: 0,
                y: 0,
                piece_type: Flatstone
            }
        );

        assert_eq!(
            "Sa1".parse::<Ply::<3>>().unwrap(),
            Ply::<3>::Place {
                x: 0,
                y: 0,
                piece_type: StandingStone
            }
        );

        assert_eq!(
            "Ca1".parse::<Ply::<3>>().unwrap(),
            Ply::<3>::Place {
                x: 0,
                y: 0,
                piece_type: Capstone
            }
        );
    }

    #[test]
    fn spread_directions() {
        assert_eq!(
            "c3+".parse::<Ply::<5>>().unwrap(),
            Ply::<5>::Spread {
                x: 2,
                y: 2,
                direction: Direction::North,
                drops: [1, 0, 0, 0, 0],
                crush: false,
            },
        );

        assert_eq!(
            "c3>".parse::<Ply::<5>>().unwrap(),
            Ply::<5>::Spread {
                x: 2,
                y: 2,
                direction: Direction::East,
                drops: [1, 0, 0, 0, 0],
                crush: false,
            },
        );

        assert_eq!(
            "c3-".parse::<Ply::<5>>().unwrap(),
            Ply::<5>::Spread {
                x: 2,
                y: 2,
                direction: Direction::South,
                drops: [1, 0, 0, 0, 0],
                crush: false,
            },
        );

        assert_eq!(
            "c3<".parse::<Ply::<5>>().unwrap(),
            Ply::<5>::Spread {
                x: 2,
                y: 2,
                direction: Direction::West,
                drops: [1, 0, 0, 0, 0],
                crush: false,
            },
        );
    }

    #[test]
    fn spread_amounts() {
        assert!("4a1+".parse::<Ply<3>>().is_err());

        assert_eq!(
            "a3>".parse::<Ply::<5>>().unwrap(),
            Ply::<5>::Spread {
                x: 0,
                y: 2,
                direction: Direction::East,
                drops: [1, 0, 0, 0, 0],
                crush: false,
            },
        );

        assert_eq!(
            "a3>1".parse::<Ply::<5>>().unwrap(),
            Ply::<5>::Spread {
                x: 0,
                y: 2,
                direction: Direction::East,
                drops: [1, 0, 0, 0, 0],
                crush: false,
            },
        );

        assert_eq!(
            "3a3>12".parse::<Ply::<5>>().unwrap(),
            Ply::<5>::Spread {
                x: 0,
                y: 2,
                direction: Direction::East,
                drops: [1, 2, 0, 0, 0],
                crush: false,
            },
        );

        assert!("3a3>22".parse::<Ply<5>>().is_err());
    }

    #[test]
    fn spread_bounds() {
        assert!("a3+".parse::<Ply<3>>().is_err());

        assert!("3a1>111".parse::<Ply<3>>().is_err());

        assert!("a1-".parse::<Ply<3>>().is_err());

        assert!("2b1<11".parse::<Ply<3>>().is_err());
    }

    #[test]
    fn crushes() {
        assert_eq!(
            "3a3>21*".parse::<Ply::<5>>().unwrap(),
            Ply::<5>::Spread {
                x: 0,
                y: 2,
                direction: Direction::East,
                drops: [2, 1, 0, 0, 0],
                crush: true,
            },
        );

        assert!("3a3>12*".parse::<Ply<5>>().is_err());
    }
}
