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

use std::fmt;

use time;

#[derive(Debug, Default)]
pub struct Header {
    pub event: String,
    pub site: String,
    pub p1: String,
    pub p2: String,
    pub round: String,
    pub date: String,
    pub result: String,
    pub size: usize,
    pub tps: String,
}

impl Header {
    pub fn new() -> Header {
        Header {
            event: String::from("Tak"),
            site: String::from("Local"),
            p1: String::new(),
            p2: String::new(),
            round: String::new(),
            date: {
                let t = time::now();
                format!("{:4}.{:02}.{:02}", t.tm_year + 1900, t.tm_mon + 1, t.tm_mday)
            },
            result: String::new(),
            size: 5,
            tps: String::new(),
        }
    }
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
