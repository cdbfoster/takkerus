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

use std::any::Any;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Instant;

use zero_sum::analysis::search::{PvSearch, Search};
use zero_sum::impls::tak::{Color, Evaluation, Ply, Resolution, State};

use game::Message;
use player::{Player, PlayTakPlayer};

pub struct PvSearchPlayer {
    pvsearch: Arc<Mutex<PvSearch<Evaluation, State, Ply, Resolution>>>,
    depth: u8,
    goal: u16,
}

impl PvSearchPlayer {
    pub fn with_depth(depth: u8) -> PvSearchPlayer {
        PvSearchPlayer {
            pvsearch: Arc::new(Mutex::new(PvSearch::with_depth(depth))),
            depth: depth,
            goal: 0,
        }
    }

    pub fn with_goal(goal: u16) -> PvSearchPlayer {
        PvSearchPlayer {
            pvsearch: Arc::new(Mutex::new(PvSearch::with_goal(goal, 12.0))),
            depth: 0,
            goal: goal,
        }
    }
}

impl Player for PvSearchPlayer {
    fn initialize(&mut self, to_game: Sender<(Color, Message)>, opponent: &Player) -> Result<Sender<Message>, String> {
        let pvsearch = self.pvsearch.clone();
        let (sender, receiver) = mpsc::channel();
        let vs_playtak = opponent.as_any().is::<PlayTakPlayer>();

        thread::spawn(move || {
            let mut color = None;
            let interrupt = Arc::new(Mutex::new(None));

            for message in receiver.iter() {
                match message {
                    Message::GameStart(assigned_color) => color = Some(assigned_color),
                    Message::MoveRequest(state) => {
                        let (interrupt_sender, interrupt_receiver) = mpsc::channel();
                        let interrupt = interrupt.clone();
                        *interrupt.lock().unwrap() = Some(interrupt_sender);

                        let pvsearch = pvsearch.clone();
                        let to_game = to_game.clone();

                        thread::spawn(move || {
                            let start_search = Instant::now();
                            let analysis = pvsearch.lock().unwrap().search(&state, Some(interrupt_receiver));
                            let elapsed_search = start_search.elapsed();

                            let mut interrupt = interrupt.lock().unwrap();
                            if interrupt.is_none() {
                                return;
                            } else {
                                *interrupt = None;
                            }

                            if let Some(ply) = analysis.principal_variation.first() {
                                println!("[PVSearch] Decision time (depth {}): {:.3} seconds{}",
                                    analysis.principal_variation.len(),
                                    elapsed_search.as_secs() as f32 + elapsed_search.subsec_nanos() as f32 / 1_000_000_000.0,
                                    if vs_playtak {
                                        format!(", Evaluation: {}", analysis.evaluation)
                                    } else {
                                        String::new()
                                    },
                                );
                                to_game.send((color.unwrap(), Message::MoveResponse(ply.clone()))).ok();
                            }
                        });
                    },
                    Message::GameOver => {
                        let mut interrupt = interrupt.lock().unwrap();
                        if let Some(ref interrupt_sender) = *interrupt {
                            interrupt_sender.send(()).ok();
                        }
                        *interrupt = None;

                        break;
                    },
                    _ => (),
                }
            }
        });

        Ok(sender)
    }

    fn get_name(&self) -> String {
        format!("Takkerus{} (PVSearch{})",
            if let Some(version) = option_env!("CARGO_PKG_VERSION") {
                format!(" v{}", version)
            } else {
                String::new()
            },
            if self.depth != 0 {
                format!(" - Depth: {}", self.depth)
            } else if self.goal != 0 {
                format!(" - Goal: {}s", self.goal)
            } else {
                String::new()
            },
        )
    }

    fn as_any(&self) -> &Any {
        self
    }
}
