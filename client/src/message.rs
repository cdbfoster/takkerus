use tak::{Color, Ply, Resolution, State};

#[derive(Debug)]
pub enum Message<const N: usize> {
    GameStart(Color),
    GameEnd(GameEnd),
    MoveRequest(State<N>),
    MoveResponse(Ply<N>),
    UndoRequest,
    UndoRequestWithdrawal,
    UndoResponse { accept: bool },
    DrawRequest,
    DrawRequestWithdrawal,
    DrawResponse { accept: bool },
}

#[derive(Debug)]
pub enum GameEnd {
    Resolution(Resolution),
    Resignation(Color),
}
