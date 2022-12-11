use std::cmp::Ordering::*;
use std::convert::TryFrom;

use crate::piece::{Color, Piece, PieceType};
use crate::stack::Stack;
use crate::state::State;

pub struct Tps(String);

impl Tps {
    pub fn new(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl<const N: usize> TryFrom<Tps> for State<N> {
    type Error = TpsError;

    fn try_from(value: Tps) -> Result<Self, Self::Error> {
        if !value.0.starts_with("[TPS \"") || !value.0.ends_with("\"]") {
            return Err(TpsError::Tag);
        }

        let body = &value.0[6..value.0.len() - 2];
        let mut segments = body.split(' ');
        let board = segments.next().ok_or(TpsError::Body)?;
        let next_player = segments.next().ok_or(TpsError::Body)?;
        let turn = segments.next().ok_or(TpsError::Body)?;

        let mut state = Self::default();

        let mut rows = board.split('/');
        for y in (0..N).rev() {
            let row = rows
                .next()
                .ok_or(TpsError::Board("Not enough rows in board."))?;

            let mut x = 0;
            for next_columns in row.split(',') {
                if x >= N {
                    return Err(TpsError::Board("Too many columns in board."));
                }

                if let Some(space_count) = next_columns.strip_prefix('x') {
                    if space_count.is_empty() {
                        x += 1;
                    } else if let Ok(spaces) = space_count.parse::<usize>() {
                        x += spaces;
                    } else {
                        return Err(TpsError::Value("Expected space count."));
                    }
                } else {
                    let mut stack = Stack::default();
                    let mut next_color = None;

                    for character in next_columns.chars() {
                        if matches!(
                            stack.last_piece_type(),
                            Some(PieceType::StandingStone | PieceType::Capstone)
                        ) {
                            return Err(TpsError::Value(
                                "Stack must end after a standing stone or capstone.",
                            ));
                        }

                        match character {
                            '1' => {
                                if let Some(color) = next_color {
                                    stack.add_piece(Piece::new(PieceType::Flatstone, color));
                                }
                                next_color = Some(Color::White);
                            }
                            '2' => {
                                if let Some(color) = next_color {
                                    stack.add_piece(Piece::new(PieceType::Flatstone, color));
                                }
                                next_color = Some(Color::Black);
                            }
                            'S' => {
                                if next_color.is_none() {
                                    return Err(TpsError::Value(
                                        "A player number must be specified before an S.",
                                    ));
                                }
                                stack.add_piece(Piece::new(
                                    PieceType::StandingStone,
                                    next_color.unwrap(),
                                ));
                                next_color = None;
                            }
                            'C' => {
                                if next_color.is_none() {
                                    return Err(TpsError::Value(
                                        "A player number must be specified before a C.",
                                    ));
                                }
                                stack.add_piece(Piece::new(
                                    PieceType::Capstone,
                                    next_color.unwrap(),
                                ));
                                next_color = None;
                            }
                            _ => return Err(TpsError::Value("Unexpected character.")),
                        }
                    }

                    if let Some(color) = next_color {
                        stack.add_piece(Piece::new(PieceType::Flatstone, color));
                    }

                    state.board[x][y] = stack;
                    x += 1;
                }
            }

            match x.cmp(&N) {
                Greater => return Err(TpsError::Board("Too many columns in row.")),
                Less => return Err(TpsError::Board("Not enough columns in row.")),
                _ => (),
            }
        }

        if rows.next().is_some() {
            return Err(TpsError::Board("Too many rows in board."));
        }

        let turn_number = turn
            .parse::<u16>()
            .map_err(|_| TpsError::Value("Turn number must be a valid number."))?;
        if turn_number == 0 {
            return Err(TpsError::Value("Turn number must be greater than zero."));
        }

        state.ply_count = match next_player {
            "1" => (turn_number - 1) * 2,
            "2" => (turn_number - 1) * 2 + 1,
            _ => return Err(TpsError::Player),
        };

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
                        return Err(TpsError::Board("Too many pieces for player."));
                    }

                    *count -= 1;
                }
            }
        }

        state.recalculate_metadata();

        Ok(state)
    }
}

impl<const N: usize> TryFrom<&str> for State<N> {
    type Error = TpsError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Tps::new(value).try_into()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum TpsError {
    Tag,
    Body,
    Board(&'static str),
    Value(&'static str),
    Player,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;

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
            r#"[TPS "x,22S,22C,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"]"#.try_into(),
            Ok(test_state()),
        )
    }

    #[test]
    fn incorrect_state() {
        assert_eq!(
            <&str as TryInto<State<5>>>::try_into(
                r#"[TPF "x,22S,22C,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"]"#
            ),
            Err(TpsError::Tag),
        );

        assert_eq!(
            <&str as TryInto<State<5>>>::try_into(
                r#"[TPS "x,22S,22C,11,21/x6/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"]"#
            ),
            Err(TpsError::Board("Too many columns in row.")),
        );

        assert_eq!(
            <&str as TryInto<State<5>>>::try_into(
                r#"[TPS "x,22S,22C,11,21/x4/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"]"#
            ),
            Err(TpsError::Board("Not enough columns in row.")),
        );

        assert_eq!(
            <&str as TryInto<State<5>>>::try_into(
                r#"[TPS "x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"]"#
            ),
            Err(TpsError::Board("Not enough rows in board.")),
        );

        assert_eq!(
            <&str as TryInto<State<5>>>::try_into(
                r#"[TPS "/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"]"#
            ),
            Err(TpsError::Board("Not enough columns in row.")),
        );

        assert_eq!(
            <&str as TryInto<State<5>>>::try_into(
                r#"[TPS "x,S,22C,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"]"#
            ),
            Err(TpsError::Value(
                "A player number must be specified before an S."
            )),
        );

        assert_eq!(
            <&str as TryInto<State<5>>>::try_into(
                r#"[TPS "x,22S,C,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"]"#
            ),
            Err(TpsError::Value(
                "A player number must be specified before a C."
            )),
        );

        assert_eq!(
            <&str as TryInto<State<5>>>::try_into(
                r#"[TPS "x,22S,22C1,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"]"#
            ),
            Err(TpsError::Value(
                "Stack must end after a standing stone or capstone."
            )),
        );

        assert_eq!(
            <&str as TryInto<State<5>>>::try_into(
                r#"[TPS "x,22S,22CS,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 1 26"]"#
            ),
            Err(TpsError::Value(
                "Stack must end after a standing stone or capstone."
            )),
        );

        assert_eq!(
            <&str as TryInto<State<5>>>::try_into(
                r#"[TPS "x,22S,22C,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 3 26"]"#
            ),
            Err(TpsError::Player),
        );

        assert_eq!(
            <&str as TryInto<State<5>>>::try_into(
                r#"[TPS "x,22S,22C,11,21/x5/121,212,12,1121C,1212S/21S,1,21,211S,12S/x,21S,2,x2 3 0"]"#
            ),
            Err(TpsError::Value("Turn number must be greater than zero.")),
        );
    }
}
