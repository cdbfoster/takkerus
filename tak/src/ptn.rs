use std::convert::TryFrom;
use std::fmt::{self, Write};
use std::fs::File;
use std::io::{Error as IoError, Read, Write as IoWrite};
use std::path::Path;
use std::str::FromStr;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::piece::{Color, PieceType};
use crate::ply::{Direction, Ply};
use crate::state::{HalfKomi, State, StateError};
use crate::tps::TpsError;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
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

    pub fn to_file(&self, filename: impl AsRef<Path>) -> Result<(), IoError> {
        let mut f = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(filename)?;

        write!(f, "{self}")
    }

    pub fn get_header(&self, key: &str) -> Option<&PtnHeader> {
        self.headers.iter().find(|h| h.key == key)
    }

    pub fn add_header(&mut self, key: &str, value: impl fmt::Display) {
        let value = format!("{value}");

        if let Some(header) = self.headers.iter_mut().find(|h| h.key == key) {
            header.value = value;
        } else {
            self.headers.push(PtnHeader {
                key: key.to_owned(),
                value,
            });
        }
    }

    pub fn remove_header(&mut self, key: &str) {
        if let Some(i) = self.headers.iter().position(|h| h.key == key) {
            self.headers.remove(i);
        }
    }

    pub fn get_plies<const N: usize>(&self) -> Result<Vec<Ply<N>>, PtnError> {
        self.turns
            .iter()
            .flat_map(|t| [t.p1_move.ply.clone(), t.p2_move.ply.clone()])
            .filter_map(|p| p.map(|p| p.try_into()))
            .collect()
    }

    pub fn add_ply<const N: usize>(&mut self, ply: Ply<N>) -> Result<(), PtnError> {
        let state: State<N> = self.clone().try_into()?;
        state.validate_ply(ply)?;

        let ptn_ply: PtnPly = ply.into();

        if let Some(turn) = self.turns.last_mut() {
            let number = turn.number;
            if turn.p2_move.ply.is_none() {
                turn.p2_move.ply = Some(ptn_ply);
            } else {
                self.turns.push(PtnTurn {
                    number: number + 1,
                    p1_move: PtnMove {
                        ply: Some(ptn_ply),
                        ..Default::default()
                    },
                    p2_move: PtnMove::default(),
                });
            }
        } else {
            let number = state.ply_count as u32 / 2 + 1;

            let ptn_move = PtnMove {
                ply: Some(ptn_ply),
                ..Default::default()
            };

            let mut ptn_turn = PtnTurn {
                number,
                ..Default::default()
            };

            match state.to_move() {
                Color::White => ptn_turn.p1_move = ptn_move,
                Color::Black => ptn_turn.p2_move = ptn_move,
            };

            self.turns.push(ptn_turn);
        }

        self.update_result::<N>()
    }

    pub fn remove_last_ply<const N: usize>(&mut self) -> Result<(), PtnError> {
        if let Some(turn) = self.turns.last_mut() {
            if turn.p2_move.ply.is_some() {
                turn.p2_move.ply = None;
            } else if turn.p1_move.ply.is_some() {
                turn.p1_move.ply = None;
            } else {
                // Somehow there was a blank turn on the end, so remove it and try again.
                self.turns.pop();
                return self.remove_last_ply::<N>();
            }

            if turn.p1_move.ply.is_none() && turn.p2_move.ply.is_none() {
                self.turns.pop();
            }
        }

        self.update_result::<N>()
    }

    pub fn validate<const N: usize>(&self) -> Result<(), PtnError> {
        let _state: State<N> = self.clone().try_into()?;
        Ok(())
    }
}

