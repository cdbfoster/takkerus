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

pub struct Timer {
    interval_ns: u64,
    remainder_ns: i64,
}

impl Timer {
    pub fn new(interval_s: f64) -> Timer {
        Timer {
            interval_ns: (1_000_000_000f64 * interval_s) as u64,
            remainder_ns: 0,
        }
    }

    pub fn elapse(&mut self, elapsed_ns: i64) {
        self.remainder_ns -= elapsed_ns;
    }

    pub fn sprung(&self) -> bool {
        self.remainder_ns <= 0
    }

    pub fn reset(&mut self) {
        self.remainder_ns = self.interval_ns as i64;
    }

    pub fn reset_with_overflow(&mut self) {
        self.remainder_ns += self.interval_ns as i64;
    }

    pub fn remainder_ns(&self) -> i64 {
        self.remainder_ns
    }

    pub fn elapsed_ns(&self) -> i64 {
        self.interval_ns as i64 - self.remainder_ns
    }
}
