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

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::iter::Peekable;
use std::io::{Read, Write};
use std::str::{Chars, FromStr};

use zero_sum::impls::tak::{Color, Ply, State};

use super::{Game, Header};

static GAMES_LOG: &'static str = "games.log";
static GAMES_LOG_TMP: &'static str = "games.log.tmp";

fn advance_whitespace(source: &mut Peekable<Chars>, include_newline: bool) {
    let mut peek_char = {
        let peek = source.peek();
        if peek.is_some() {
            *peek.unwrap()
        } else {
            return;
        }
    };

    while peek_char == ' ' ||
          peek_char == '\t' ||
          (include_newline && (peek_char == '\r' || peek_char == '\n')) {
        source.next();

        peek_char = {
            let peek = source.peek();
            if peek.is_some() {
                *peek.unwrap()
            } else {
                break;
            }
        };
    }
}

#[derive(Clone, PartialEq)]
enum TagResult {
    NoTag,
    Ok(String, String),
    InvalidTag,
}

fn parse_tag(source: &mut Peekable<Chars>) -> TagResult {
    if source.peek() != Some(&'[') {
        return TagResult::NoTag;
    } else {
        source.next();
    }

    let mut name = String::new();
    let mut value = String::new();

    let mut peek_char = {
        let peek = source.peek();
        if peek.is_some() {
            *peek.unwrap()
        } else {
            return TagResult::InvalidTag;
        }
    };

    while peek_char != ' ' &&
          peek_char != '"' &&
          peek_char != '[' &&
          peek_char != ']' &&
          peek_char != '\t' &&
          peek_char != '\r' &&
          peek_char != '\n' {
        name.push(peek_char);

        source.next();

        peek_char = {
            let peek = source.peek();
            if peek.is_some() {
                *peek.unwrap()
            } else {
                return TagResult::InvalidTag;
            }
        };
    }

    if name.is_empty() {
        return TagResult::InvalidTag;
    }

    if peek_char != ' ' {
        return TagResult::InvalidTag;
    } else {
        source.next();
    }

    if source.peek() != Some(&'"') {
        return TagResult::InvalidTag;
    } else {
        source.next();
    }

    peek_char = {
        let peek = source.peek();
        if peek.is_some() {
            *peek.unwrap()
        } else {
            return TagResult::InvalidTag;
        }
    };

    while peek_char != '"' &&
          peek_char != '[' &&
          peek_char != ']' &&
          peek_char != '\r' &&
          peek_char != '\n' {
        value.push(peek_char);

        source.next();

        peek_char = {
            let peek = source.peek();
            if peek.is_some() {
                *peek.unwrap()
            } else {
                return TagResult::InvalidTag;
            }
        };
    }

    if peek_char != '"' {
        return TagResult::InvalidTag;
    } else {
        source.next();
    }

    if source.peek() != Some(&']') {
        return TagResult::InvalidTag;
    } else {
        source.next();
    }

    TagResult::Ok(name, value)
}

fn parse_header(source: &mut Peekable<Chars>) -> Option<Header> {
    let mut header = Header::default();

    advance_whitespace(source, true);
    let mut tag = parse_tag(source);

    while let TagResult::Ok(name, value) = tag {
        if name == "Event" {
            header.event = value;
        } else if name == "Site" {
            header.site = value;
        } else if name == "Player1" {
            header.p1 = value;
        } else if name == "Player2" {
            header.p2 = value;
        } else if name == "Round" {
            header.round = value;
        } else if name == "Date" {
            header.date = value;
        } else if name == "Result" {
            header.result = value;
        } else if name == "Size" {
            match usize::from_str(&value) {
                Ok(size) => header.size = size,
                _ => return None,
            }
        } else if name == "TPS" {
            header.tps = value;
        }

        advance_whitespace(source, true);
        tag = parse_tag(source);
    }

    if tag == TagResult::InvalidTag {
        return None;
    }

    if header.size < 3 || header.size > 8 {
        return None;
    }

    if let Some(state) = State::from_tps(&format!("[TPS \"{}\"]", header.tps)) {
        if state.board.len() != header.size {
            return None;
        }
    } else if !header.tps.is_empty() {
        return None;
    }

    Some(header)
}

