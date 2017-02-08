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
use std::sync::mpsc::Sender;

use zero_sum::impls::tak::Color;

use game::Message;

pub trait Player {
    fn initialize(&mut self, to_game: Sender<(Color, Message)>, opponent: &Player) -> Result<Sender<Message>, String>;
    fn get_name(&self) -> String;
    fn as_any(&self) -> &Any;
}

pub use self::cli_player::CliPlayer;
pub use self::playtak_player::PlayTakPlayer;
pub use self::pvsearch_player::PvSearchPlayer;

mod cli_player;
pub mod playtak_player;
mod pvsearch_player;
