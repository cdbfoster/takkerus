use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;
use std::vec;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::piece::{Color, Piece, PieceType};
use crate::stack::Stack;
use crate::state::State;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tps {
    pub board: Vec<Vec<Stack>>,
    pub to_move: Color,
    pub turn: u32,
}

impl Tps {
    pub fn size(&self) -> usize {
        self.board.len()
    }
}

impl FromStr for Tps {
    type Err = TpsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut segments = s.split(' ');

        let mut board_segment = segments.next().ok_or(TpsError::ExpectedBoard)?;
        let mut board = vec![vec![]];

        while let Some(c) = BOARD_ELEMENT.captures(board_segment) {
            if c.name("space").is_some() {
                let count = c
                    .name("repeat")
                    .map(|s| s.as_str().parse::<usize>().unwrap())
                    .unwrap_or(1);
                board
                    .last_mut()
                    .unwrap()
                    .extend(std::iter::repeat(Stack::default()).take(count));
            } else if let Some(s) = c.name("stack") {
                let mut stack = Stack::default();
                let mut stones = s.as_str().chars().peekable();

                while let Some(stone) = stones.next() {
                    let color = match stone {
                        '1' => Color::White,
                        '2' => Color::Black,
                        _ => break,
                    };

                    match stones.peek() {
                        Some('S') => stack.add_piece(Piece::new(PieceType::StandingStone, color)),
                        Some('C') => stack.add_piece(Piece::new(PieceType::Capstone, color)),
                        _ => stack.add_piece(Piece::new(PieceType::Flatstone, color)),
                    }
                }

                board.last_mut().unwrap().push(stack);
            }

            match c.name("end").map(|s| s.as_str()) {
                Some(",") => (),
                Some("/") => board.push(Vec::new()),
                _ => break,
            }

            board_segment = &board_segment[c.get(0).unwrap().end()..];
        }

        if !board.iter().all(|row| row.len() == board.len()) {
            return Err(TpsError::Dimensions(format!(
                "Column count does not equal row count: {}",
                board.len()
            )));
        }

        let player_segment = segments.next().ok_or(TpsError::ExpectedPlayer)?;
        let to_move = match player_segment {
            "1" => Color::White,
            "2" => Color::Black,
            _ => return Err(TpsError::InvalidPlayer(player_segment.to_owned())),
        };

        let turn_segment = segments.next().ok_or(TpsError::ExpectedTurn)?;
        let turn = turn_segment
            .parse()
            .map_err(|_| TpsError::InvalidTurn(turn_segment.to_owned()))?;
        if turn == 0 {
            return Err(TpsError::InvalidTurn(turn_segment.to_owned()));
        }

        Ok(Tps {
            board,
            to_move,
            turn,
        })
    }
}

impl<const N: usize> TryFrom<Tps> for State<N> {
    type Error = TpsError;

    fn try_from(tps: Tps) -> Result<Self, Self::Error> {
        let mut state = Self::default();

        if tps.size() != N {
            return Err(TpsError::Dimensions(format!(
                "TPS board size doesn't match state board size: {} != {N}",
                tps.size()
            )));
        }

        for x in 0..N {
            for y in 0..N {
                state.board[x][y] = tps.board[N - y - 1][x];
            }
        }

        state.ply_count = match tps.to_move {
            Color::White => (tps.turn - 1) * 2,
            Color::Black => (tps.turn - 1) * 2 + 1,
        } as u16;

        for x in 0..N {
            for y in 0..N {
                for piece in state.board[x][y].iter() {
                    let counts = match piece.color() {
                        Color::White => (&mut state.p1_flatstones, &mut state.p1_capstones),
                        Color::Black => (&mut state.p2_flatstones, &mut state.p2_capstones),
                    };

                    let count = match piece.piece_type() {
                        PieceType::Capstone => counts.1,
                        _ => counts.0,
                    };

                    if *count == 0 {
                        return Err(TpsError::InvalidBoard(format!(
                            "{:?} has too many pieces.",
                            piece.color()
                        )));
                    }

                    *count -= 1;
                }
            }
        }

        state.recalculate_metadata();

        Ok(state)
    }
}

impl<const N: usize> FromStr for State<N> {
    type Err = TpsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<Tps>()?.try_into()
    }
}

impl<const N: usize> From<State<N>> for Tps {
    fn from(state: State<N>) -> Self {
        let mut board = Vec::new();
        for y in 0..N {
            board.push(Vec::new());
            for x in 0..N {
                board[y].push(state.board[x][N - y - 1]);
            }
        }

        Self {
            board,
            to_move: state.to_move(),
            turn: state.ply_count as u32 / 2 + 1,
        }
    }
}

impl fmt::Display for Tps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for y in 0..self.size() {
            if y != 0 {
                write!(f, "/")?;
            }

            let mut first_write = true;
            let mut empty_count = 0;
            for x in 0..self.board[y].len() {
                if self.board[y][x].is_empty() {
                    empty_count += 1;
                } else {
                    if !first_write {
                        write!(f, ",")?;
                    }

                    if empty_count > 0 {
                        write!(f, "x")?;
                        if empty_count > 1 {
                            write!(f, "{empty_count}")?;
                        }
                        write!(f, ",")?;
                        empty_count = 0;
                    }

                    for piece in self.board[y][x].iter().rev() {
                        match piece.color() {
                            Color::White => write!(f, "1")?,
                            Color::Black => write!(f, "2")?,
                        }
                        match piece.piece_type() {
                            PieceType::StandingStone => write!(f, "S")?,
                            PieceType::Capstone => write!(f, "C")?,
                            _ => (),
                        }
                    }

                    first_write = false;
                }
            }