fn parse_plies(source: &mut Peekable<Chars>, turn_offset: usize) -> Option<Vec<Ply>> {
    fn parse_turn_number(source: &mut Peekable<Chars>) -> Option<usize> {
        advance_whitespace(source, true);
        parse_comment(source);
        advance_whitespace(source, true);

        let mut turn_number_string = String::new();

        let mut peek_char = {
            let peek = source.peek();
            if peek.is_some() {
                *peek.unwrap()
            } else {
                return None;
            }
        };

        while peek_char.is_digit(10) {
            turn_number_string.push(peek_char);

            source.next();

            peek_char = {
                let peek = source.peek();
                if peek.is_some() {
                    *peek.unwrap()
                } else {
                    return None;
                }
            };
        }

        if peek_char == '.' {
            source.next();
        } else {
            return None;
        }

        match usize::from_str(&turn_number_string) {
            Ok(turn_number) => Some(turn_number),
            _ => None,
        }
    }

    fn parse_ptn(source: &mut Peekable<Chars>, color: Color) -> Option<Ply> {
        advance_whitespace(source, false);
        parse_comment(source);
        advance_whitespace(source, false);

        let mut ptn = String::new();

        let mut peek_char = {
            let peek = source.peek();
            if peek.is_some() {
                *peek.unwrap()
            } else {
                return None;
            }
        };

        while peek_char.is_digit(10) ||
              peek_char.is_alphabetic() ||
              peek_char == '+' || peek_char == '-' ||
              peek_char == '<' || peek_char == '>' {
            ptn.push(peek_char);

            source.next();

            peek_char = {
                let peek = source.peek();
                if peek.is_some() {
                    *peek.unwrap()
                } else {
                    break;
                }
            };
        }

        while peek_char == '\'' ||
              peek_char == '?' ||
              peek_char == '!' {
            source.next();

            peek_char = {
                let peek = source.peek();
                if peek.is_some() {
                    *peek.unwrap()
                } else {
                    break;
                }
            };
        }

        Ply::from_ptn(&ptn, color)
    }

    fn parse_comment(source: &mut Peekable<Chars>) -> Option<String> {
        let mut comment = String::new();

        let mut peek_char = {
            let peek = source.peek();
            if peek.is_some() {
                *peek.unwrap()
            } else {
                return None;
            }
        };

        if peek_char != '{' {
            return None;
        } else {
            source.next();
        }

        peek_char = {
            let peek = source.peek();
            if peek.is_some() {
                *peek.unwrap()
            } else {
                return None;
            }
        };

        while peek_char != '}' {
            comment.push(peek_char);

            source.next();

            peek_char = {
                let peek = source.peek();
                if peek.is_some() {
                    *peek.unwrap()
                } else {
                    return None;
                }
            };
        }

        source.next();

        Some(comment)
    }

    let mut turn_number = 1;
    let mut plies = Vec::new();

    while let Some(t) = parse_turn_number(source) {
        if t != turn_number {
            return None;
        }

        match parse_ptn(source, if turn_number + turn_offset != 1 {
            Color::White
        } else {
            Color::Black
        }) {
            Some(ply) => plies.push(ply),
            None => return None,
        }

        match parse_ptn(source, if turn_number + turn_offset != 1 {
            Color::Black
        } else {
            Color::White
        }) {
            Some(ply) => plies.push(ply),
            None => break,
        }

        turn_number += 1;
    }

    Some(plies)
}

fn parse_game(source: &mut Peekable<Chars>) -> Option<(Header, Vec<Ply>)> {
    let header = match parse_header(source) {
        Some(header) => header,
        None => return None,
    };

    let turn_offset = if header.tps.is_empty() {
        0
    } else {
        State::from_tps(&format!("[TPS \"{}\"]", header.tps)).unwrap().ply_count as usize / 2
    };

    let plies = match parse_plies(source, turn_offset) {
        Some(plies) => plies,
        None => return None,
    };

    Some((header, plies))
}

fn parse_adversary_dictionary() -> HashMap<String, HashMap<String, usize>> {
    let mut dictionary = HashMap::new();

    let games_data = match OpenOptions::new().read(true).open(GAMES_LOG) {
        Ok(mut file) => {
            let mut data = String::new();
            file.read_to_string(&mut data).ok();

            data
        },
        _ => String::new(),
    };

    let mut source = games_data.chars().peekable();

    while let Some((header, _)) = parse_game(&mut source) {
        {
            let p1_entry = dictionary.entry(header.p1.clone()).or_insert_with(HashMap::new);
            let p2_entry = p1_entry.entry(header.p2.clone()).or_insert(0);
            *p2_entry += 1;
        }

        if header.p1 != header.p2 {
            let p2_entry = dictionary.entry(header.p2.clone()).or_insert_with(HashMap::new);
            let p1_entry = p2_entry.entry(header.p1.clone()).or_insert(0);
            *p1_entry += 1;
        }
    }

    dictionary
}

pub fn get_round_number(game: &Game) -> usize {
    let dictionary = parse_adversary_dictionary();

    match dictionary.get(&game.header.p1) {
        Some(p2_dictionary) => *p2_dictionary.get(&game.header.p2).unwrap_or(&0) + 1,
        None => 1,
    }
}

pub fn open_ptn_file(file_name: &str) -> Result<(Header, Vec<Ply>), String> {
    match OpenOptions::new().read(true).open(file_name) {
        Ok(mut file) => {
            let mut data = String::new();
            file.read_to_string(&mut data).ok();

            let mut source = data.chars().peekable();

            match parse_game(&mut source) {
                Some((header, plies)) => {
                    let game = Game {
                        header: header,
                        plies: plies,
                        p1: None, p2: None, p1_sender: None, p2_sender: None,
                    };
                    if game.to_state().is_ok() {
                        Ok((game.header, game.plies))
                    } else {
                        Err(String::from("Invalid PTN"))
                    }
                },
                None => Err(String::from("Invalid PTN")),
            }
        },
        _ => Err(String::from("Cannot open PTN file")),
    }
}

pub fn read_tmp_file() -> Result<(Header, Vec<Ply>), String> {
    open_ptn_file(GAMES_LOG_TMP)
}

pub fn write_tmp_file(game: &Game) {
    if let Ok(mut file) = OpenOptions::new().write(true).truncate(true).create(true).open(GAMES_LOG_TMP) {
        write!(&mut file, "{}", game).ok();
    }
}

pub fn finalize_tmp_file() {
    if let Ok(mut tmp_file) = OpenOptions::new().read(true).open(GAMES_LOG_TMP) {
        let mut tmp_data = String::new();
        tmp_file.read_to_string(&mut tmp_data).ok();

        if let Ok(mut file) = OpenOptions::new().append(true).create(true).open(GAMES_LOG) {
            write!(&mut file, "{}", tmp_data).ok();

            fs::remove_file(GAMES_LOG_TMP).ok();
        }
    }
}
