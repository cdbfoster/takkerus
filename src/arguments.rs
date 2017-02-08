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

use getopts::{Matches, Options as GetOptsOptions, ParsingStyle};
use zero_sum::impls::tak::Color;

use game::Game;
use player::{self, Player};

pub struct Options {
    options: GetOptsOptions,
}

impl Options {
    pub fn new() -> Options {
        let mut options = Options {
            options: GetOptsOptions::new(),
        };

        options.options.parsing_style(ParsingStyle::StopAtFirstFree);
        options
    }

    pub fn flag(&mut self, short: &str, long: &str) -> &mut Options {
        self.options.optflag(short, long, "");
        self
    }

    pub fn opt(&mut self, short: &str, long: &str) -> &mut Options {
        self.options.optopt(short, long, "", "");
        self
    }

    pub fn parse(&self, args: &[String]) -> Matches {
        for i in 0..args.len() + 1 {
            if let Ok(mut matches) = self.options.parse(&args[..args.len() - i]) {
                matches.free.extend(args[args.len() - i..].iter().cloned());
                return matches;
            }
        }

        panic!("No okay matches!");
    }
}

pub fn parse_player(game: &Game, player: &str, matches: &Matches) -> Result<(Option<Box<Player>>, Vec<String>), String> {
    if let Some(player_type) = matches.opt_str(player) {
        if player_type == "human" {
            let mut human_options = Options::new();
            human_options
                .opt("n", "name");

            let matches = human_options.parse(&matches.free);

            Ok((Some(Box::new(player::CliPlayer::new(&(
                if let Some(name) = matches.opt_str("name") {
                    name
                } else {
                    String::from("Human")
                }
            )))), matches.free))
        } else if player_type == "pvsearch" {
            let mut pvsearch_options = Options::new();
            pvsearch_options
                .opt("d", "depth")
                .opt("g", "goal");

            let matches = pvsearch_options.parse(&matches.free);

            if matches.opt_present("depth") && matches.opt_present("goal") {
                return Err(String::from("Both depth and goal were specified."));
            }

            Ok((Some(Box::new(
                if let Some(depth) = matches.opt_str("depth") {
                    if let Ok(depth) = u8::from_str(&depth) {
                        player::PvSearchPlayer::with_depth(depth)
                    } else {
                        return Err(String::from("Invalid depth."));
                    }
                } else {
                    let goal = if let Some(goal) = matches.opt_str("goal") {
                        if let Ok(goal) = u16::from_str(&goal) {
                            goal
                        } else {
                            return Err(String::from("Invalid goal."));
                        }
                    } else {
                        60
                    };

                    player::PvSearchPlayer::with_goal(goal)
                }
            )), matches.free))
        } else if player_type == "playtak" {
            let mut playtak_options = Options::new();
            playtak_options
                .opt("h", "host")
                .opt("u", "user")
                .opt("p", "pass");

            let matches = playtak_options.parse(&matches.free);

            let host = matches.opt_str("host").unwrap_or(String::from("playtak.com:10000"));
            let user = matches.opt_str("user").unwrap_or(String::new());
            let pass = matches.opt_str("pass").unwrap_or(String::new());

            let mut game_type_options = Options::new();
            game_type_options
                .flag("", "accept")
                .flag("", "seek");

            let mut matches = game_type_options.parse(&matches.free);

            if matches.opt_present("accept") && matches.opt_present("seek") {
                return Err(String::from("Both --accept and --seek were specified."));
            }

            let game_type = if matches.opt_present("accept") {
                let mut accept_options = Options::new();
                accept_options
                    .opt("f", "from");

                matches = accept_options.parse(&matches.free);

                player::playtak_player::GameType::accept(&(
                    if let Some(from) = matches.opt_str("from") {
                        from
                    } else {
                        return Err(String::from("--from must be specified with --accept."));
                    }
                ))
            } else {
                let mut seek_options = Options::new();
                seek_options
                    .opt("t", "time")
                    .opt("i", "inc")
                    .opt("c", "color");

                matches = seek_options.parse(&matches.free);

                let time = if let Some(time) = matches.opt_str("time") {
                    if let Ok(time) = u32::from_str(&time) {
                        time
                    } else {
                        return Err(String::from("Invalid time."));
                    }
                } else {
                    1200
                };

                let inc = if let Some(inc) = matches.opt_str("inc") {
                    if let Ok(inc) = u32::from_str(&inc) {
                        inc
                    } else {
                        return Err(String::from("Invalid increment."));
                    }
                } else {
                    30
                };

                let color = if let Some(color) = matches.opt_str("color") {
                    if color == "white" {
                        Some(Color::White)
                    } else if color == "black" {
                        Some(Color::Black)
                    } else if color == "none" {
                        None
                    } else {
                        return Err(format!("Unrecognized color option: {}.", color));
                    }
                } else {
                    None
                };

                player::playtak_player::GameType::seek(
                    game.header.size,
                    time,
                    inc,
                    color,
                )
            };

            Ok((
                Some(Box::new(player::PlayTakPlayer::new(&host, &user, &pass, game_type))),
                matches.free,
            ))
        } else {
            Err(format!("Unrecognized player type: {}.", player_type))
        }
    } else {
        Ok((None, matches.free.clone()))
    }
}
