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

use zero_sum::impls::tak::Color;

#[derive(Clone)]
pub enum GameType {
    Accept(String),
    Seek(Seek),
}

impl GameType {
    pub fn accept(from: &str) -> GameType {
        GameType::Accept(from.to_string())
    }

    pub fn seek(size: usize, time: u32, increment: u32, color: Option<Color>) -> GameType {
        GameType::Seek(Seek {
            size: size,
            time: time,
            increment: increment,
            color: color,
        })
    }
}

#[derive(Clone)]
pub struct Seek {
    pub size: usize,
    pub time: u32,
    pub increment: u32,
    pub color: Option<Color>,
}

#[derive(PartialEq)]
pub struct ListedGame {
    pub id: String,
    pub p1: String,
    pub p2: String,
    pub size: usize,
}
