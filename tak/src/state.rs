use std::cmp::Ordering::*;
use std::fmt;
use std::ops::Neg;
use std::str::FromStr;

use tracing::{instrument, trace};

use crate::bitmap::{board_mask, edge_masks, Bitmap};
use crate::metadata::Metadata;
use crate::piece::{Color, Piece, PieceType};
use crate::ply::{Direction, Ply, PlyError};
use crate::stack::Stack;
use crate::tps::Tps;
use crate::zobrist::{zobrist_advance_move, zobrist_hash_stack, zobrist_hash_state};

#[derive(Clone, Eq, PartialEq)]
pub struct State<const N: usize> {
    pub p1_flatstones: u8,
    pub p1_capstones: u8,

    pub p2_flatstones: u8,
    pub p2_capstones: u8,

    /// Column-major left-to-right, rows bottom-to-top.
    pub board: [[Stack; N]; N],

    pub ply_count: u16,

    pub komi: Komi,

    pub metadata: Metadata<N>,
}

impl<const N: usize> Default for State<N> {
    fn default() -> Self {
        let (flatstones, capstones) = match N {
            3 => (10, 0),
            4 => (15, 0),
            5 => (21, 1),
            6 => (30, 1),
            7 => (40, 2),
            8 => (50, 2),
            _ => panic!("invalid board size"),
        };

        Self {
            p1_flatstones: flatstones,
            p1_capstones: capstones,
            p2_flatstones: flatstones,
            p2_capstones: capstones,
            board: [[Stack::default(); N]; N],
            ply_count: 0,
            komi: Komi::default(),
            metadata: Metadata::default(),
        }
    }
}

impl<const N: usize> State<N> {
    pub fn to_move(&self) -> Color {
        if self.ply_count % 2 == 0 {
            Color::White
        } else {
            Color::Black
        }
    }

    #[instrument(level = "trace", skip(self))]
    pub fn validate_ply(&self, mut ply: Ply<N>) -> Result<Ply<N>, StateError> {
        use Color::*;
        use PieceType::*;

        ply.validate()?;

        let player_color = self.to_move();

        match ply {
            Ply::Place { x, y, piece_type } => {
                // The board space must be empty.
                if !self.board[x as usize][y as usize].is_empty() {
                    return Err(StateError::InvalidPlace("Board space is occupied."));
                }

                // Determine piece color.
                let color = if self.ply_count >= 2 {
                    player_color
                } else {
                    player_color.other()
                };

                let player_counts = match color {
                    White => (&self.p1_flatstones, &self.p1_capstones),
                    Black => (&self.p2_flatstones, &self.p2_capstones),
                };

                let selected_count = match piece_type {
                    Capstone => player_counts.1,
                    _ => player_counts.0,
                };

                // Must have enough of the stone we're trying to place.
                if *selected_count == 0 {
                    return Err(StateError::InvalidPlace(
                        "Insufficient reserve for placement.",
                    ));
                }
            }
            Ply::Spread {
                x,
                y,
                direction,
                drops,
                crush,
            } => {
                // Board space must not be empty.
                let stack = &self.board[x as usize][y as usize];
                if stack.is_empty() {
                    return Err(StateError::InvalidSpread("Board space is empty."));
                }

                // Stack must be controlled by this player.
                let top_piece = stack.last().unwrap();
                if top_piece.color() != player_color {
                    return Err(StateError::InvalidSpread(
                        "Cannot move an opponent's piece.",
                    ));
                }

                let drop_count = drops.into_iter().position(|d| d == 0).unwrap();

                // Must not carry more than the size of the size of the stack.
                let carry_total = drops.iter().sum::<u8>() as usize;
                if carry_total > stack.len() {
                    return Err(StateError::InvalidSpread("Illegal carry amount."));
                }

                // Validate the spread stays in bounds, and doesn't go over a blocking piece
                // unless we're crushing a standing stone.
                let (dx, dy) = direction.to_offset();
                let (mut tx, mut ty) = (x as i8, y as i8);
                let mut valid_crush = false;
                for i in 0..drop_count {
                    tx += dx;
                    ty += dy;

                    match self.board[tx as usize][ty as usize].last_piece_type() {
                        Some(Flatstone) | None => (),
                        Some(Capstone) => {
                            return Err(StateError::InvalidSpread(
                                "Cannot spread onto a capstone.",
                            ));
                        }
                        Some(StandingStone) => {
                            valid_crush = i == drop_count - 1
                                && top_piece.piece_type() == Capstone
                                && drops[i] == 1;

                            if !valid_crush {
                                return Err(StateError::InvalidSpread(
                                    "Cannot spread onto a standing stone.",
                                ));
                            } else if !crush {
                                trace!(
                                    "Ply describes a valid crush, but the crush flag was not set."
                                );
                                ply = Ply::Spread {
                                    x,
                                    y,
                                    direction,
                                    drops,
                                    crush: true,
                                };
                            }
                        }
                    }
                }

                if crush && !valid_crush {
                    return Err(StateError::InvalidSpread(
                        "Spread is not a crushing move, but the crush flag was set.",
                    ));
                }
            }
        }

        Ok(ply)
    }

