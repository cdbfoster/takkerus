//
// This file is part of tak-rs.
//
// tak-rs is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// tak-rs is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with tak-rs. If not, see <http://www.gnu.org/licenses/>.
//
// Copyright 2016 Chris Foster
//

extern crate sdl2;
extern crate sdl2_gfx;
extern crate time;

use sdl2::event::Event;

mod render;
mod tak;
mod timer;

use render::Renderable;
use timer::Timer;

fn main() {
    let mut player1 = tak::Player::new(tak::Color::White, 21, 1);
    let mut player2 = tak::Player::new(tak::Color::Black, 21, 1);

    {
        let white_piece = player1.get_flatstone().unwrap();
        let black_piece = player2.get_flatstone().unwrap();

        player1.pieces.insert(0, black_piece);
        player2.pieces.insert(0, white_piece);
    }

    let mut board = tak::Board::new(5);

    board.spaces[1][1].push(tak::Piece::Flatstone(tak::Color::White));
    board.spaces[2][1].push(tak::Piece::Flatstone(tak::Color::White));

    board.spaces[2][3].push(tak::Piece::Flatstone(tak::Color::Black));
    board.spaces[2][3].push(tak::Piece::Flatstone(tak::Color::White));
    board.spaces[2][3].push(tak::Piece::Flatstone(tak::Color::White));
    board.spaces[2][3].push(tak::Piece::Flatstone(tak::Color::Black));
    board.spaces[2][3].push(tak::Piece::Capstone(tak::Color::White));
    board.spaces[3][3].push(tak::Piece::Flatstone(tak::Color::Black));
    board.spaces[3][3].push(tak::Piece::Capstone(tak::Color::Black));

    let sdl_context = sdl2::init().unwrap();
    let mut sdl_event_pump = sdl_context.event_pump().unwrap();
    let mut render_context = render::initialize(&sdl_context, (1366, 768), false);

    let mut input_timer = Timer::new(1.0 / 30.0);
    let mut render_timer = Timer::new(1.0 / 45.0);

    let mut old_time = time::precise_time_ns();
    'main: loop {
        let elapsed_time = (time::precise_time_ns() - old_time) as i64;
        input_timer.elapse(elapsed_time);
        render_timer.elapse(elapsed_time);
        old_time += elapsed_time as u64;

        if input_timer.sprung() {
            for event in sdl_event_pump.poll_iter() {
                match event {
                    Event::Quit {..} => break 'main,

                    _ => (),
                }
            }

            input_timer.reset_with_overflow();
        }

        if render_timer.sprung() {
            render::clear(&mut render_context);

            board.render(&mut render_context);

            render::present(&mut render_context);

            render_timer.reset_with_overflow();
        }

        let sleep = {
            let remainders = vec![
                input_timer.remainder_ns(),
                render_timer.remainder_ns(),
            ];

            let min = *remainders.iter().min().unwrap() - (time::precise_time_ns() - old_time) as i64;
            if min > 0 { min as u32 } else { 0 }
        };
        std::thread::sleep(std::time::Duration::new(0, sleep));
    }
}
