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
// Copyright 2016 Chris Foster
//

use std::collections::HashMap;
use std::fmt::{self, Write};
use std::fs::{self, OpenOptions};
use std::iter::Peekable;
use std::io::{Read, Write as IoWrite};
use std::str::{Chars, FromStr};

use time;

use tak::*;

static GAMES_LOG: &'static str = "games.log";
static GAMES_LOG_TMP: &'static str = "games.log.tmp";

#[derive(Debug)]
pub struct Game {
    pub header: Header,
    pub plies: Vec<Ply>,
}

impl Game {
    pub fn new() -> Game {
        Game {
            header: Header {
                event: String::from("Tak"),
                site: String::from("Local"),
                p1: String::new(),
                p2: String::new(),
                round: String::new(),
                date: {
                    let t = time::now();
                    let mut date = String::new();
                    write!(date, "{:4}.{:02}.{:02}", t.tm_year + 1900, t.tm_mon + 1, t.tm_mday).ok();
                    date
                },
                result: String::new(),
                size: 5,
                tps: String::new(),
            },
            plies: Vec::new(),
        }
    }

    pub fn to_state(&self) -> Option<State> {
        let mut state = if self.header.tps.is_empty() {
            State::new(self.header.size as usize)
        } else {
            let mut tps = String::new();
            write!(tps, "[TPS \"{}\"]", self.header.tps).ok();
            State::from_tps(&tps).unwrap()
        };

        for ply in self.plies.iter() {
            match state.execute_ply(ply) {
                Ok(next) => state = next,
                _ => return None,
            }
        }

        Some(state)
    }
}

impl fmt::Display for Game {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}\r\n", self.header).ok();

        for turn in 0..(self.plies.len() + 1) / 2 {
            write!(f, "{:2}. {:8}", turn + 1, self.plies[turn * 2].to_ptn()).ok();

            if turn * 2 + 1 < self.plies.len() {
                write!(f, "{}", self.plies[turn * 2 + 1].to_ptn()).ok();
            }

            write!(f, "\r\n").ok();
        }

        write!(f, "\r\n")
    }
}

#[derive(Debug, Default)]
pub struct Header {
    pub event: String,
    pub site: String,
    pub p1: String,
    pub p2: String,
    pub round: String,
    pub date: String,
    pub result: String,
    pub size: u8,
    pub tps: String,
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if !self.event.is_empty() {
            write!(f, "[Event \"{}\"]\r\n", self.event).ok();
        }
        if !self.site.is_empty() {
            write!(f, "[Site \"{}\"]\r\n", self.site).ok();
        }
        if !self.p1.is_empty() {
            write!(f, "[Player1 \"{}\"]\r\n", self.p1).ok();
        }
        if !self.p2.is_empty() {
            write!(f, "[Player2 \"{}\"]\r\n", self.p2).ok();
        }
        if !self.round.is_empty() {
            write!(f, "[Round \"{}\"]\r\n", self.round).ok();
        }
        if !self.date.is_empty() {
            write!(f, "[Date \"{}\"]\r\n", self.date).ok();
        }
        if !self.result.is_empty() {
            write!(f, "[Result \"{}\"]\r\n", self.result).ok();
        }

        let result = write!(f, "[Size \"{}\"]", self.size);

        if !self.tps.is_empty() {
            write!(f, "\r\n[TPS \"{}\"]", self.tps).ok();
        }

        result
    }
}

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

impl TagResult {
    fn is_some(&self) -> bool {
        match self {
            &TagResult::Ok(_, _) => true,
            _ => false,
        }
    }

    fn unwrap(self) -> (String, String) {
        match self {
            TagResult::Ok(name, value) => (name, value),
            _ => panic!("unwrap() called on an empty TagResult!"),
        }
    }
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

    'parse_tags: while tag.is_some() {
        let (name, value) = tag.clone().unwrap();

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
            match u8::from_str(&value) {
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

    let state = {
        let mut tps = String::new();
        write!(tps, "[TPS \"{}\"]", header.tps).ok();

        State::from_tps(&tps)
    };

    if state.is_none() && !header.tps.is_empty() {
        return None;
    }

    if state.is_some() && state.clone().unwrap().board.len() != header.size as usize {
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

    loop {
        match parse_turn_number(source) {
            Some(t) => if t != turn_number {
                return None;
            },
            None => break,
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

fn parse_game(source: &mut Peekable<Chars>) -> Option<Game> {
    let header = match parse_header(source) {
        Some(header) => header,
        None => return None,
    };

    let turn_offset = if header.tps.is_empty() {
        0
    } else {
        let mut tps = String::new();
        write!(tps, "[TPS \"{}\"]", header.tps).ok();
        State::from_tps(&tps).unwrap().ply_count as usize / 2
    };

    let plies = match parse_plies(source, turn_offset) {
        Some(plies) => plies,
        None => return None,
    };

    Some(Game {
        header: header,
        plies: plies,
    })
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

    let mut game = parse_game(&mut source);
    while game.is_some() {
        let header = game.unwrap().header;

        {
            let p1_entry = dictionary.entry(header.p1.clone()).or_insert(HashMap::new());
            let p2_entry = p1_entry.entry(header.p2.clone()).or_insert(0);
            *p2_entry += 1;
        }

        if header.p1 != header.p2 {
            let p2_entry = dictionary.entry(header.p2.clone()).or_insert(HashMap::new());
            let p1_entry = p2_entry.entry(header.p1.clone()).or_insert(0);
            *p1_entry += 1;
        }

        game = parse_game(&mut source);
    }

    dictionary
}

pub fn populate_game(game: &mut Game, p1: &Player, p2: &Player) {
    game.header.p1 = p1.get_name();
    game.header.p2 = p2.get_name();

    let dictionary = parse_adversary_dictionary();

    let round = match dictionary.get(&game.header.p1) {
        Some(p2_dictionary) => *p2_dictionary.get(&game.header.p2).unwrap_or(&0) + 1,
        None => 1,
    };

    game.header.round.clear();
    write!(game.header.round, "{}", round).ok();
}

#[derive(Debug)]
pub enum GameState {
    New(Game),
    Resume(Game),
}

pub fn check_tmp_file() -> GameState {
    match OpenOptions::new().read(true).open(GAMES_LOG_TMP) {
        Ok(mut file) => {
            let mut data = String::new();
            file.read_to_string(&mut data).ok();

            let mut source = data.chars().peekable();

            match parse_game(&mut source) {
                Some(game) => if game.to_state().is_some() {
                    GameState::Resume(game)
                } else {
                    GameState::New(Game::new())
                },
                None => GameState::New(Game::new()),
            }
        },
        _ => GameState::New(Game::new())
    }
}

pub fn write_tmp_file(game: &Game) {
    match OpenOptions::new().write(true).truncate(true).create(true).open(GAMES_LOG_TMP) {
        Ok(mut file) => {
            write!(&mut file, "{}", game).ok();
        },
        _ => (),
    }
}

pub fn finalize_tmp_file() {
    match OpenOptions::new().read(true).open(GAMES_LOG_TMP) {
        Ok(mut tmp_file) => {
            let mut tmp_data = String::new();
            tmp_file.read_to_string(&mut tmp_data).ok();

            match OpenOptions::new().append(true).create(true).open(GAMES_LOG) {
                Ok(mut file) => {
                    write!(&mut file, "{}", tmp_data).ok();

                    fs::remove_file(GAMES_LOG_TMP).ok();
                },
                _ => (),
            }
        },
        _ => (),
    }
}