    pub fn execute_ply(&mut self, mut ply: Ply<N>) -> Result<Ply<N>, StateError> {
        ply = self.validate_ply(ply)?;
        self.execute_ply_unchecked(ply);
        Ok(ply)
    }

    #[instrument(level = "trace", skip(self))]
    pub fn execute_ply_unchecked(&mut self, ply: Ply<N>) {
        use Color::*;
        use PieceType::*;

        let player_color = self.to_move();

        match ply {
            Ply::Place { x, y, piece_type } => {
                let color = if self.ply_count >= 2 {
                    player_color
                } else {
                    player_color.other()
                };

                let player_counts = match color {
                    White => (&mut self.p1_flatstones, &mut self.p1_capstones),
                    Black => (&mut self.p2_flatstones, &mut self.p2_capstones),
                };

                let selected_count = match piece_type {
                    Capstone => player_counts.1,
                    _ => player_counts.0,
                };

                // Execute the placement.
                *selected_count -= 1;
                let piece = Piece::new(piece_type, color);
                self.board[x as usize][y as usize].add_piece(piece);
                self.metadata.place_piece(piece, x as usize, y as usize);
                self.metadata.hash ^= zobrist_hash_stack(self, x as usize, y as usize);
            }
            Ply::Spread {
                x,
                y,
                direction,
                drops,
                ..
            } => {
                self.metadata.hash ^= zobrist_hash_stack(self, x as usize, y as usize);

                let carry_total = drops.iter().sum::<u8>() as usize;
                let mut carry = self.board[x as usize][y as usize].take(carry_total);

                self.metadata.set_stack(
                    &self.board[x as usize][y as usize],
                    x as usize,
                    y as usize,
                );
                self.metadata.hash ^= zobrist_hash_stack(self, x as usize, y as usize);

                let (dx, dy) = direction.to_offset();
                let (mut tx, mut ty) = (x as i8, y as i8);
                for drop in drops.into_iter().filter(|d| *d > 0) {
                    tx += dx;
                    ty += dy;

                    self.metadata.hash ^= zobrist_hash_stack(self, tx as usize, ty as usize);

                    self.board[tx as usize][ty as usize].add(carry.drop(drop as usize));
                    self.metadata.set_stack(
                        &self.board[tx as usize][ty as usize],
                        tx as usize,
                        ty as usize,
                    );
                    self.metadata.hash ^= zobrist_hash_stack(self, tx as usize, ty as usize);
                }
            }
        }

        self.metadata.hash ^= zobrist_advance_move::<N>();
        self.ply_count += 1;
    }