impl PtnGame {
    fn update_result<const N: usize>(&mut self) -> Result<(), PtnError> {
        let state: State<N> = self.clone().try_into()?;

        if let Some(resolution) = state.resolution() {
            self.add_header("Result", resolution);
            self.result = Some(resolution.to_string());
        } else {
            self.remove_header("Result");
            self.result = None;
        }

        Ok(())
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

impl<const N: usize> TryFrom<PtnGame> for State<N> {
    type Error = PtnError;

    fn try_from(ptn: PtnGame) -> Result<Self, Self::Error> {
        let size_header = ptn
            .get_header("Size")
            .map(PtnHeader::parse_value)
            .transpose()?
            .unwrap_or(N);

        if size_header != N {
            return Err(PtnError::IncorrectSize(format!(
                "Size header is {size_header} but requested size is {N}."
            )));
        }

        let mut state = ptn
            .get_header("TPS")
            .map(|h| h.value.parse::<Self>())
            .transpose()?
            .unwrap_or_default();

        let komi_header = ptn
            .get_header("Komi")
            .map(PtnHeader::parse_value::<HalfKomi>)
            .transpose()?;

        if let Some(half_komi) = komi_header {
            state.half_komi = half_komi;
        }

        let plies = ptn.turns.iter().cloned().flat_map(|t| {
            [
                (t.number, Color::White, t.p1_move),
                (t.number, Color::Black, t.p2_move),
            ]
        });

        for (turn, color, PtnMove { ply, .. }) in plies {
            let current_turn = state.ply_count as u32 / 2 + 1;

            if turn != current_turn {
                return Err(PtnError::IncorrectTurn(format!(
                    "Stated turn is {turn} but should be {current_turn}."
                )));
            }

            if let Some(ptn_ply) = ply {
                if state.to_move() == color {
                    let ply: Ply<N> = ptn_ply.try_into()?;
                    state.execute_ply(ply)?;
                } else {
                    return Err(PtnError::InvalidPly("Incorrect player to move.".to_owned()));
                }
            }
        }

        fn validate_result(r: &str) -> Result<&str, PtnError> {
            match r {
                "R-0" | "0-R" | "F-0" | "0-F" | "1-0" | "0-1" | "1/2-1/2" => Ok(r),
                _ => Err(PtnError::InvalidResult(r.to_owned())),
            }
        }

        let result_header = ptn
            .get_header("Result")
            .map(|h| validate_result(&h.value))
            .transpose()?;

        let result_stated = ptn.result.as_deref().map(validate_result).transpose()?;

        let result_resolution = state.resolution().map(|r| r.to_string());

        let correct = match (result_header, result_stated, result_resolution) {
            (Some(a), Some(b), Some(c)) => a == b && b == c,
            (Some(a), Some(b), _) => a == b,
            (Some(a), _, Some(c)) => a == c,
            (_, Some(b), Some(c)) => b == c,
            _ => true,
        };

        if !correct {
            return Err(PtnError::IncorrectResult("Results disagree.".to_owned()));
        }

        Ok(state)
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

impl PtnHeader {
    pub fn new(key: &str, value: &str) -> Self {
        Self {
            key: key.to_owned(),
            value: value.to_owned(),
        }
    }

    pub fn parse_value<T: FromStr>(&self) -> Result<T, PtnError> {
        self.value
            .parse()
            .map_err(|_| PtnError::InvalidHeader(self.to_string()))
    }
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
                .unwrap_or_else(Vec::new);
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
                    .unwrap_or_else(Vec::new),
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

    fn try_from(ply: PtnPly) -> Result<Self, Self::Error> {
        // Validate x and y.
        let (x, y) = match &ply {
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

        match ply {
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

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<PtnPly>().and_then(|p| p.try_into())
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
                drops: drops.into_iter().filter(|d| *d != 0).collect(),
                annotations: crush.then(|| "*".to_owned()),
            },
        }
    }
}

impl fmt::Display for PtnPly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut b = String::new();

        match self {
            Self::Place {
                x,
                y,
                piece_type,
                annotations,
            } => {
                match piece_type {
                    PieceType::Flatstone => (),
                    PieceType::StandingStone => b.write_char('S')?,
                    PieceType::Capstone => b.write_char('C')?,
                }

                b.write_char(char::from_digit(*x as u32 + 10, 18).unwrap())?;
                b.write_char(char::from_digit(*y as u32 + 1, 10).unwrap())?;

                if let Some(annotations) = annotations {
                    b.write_str(annotations)?;
                }
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
                    b.write_char(char::from_digit(count, 10).unwrap())?;
                }

                b.write_char(char::from_digit(*x as u32 + 10, 18).unwrap())?;
                b.write_char(char::from_digit(*y as u32 + 1, 10).unwrap())?;

                b.write_char(match direction {
                    Direction::North => '+',
                    Direction::East => '>',
                    Direction::South => '-',
                    Direction::West => '<',
                })?;

                if drops.len() > 1 {
                    for &drop in drops {
                        b.write_char(char::from_digit(drop as u32, 10).unwrap())?;
                    }
                }

                if let Some(annotations) = annotations {
                    b.write_str(annotations)?;
                }
            }
        }

        f.pad(&b)
    }
}

#[derive(Debug)]
pub enum PtnError {
    IoError(String),
    InvalidHeader(String),
    InvalidPly(String),
    InvalidResult(String),
    OutOfBounds(String),
    IllegalCarry(String),
    IncorrectSize(String),
    IncorrectTurn(String),
    IncorrectResult(String),
    TpsError(TpsError),
    StateError(StateError),
}

impl From<IoError> for PtnError {
    fn from(error: IoError) -> Self {
        PtnError::IoError(error.to_string())
    }
}

impl From<TpsError> for PtnError {
    fn from(error: TpsError) -> Self {
        PtnError::TpsError(error)
    }
}

impl From<StateError> for PtnError {
    fn from(error: StateError) -> Self {
        PtnError::StateError(error)
    }
}

static HEADER_SECTION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?:\[[^\]\n]+\](?:\n|$))+").unwrap());

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
    let end = r"(?:\s+|$)";

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

    use crate::tps::Tps;

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

    #[test]
    fn ptn_with_tps_to_state() {
        let ptn = r#"[Size "3"]
[TPS "2,1,x/1,x2/x3 2 2"]"#;

        let game: PtnGame = ptn.parse().unwrap();
        let state: State<3> = game.try_into().unwrap();
        let tps: Tps = state.into();

        assert_eq!(tps.to_string(), "2,1,x/1,x2/x3 2 2");
    }

    #[test]
    fn ptn_with_tps_and_moves_to_state() {
        let ptn = r#"[Size "3"]
[TPS "2,1,x/1,x2/x3 2 2"]
2. -- b2 3. a2+ b2+ 4. 2a3>"#;

        let game: PtnGame = ptn.parse().unwrap();
        let state: State<3> = game.try_into().unwrap();
        let tps: Tps = state.into();

        assert_eq!(tps.to_string(), "x,1221,x/x3/x3 2 4");
    }

    #[test]
    fn add_plies_to_ptn() {
        let ptn = r#"[Size "3"]
[TPS "2,1,x/1,x2/x3 2 2"]"#;

        let mut game: PtnGame = ptn.parse().unwrap();
        game.add_ply("b2".parse::<Ply<3>>().unwrap()).unwrap();
        game.add_ply("a2+".parse::<Ply<3>>().unwrap()).unwrap();
        game.add_ply("b2+".parse::<Ply<3>>().unwrap()).unwrap();
        game.add_ply("2a3>".parse::<Ply<3>>().unwrap()).unwrap();

        assert_eq!(
            game.to_string(),
            r#"[Size "3"]
[TPS "2,1,x/1,x2/x3 2 2"]

2. -- b2
3. a2+ b2+
4. 2a3>"#,
        );
    }

    #[test]
    fn remove_last_ply_from_ptn() {
        let ptn = r#"[Size "3"]
[TPS "2,1,x/1,x2/x3 2 2"]
2. -- b2 3. a2+ b2+ 4. 2a3>"#;

        let mut game: PtnGame = ptn.parse().unwrap();
        game.remove_last_ply::<3>().unwrap();

        assert_eq!(
            game.to_string(),
            r#"[Size "3"]
[TPS "2,1,x/1,x2/x3 2 2"]

2. -- b2
3. a2+ b2+"#,
        );

        game.remove_last_ply::<3>().unwrap();

        assert_eq!(
            game.to_string(),
            r#"[Size "3"]
[TPS "2,1,x/1,x2/x3 2 2"]

2. -- b2
3. a2+"#,
        );
    }
}
