//
// This file is part of Takkerus.
//
// Takkerus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Takkerus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Takkerus. If not, see <http://www.gnu.org/licenses/>.
//
// Copyright 2016-2017 Chris Foster
//

use std::str::FromStr;

use regex::Regex;
use zero_sum::impls::tak::{Color, Direction, Piece, Ply};

use super::game_type::ListedGame;
use super::message_queue::MessageQueue;

lazy_static! {
    static ref WELCOME: Regex = Regex::new(
        r"^Welcome (.*)!$"
    ).unwrap();

    static ref SHOUT_COMMAND: Regex = Regex::new(
        r"^Shout <([^>]+)> ([^ :]+):?\s*([^ :]+):?\s*(.*)$"
    ).unwrap();

    static ref GAMELIST_CHANGE: Regex = Regex::new(
        r"^GameList ([^ ]+) ([^ ]+) ([^ ]+) vs ([^,]+), (.).*$"
    ).unwrap();
}

pub fn welcome(message: &str) -> Option<String> {
    if let Some(captures) = WELCOME.captures(message) {
        Some(captures[1].to_string())
    } else {
        None
    }
}

// Extracts the game's id string, the playtak player's name, the size of the board, and the playtak player's color
pub fn game_start(message: &str) -> (String, String, usize, Color) {
    let parts = message.split_whitespace().collect::<Vec<_>>();
    (
        format!("Game#{}", parts[2]),
        if parts[7] == "white" {
            parts[6].to_string()
        } else {
            parts[4].to_string()
        },
        usize::from_str(parts[3]).unwrap(),
        if parts[7] == "white" {
            Color::Black
        } else {
            Color::White
        },
    )
}

pub fn game(message_queue: &MessageQueue, id: &str) -> Vec<Ply> {
    let mut plies = Vec::new();

    for message in message_queue.iter_select(|m| m.starts_with(id)) {
        if message.starts_with(&format!("{} Time", id)) {
            break;
        }

        let parts = message.split_whitespace().collect::<Vec<_>>();

        if parts.len() <= 1 {
            continue;
        }

        if parts[1] == "P" || parts[1] == "M" {
            let string = parts[1..].join(" ");

            let next_color = {
                let next_color = if plies.len() % 2 == 0 {
                    Color::White
                } else {
                    Color::Black
                };

                if plies.len() < 2 {
                    next_color.flip()
                } else {
                    next_color
                }
            };

            if let Some(ply) = ply(&string, next_color) {
                plies.push(ply);
            }
        }
    }

    plies
}

pub fn ply(string: &str, color: Color) -> Option<Ply> {
    fn parse_square(square: &str) -> Option<(usize, usize)> {
        let mut chars = square.chars();

        let x = if let Some(x) = chars.next() {
            (x as u8 - 65) as usize
        } else {
            return None;
        };

        let y = if let Some(y) = chars.next() {
            (y as u8 - 49) as usize
        } else {
            return None;
        };

        Some((x, y))
    }

    let parts = string.split_whitespace().collect::<Vec<_>>();

    if parts[0] == "P" {
        if parts.len() < 2 {
            return None;
        }

        let (x, y) = if let Some(coordinates) = parse_square(parts[1]) {
            coordinates
        } else {
            return None;
        };

        let piece = if parts.len() >= 3 {
            if parts[2] == "W" {
                Piece::StandingStone(color)
            } else if parts[2] == "C" {
                Piece::Capstone(color)
            } else {
                return None;
            }
        } else {
            Piece::Flatstone(color)
        };

        Some(Ply::Place {
            x: x,
            y: y,
            piece: piece
        })
    } else if parts[0] == "M" {
        if parts.len() < 4 {
            return None;
        }

        let (x, y) = if let Some(coordinates) = parse_square(parts[1]) {
            coordinates
        } else {
            return None;
        };

        let (tx, ty) = if let Some(coordinates) = parse_square(parts[2]) {
            coordinates
        } else {
            return None;
        };

        let direction = {
            let (dx, dy) = (
                tx as i8 - x as i8,
                ty as i8 - y as i8,
            );

            if dx < 0 && dy == 0 {
                Direction::West
            } else if dx > 0 && dy == 0 {
                Direction::East
            } else if dy < 0 && dx == 0 {
                Direction::South
            } else if dy > 0 && dx == 0 {
                Direction::North
            } else {
                return None;
            }
        };

        let drops = parts[3..].iter().map(|drop| u8::from_str(drop).unwrap()).collect::<Vec<_>>();

        Some(Ply::Slide {
            x: x,
            y: y,
            direction: direction,
            drops: drops,
        })
    } else {
        None
    }
}

pub fn shout(message: &str, username: &String) -> Option<(String, String, String)> {
    if let Some(captures) = SHOUT_COMMAND.captures(message) {
        let invoker = captures[1].to_string();
        let target = captures[2].to_lowercase();

        if target != username.to_lowercase() &&
           format!("{}bot", target) != username.to_lowercase() {
            return None;
        }

        let command = {
            let raw = captures[3].to_lowercase();

            if raw == "size" || raw == "color" {
                raw
            } else if raw == "evaluate" || raw == "evaluation" || raw == "eval" {
                String::from("evaluate")
            } else {
                String::new()
            }
        };

        let value = captures[4].to_lowercase();

        Some((invoker, command, value))
    } else {
        None
    }
}

pub fn game_list(message: &str) -> Option<(ListedGame, String)> {
    if let Some(captures) = GAMELIST_CHANGE.captures(message) {
        Some((
            ListedGame {
                id: captures[2].to_string(),
                p1: captures[3].to_string(),
                p2: captures[4].to_string(),
                size: usize::from_str(&captures[5]).unwrap(),
            },
            captures[1].to_string(),
        ))
    } else {
        None
    }
}
