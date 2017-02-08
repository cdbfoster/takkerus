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

use zero_sum::impls::tak::{Color, Ply, State};

#[derive(Clone, Debug)]
pub enum Message {
    GameStart(Color),
    GameOver,
    MoveRequest(State),
    MoveResponse(Ply),
    UndoRequest,
    UndoAccept,
    UndoRemove,
    //Chat(String),
    Special(String),
}
