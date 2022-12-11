use std::convert::TryFrom;
use std::fmt;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PieceType {
    Flatstone = 0x10,
    StandingStone = 0x20,
    Capstone = 0x40,
}

impl TryFrom<u8> for PieceType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x10 => Ok(PieceType::Flatstone),
            0x20 => Ok(PieceType::StandingStone),
            0x40 => Ok(PieceType::Capstone),
            _ => Err("invalid piece type value"),
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Color {
    White = 0x01,
    Black = 0x02,
}

impl Color {
    pub fn other(self) -> Self {
        (0x03 - self as u8).try_into().unwrap()
    }
}

impl TryFrom<u8> for Color {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Color::White),
            0x02 => Ok(Color::Black),
            _ => Err("invalid color value"),
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Piece(u8);

impl Piece {
    pub fn new(piece_type: PieceType, color: Color) -> Self {
        Self(piece_type as u8 | color as u8)
    }

    pub fn piece_type(self) -> PieceType {
        (self.0 & 0xF0).try_into().unwrap()
    }

    pub fn set_piece_type(&mut self, piece_type: PieceType) {
        self.0 &= 0x0F;
        self.0 |= piece_type as u8;
    }

    pub fn color(self) -> Color {
        (self.0 & 0x0F).try_into().unwrap()
    }

    pub fn set_color(&mut self, color: Color) {
        self.0 &= 0xF0;
        self.0 |= color as u8;
    }
}

impl fmt::Debug for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            match self.color() {
                Color::White => "W",
                Color::Black => "B",
            },
            match self.piece_type() {
                PieceType::Flatstone => "",
                PieceType::StandingStone => "S",
                PieceType::Capstone => "C",
            }
        )
    }
}