    pub fn resolution(&self) -> Option<Resolution> {
        fn spans_board<const N: usize>(bitmap: Bitmap<N>) -> bool {
            use Direction::*;
            let edge = edge_masks();

            let all_edges = edge[North as usize]
                | edge[East as usize]
                | edge[South as usize]
                | edge[West as usize];

            for group in bitmap.groups_from(bitmap & all_edges) {
                if (!(group & edge[North as usize]).is_empty()
                    && (!group & edge[South as usize]).is_empty())
                    || (!(group & edge[West as usize]).is_empty()
                        && !(group & edge[East as usize]).is_empty())
                {
                    return true;
                }
            }
            false
        }

        let m = &self.metadata;
        let p1_road = m.p1_pieces & (m.flatstones | m.capstones);
        let p2_road = m.p2_pieces & (m.flatstones | m.capstones);

        let p1_road_spans_board = spans_board(p1_road);
        let p2_road_spans_board = spans_board(p2_road);

        if p1_road_spans_board && p2_road_spans_board {
            // If both players have a road, the win goes to whomever made the move.
            if self.ply_count % 2 == 1 {
                Some(Resolution::Road(Color::White))
            } else {
                Some(Resolution::Road(Color::Black))
            }
        } else if p1_road_spans_board {
            Some(Resolution::Road(Color::White))
        } else if p2_road_spans_board {
            Some(Resolution::Road(Color::Black))
        } else if (self.p1_flatstones + self.p1_capstones) == 0
            || (self.p2_flatstones + self.p2_capstones) == 0
            || (m.p1_pieces | m.p2_pieces) == board_mask()
        {
            let p1_flat_count = m.p1_flat_count as i8;
            let p2_flat_count = m.p2_flat_count as i8;

            let p1_score = 2 * p1_flat_count;
            let p2_score = 2 * p2_flat_count + self.komi.as_half_komi();

            let resolution = match p1_score.cmp(&p2_score) {
                Greater => Resolution::Flats {
                    color: Color::White,
                    spread: p1_flat_count - p2_flat_count,
                    komi: -self.komi,
                },
                Less => Resolution::Flats {
                    color: Color::Black,
                    spread: p2_flat_count - p1_flat_count,
                    komi: self.komi,
                },
                Equal => Resolution::Draw,
            };

            Some(resolution)
        } else {
            None
        }
    }

    pub fn recalculate_metadata(&mut self) {
        self.metadata = Default::default();

        for x in 0..N {
            for y in 0..N {
                let stack = &self.board[x][y];
                if !stack.is_empty() {
                    self.metadata.set_stack(stack, x, y);
                }
            }
        }

        self.metadata.hash = zobrist_hash_state(self);
    }
}

impl<const N: usize> fmt::Debug for State<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("komi", &self.komi)
            .field("ply", &self.ply_count)
            .field("flats", &(self.p1_flatstones, self.p2_flatstones))
            .field("caps", &(self.p1_capstones, self.p2_capstones))
            .field("board", &Tps::from(self.clone()).to_string())
            .finish()
    }
}

