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

#[macro_use]
extern crate lazy_static;

mod tak;

use tak::Player;

fn main() {
    let mut state = tak::State::new(5);

    let mut p1 = tak::cli_player::CliPlayer::new(tak::Color::White);
    let mut p2 = tak::cli_player::CliPlayer::new(tak::Color::Black);

    loop {
        let p = if state.ply_count % 2 == 0 {
            &mut p1
        } else {
            &mut p2
        };

        println!("{:?}", state);

        let ply = p.get_move(&state);
        state = state.execute_ply(&ply).unwrap();
    }
}
