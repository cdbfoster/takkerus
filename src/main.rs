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
extern crate sdl2;
extern crate sdl2_gfx;
extern crate time;

use sdl2::event::Event;

//mod render;
mod tak;
mod timer;

//use render::Renderable;
use timer::Timer;

fn main() {
    let mut state = tak::State::new(5);

    state = state.execute_ply(&tak::Ply::from_ptn("Ca1", tak::Color::White).unwrap()).ok().unwrap();
    state = state.execute_ply(&tak::Ply::from_ptn("Sb1", tak::Color::Black).unwrap()).ok().unwrap();
    state = state.execute_ply(&tak::Ply::from_ptn("a1>", tak::Color::White).unwrap()).ok().unwrap();
    state = state.execute_ply(&tak::Ply::from_ptn("2b1+11", tak::Color::White).unwrap()).ok().unwrap();
    println!("{:?}", state);

    let sdl_context = sdl2::init().unwrap();
    let mut sdl_event_pump = sdl_context.event_pump().unwrap();
    //let mut render_context = render::initialize(&sdl_context, (1366, 768), false);

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
            //render::clear(&mut render_context);

            //board.render(&mut render_context);

            //render::present(&mut render_context);

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