impl<const N: usize> fmt::Display for State<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "  White: {:>2} flatstone{}, {} capstone{}",
            self.p1_flatstones,
            if self.p1_flatstones != 1 { "s" } else { "" },
            self.p1_capstones,
            if self.p1_capstones != 1 { "s" } else { "" },
        )?;
        writeln!(
            f,
            "  Black: {:>2} flatstone{}, {} capstone{}\n",
            self.p2_flatstones,
            if self.p2_flatstones != 1 { "s" } else { "" },
            self.p2_capstones,
            if self.p2_capstones != 1 { "s" } else { "" },
        )?;

        let board: Vec<Vec<String>> = self
            .board
            .iter()
            .map(|c| c.iter().map(|r| format!("[{r}]")).collect())
            .collect();

        let column_widths: Vec<usize> = board
            .iter()
            .map(|c| c.iter().map(|r| r.len() + 1).max().unwrap())
            .collect();

        for (rank, row) in (0..N)
            .map(|r| board.iter().map(move |c| &c[r]).zip(&column_widths))
            .enumerate()
            .rev()
        {
            write!(f, "  {}   ", rank + 1)?;
            for (stack, width) in row {
                write!(f, "{stack:<width$}", width = width)?;
            }
            writeln!(f)?;
        }

        write!(f, "\n      ")?;
        for (file, width) in (0..N)
            .map(|c| char::from_digit(c as u32 + 10, 10 + N as u32).unwrap())
            .zip(&column_widths)
        {
            write!(f, "{:<width$}", format!(" {file}"), width = width)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Resolution {
    Road(Color),
    Flats {
        color: Color,
        spread: i8,
        komi: Komi,
    },
    Draw,
}

impl Resolution {
    pub fn color(self) -> Option<Color> {
        match self {
            Resolution::Road(color) => Some(color),
            Resolution::Flats { color, .. } => Some(color),
            _ => None,
        }
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (token, color) = match self {
            Resolution::Road(color) => ("R", color),
            Resolution::Flats { color, .. } => ("F", color),
            Resolution::Draw => return write!(f, "1/2-1/2"),
        };

        match color {
            Color::White => write!(f, "{token}-0"),
            Color::Black => write!(f, "0-{token}"),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum StateError {
    PlyError(PlyError),
    InvalidPlace(&'static str),
    InvalidSpread(&'static str),
    NoPreviousPlies,
}

impl From<PlyError> for StateError {
    fn from(error: PlyError) -> Self {
        Self::PlyError(error)
    }
}

#[derive(Copy, Clone, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Komi(i8);

impl Komi {
    pub fn from_half_komi(value: i8) -> Self {
        Self(value)
    }

    pub fn as_half_komi(&self) -> i8 {
        self.0
    }
}

impl FromStr for Komi {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let half_komi = if let Some(period) = s.find('.') {
            let full = 2 * s[..period]
                .parse::<i8>()
                .map_err(|_| format!("invalid value for komi: {s}"))?;
            let half = match &s[period + 1..] {
                "0" => 0,
                "5" => 1,
                _ => return Err("only half komi are supported (*.0 or *.5)".to_owned()),
            };
            let sign = if full >= 0 { 1 } else { -1 };
            full + sign * half
        } else {
            2 * s
                .parse::<i8>()
                .map_err(|_| format!("invalid value for komi: {s}"))?
        };
        Ok(Self::from_half_komi(half_komi))
    }
}

impl fmt::Debug for Komi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let komi = self.0 / 2;
        let half = self.0 % 2 * 5;

        write!(f, "{komi}")?;
        if half > 0 {
            write!(f, ".{half}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Komi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Neg for Komi {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::from_half_komi(-self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_size() {
        for n in 3..=8 {
            let (size, alignment) = match n {
                3 => (
                    std::mem::size_of::<State<3>>(),
                    std::mem::align_of::<State<3>>(),
                ),
                4 => (
                    std::mem::size_of::<State<4>>(),
                    std::mem::align_of::<State<4>>(),
                ),
                5 => (
                    std::mem::size_of::<State<5>>(),
                    std::mem::align_of::<State<5>>(),
                ),
                6 => (
                    std::mem::size_of::<State<6>>(),
                    std::mem::align_of::<State<6>>(),
                ),
                7 => (
                    std::mem::size_of::<State<7>>(),
                    std::mem::align_of::<State<7>>(),
                ),
                8 => (
                    std::mem::size_of::<State<8>>(),
                    std::mem::align_of::<State<8>>(),
                ),
                _ => unreachable!(),
            };

            println!("State<{n}>: {size} bytes, {alignment} byte alignment");
        }
    }

    fn state<const N: usize>(tps: &str) -> State<N> {
        tps.parse().unwrap()
    }

    fn ply<const N: usize>(ptn: &str) -> Ply<N> {
        ptn.parse().unwrap()
    }

    #[test]
    fn execute_valid_plies() {
        let mut s = state::<5>("x5/x,212121C,212,2S,x/x5/x5/x5 1 2");

        s.execute_ply(ply("b2")).unwrap();
        assert_eq!(s, state("x5/x,212121C,212,2S,x/x5/x,1,x3/x5 2 2"),);

        s.execute_ply(ply("d4>")).unwrap();
        assert_eq!(s, state("x5/x,212121C,212,x,2S/x5/x,1,x3/x5 1 3"),);

        s.execute_ply(ply("4b4>211*")).unwrap();
        assert_eq!(s, state("x5/x,21,21221,2,21C/x5/x,1,x3/x5 2 3"));
    }

    #[test]
    fn execute_bad_crush() {
        let s = state::<5>("x5/x,21C,2S,x2/x5/x5/x5 1 2");

        assert_eq!(
            s.validate_ply(ply("2b4>")),
            Err(StateError::InvalidSpread(
                "Cannot spread onto a standing stone."
            )),
        );
    }

    #[test]
    fn resolution() {
        let s = State::<5>::default();
        assert_eq!(s.resolution(), None);

        let s = state::<5>("x5/21,1,1C,x2/2,12,2121,x2/x,2,1,221,1/x,2,1,x2 1 1");
        assert_eq!(s.resolution(), Some(Resolution::Road(Color::White)));

        let s = state::<5>("x5/21,1,1C,x2/2,12,2121,x2/x,2,1,221,1/x,2,2,2,2 1 2");
        assert_eq!(s.resolution(), Some(Resolution::Road(Color::Black)));

        let s = state::<5>("x5/21,1,1C,x2/2,12,2121,x2/x,2,1,221,1/x,2,2,2,2 2 2");
        assert_eq!(s.resolution(), Some(Resolution::Road(Color::White)));

        let mut s = state::<5>("1,1,2,2,1/2,2,1,1,2/1,1,2,2,1/2,2,1,1,2/1,1,2,2,1 1 3");
        assert_eq!(
            s.resolution(),
            Some(Resolution::Flats {
                color: Color::White,
                spread: 1,
                komi: Komi::default(),
            })
        );
        s.komi = Komi::from_half_komi(2);
        assert_eq!(s.resolution(), Some(Resolution::Draw));
        s.komi = Komi::from_half_komi(4);
        assert_eq!(
            s.resolution(),
            Some(Resolution::Flats {
                color: Color::Black,
                spread: -1,
                komi: Komi::from_half_komi(4),
            })
        );

        let s = state::<5>("1111,1111,1111,1111,2/1111,11C,x,x,2/x5/x5/x5 2 3");
        assert_eq!(
            s.resolution(),
            Some(Resolution::Flats {
                color: Color::White,
                spread: 3,
                komi: Komi::default(),
            })
        );
    }

    #[test]
    fn metadata_updates_correctly() {
        let mut s = state::<5>("12,1,22,121211,2C/x5/x5/x5/x5 1 2");
        assert_eq!(s.metadata.p1_flat_count, 2);
        assert_eq!(s.metadata.p2_flat_count, 2);

        s.execute_ply(ply("a4")).unwrap();
        assert_eq!(s.metadata.p1_flat_count, 3);

        s.execute_ply(ply("Sb4")).unwrap();
        assert_eq!(s.metadata.p2_flat_count, 2);

        s.execute_ply(ply("5d5<221")).unwrap();
        assert_eq!(s.metadata.p1_flat_count, 5);
        assert_eq!(s.metadata.p2_flat_count, 0);

        assert_eq!(
            s.metadata.p1_stacks,
            [
                [0, 0, 0, 0b00000001, 0b00000101],
                [0, 0, 0, 0, 0b00000101],
                [0, 0, 0, 0, 0b00001000],
                [0, 0, 0, 0, 0b00000001],
                [0; 5],
            ]
        );

        assert_eq!(
            s.metadata.p2_stacks,
            [
                [0, 0, 0, 0, 0b00000010],
                [0, 0, 0, 0b00000001, 0b00000010],
                [0, 0, 0, 0, 0b00000111],
                [0; 5],
                [0, 0, 0, 0, 0b00000001],
            ]
        );

        assert_eq!(s.metadata.p1_pieces, 0b11110_10000_00000_00000_00000.into());
        assert_eq!(s.metadata.p2_pieces, 0b00001_01000_00000_00000_00000.into());
        assert_eq!(
            s.metadata.flatstones,
            0b11110_10000_00000_00000_00000.into()
        );
        assert_eq!(
            s.metadata.standing_stones,
            0b00000_01000_00000_00000_00000.into()
        );
        assert_eq!(s.metadata.capstones, 0b00001_00000_00000_00000_00000.into());
    }

    #[test]
    fn hashes_are_uniquely_identifying() {
        let mut s = state::<5>("x5/x5/x5/x5/21,x,2,x2 1 2");
        let initial_hash = s.metadata.hash;

        s.execute_ply(ply("2a1>11")).unwrap();
        assert_ne!(s.metadata.hash, initial_hash);

        s.execute_ply(ply("b1<")).unwrap();
        assert_ne!(s.metadata.hash, initial_hash);

        s.execute_ply(ply("2c1<11")).unwrap();
        assert_ne!(s.metadata.hash, initial_hash);

        s.execute_ply(ply("b1>")).unwrap();
        assert_eq!(s.metadata.hash, initial_hash);
    }
}