            if empty_count > 0 {
                if !first_write {
                    write!(f, ",")?;
                }

                write!(f, "x")?;
                if empty_count > 1 {
                    write!(f, "{empty_count}")?;
                }
            }
        }

        match self.to_move {
            Color::White => write!(f, " 1 {}", self.turn),
            Color::Black => write!(f, " 2 {}", self.turn),
        }
    }
}

#[derive(Debug)]
pub enum TpsError {
    ExpectedBoard,
    ExpectedPlayer,
    ExpectedTurn,
    Dimensions(String),
    InvalidBoard(String),
    InvalidPlayer(String),
    InvalidTurn(String),
}

static BOARD_ELEMENT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:(?P<space>x(?P<repeat>\d)?)|(?P<stack>[12]+[SC]?))(?:(?P<end>[,/])|$)")
        .unwrap()
});

#[cfg(test)]
mod tests {
    use super::*;

    use Color::*;
    use PieceType::*;

    fn test_state() -> State<5> {
        let mut state = State::default();

        state.board[1][4].add_piece(Piece::new(Flatstone, Black));
        state.board[1][4].add_piece(Piece::new(StandingStone, Black));
        state.board[2][4].add_piece(Piece::new(Flatstone, Black));
        state.board[2][4].add_piece(Piece::new(Capstone, Black));
        state.board[3][4].add_piece(Piece::new(Flatstone, White));
        state.board[3][4].add_piece(Piece::new(Flatstone, White));
        state.board[4][4].add_piece(Piece::new(Flatstone, Black));
        state.board[4][4].add_piece(Piece::new(Flatstone, White));

        state.board[0][2].add_piece(Piece::new(Flatstone, White));
        state.board[0][2].add_piece(Piece::new(Flatstone, Black));
        state.board[0][2].add_piece(Piece::new(Flatstone, White));
        state.board[1][2].add_piece(Piece::new(Flatstone, Black));
        state.board[1][2].add_piece(Piece::new(Flatstone, White));
        state.board[1][2].add_piece(Piece::new(Flatstone, Black));
        state.board[2][2].add_piece(Piece::new(Flatstone, White));
        state.board[2][2].add_piece(Piece::new(Flatstone, Black));
        state.board[3][2].add_piece(Piece::new(Flatstone, White));
        state.board[3][2].add_piece(Piece::new(Flatstone, White));
        state.board[3][2].add_piece(Piece::new(Flatstone, Black));
        state.board[3][2].add_piece(Piece::new(Capstone, White));
        state.board[4][2].add_piece(Piece::new(Flatstone, White));
        state.board[4][2].add_piece(Piece::new(Flatstone, Black));
        state.board[4][2].add_piece(Piece::new(Flatstone, White));
        state.board[4][2].add_piece(Piece::new(StandingStone, Black));

        state.board[0][1].add_piece(Piece::new(Flatstone, Black));
        state.board[0][1].add_piece(Piece::new(StandingStone, White));
        state.board[1][1].add_piece(Piece::new(Flatstone, White));
        state.board[2][1].add_piece(Piece::new(Flatstone, Black));
        state.board[2][1].add_piece(Piece::new(Flatstone, White));
        state.board[3][1].add_piece(Piece::new(Flatstone, Black));
        state.board[3][1].add_piece(Piece::new(Flatstone, White));
        state.board[3][1].add_piece(Piece::new(StandingStone, White));
        state.board[4][1].add_piece(Piece::new(Flatstone, White));
        state.board[4][1].add_piece(Piece::new(StandingStone, Black));

        state.board[1][0].add_piece(Piece::new(Flatstone, Black));
        state.board[1][0].add_piece(Piece::new(StandingStone, White));
        state.board[2][0].add_piece(Piece::new(Flatstone, Black));

        state.p1_flatstones = 3;
        state.p1_capstones = 0;
        state.p2_flatstones = 4;
        state.p2_capstones = 0;

        state.ply_count = 50;

        state.recalculate_metadata();

        state
    }

    #[test]
    fn correct_state() {
        assert_eq!(
            "x,22S,22C,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"
                .parse::<State<5>>()
                .unwrap(),
            test_state(),
        )
    }

    #[test]
    fn incorrect_state() {
        // Too many columns in a row.
        assert!(
            "x,22S,22C,11,21/x6/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"
                .parse::<State<5>>()
                .is_err()
        );

        // Not enough columns in a row.
        assert!(
            "x,22S,22C,11,21/x4/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"
                .parse::<State<5>>()
                .is_err()
        );

        // Not enough rows.
        assert!(
            "x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"
                .parse::<State<5>>()
                .is_err()
        );

        // Invalid element.
        assert!(
            "/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"
                .parse::<State<5>>()
                .is_err()
        );

        // Invalid element.
        assert!(
            "x,S,22C,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"
                .parse::<State<5>>()
                .is_err()
        );

        // Invalid element.
        assert!(
            "x,22S,C,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"
                .parse::<State<5>>()
                .is_err()
        );

        // Invalid stack.
        assert!(
            "x,22S,22C1,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"
                .parse::<State<5>>()
                .is_err()
        );

        // Invalid stack.
        assert!(
            "x,22S,22CS,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"
                .parse::<State<5>>()
                .is_err()
        );

        // Invalid player.
        assert!(
            "x,22S,22C,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 3 26"
                .parse::<State<5>>()
                .is_err()
        );

        // Invalid turn.
        assert!(
            "x,22S,22C,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 3 0"
                .parse::<State<5>>()
                .is_err()
        );
    }

    #[test]
    fn correct_tps() {
        let tps: Tps = test_state().into();

        assert_eq!(
            tps.to_string(),
            "x,22S,22C,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26",
        )
    }
}
