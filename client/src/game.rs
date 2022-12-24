use std::fmt::Write;

use async_std::prelude::*;
use async_std::task::{self, JoinHandle};
use futures::channel::mpsc::{self, UnboundedReceiver as Receiver, UnboundedSender as Sender};
use futures::{select, FutureExt, SinkExt};
use tracing::{error, instrument, trace, warn};

use tak::{Color, Ply, PlyError, PtnPly, Resolution, Stack, State, StateError};

use crate::message::{GameEnd as GameEndType, Message};

pub struct Player<const N: usize> {
    pub name: Option<String>,
    pub to_player: Sender<Message<N>>,
    pub task: JoinHandle<()>,
    pub color_select: Option<Color>,
}

pub trait PlayerInitializer<const N: usize>: Fn(Sender<Message<N>>) -> Player<N> {}

impl<T, const N: usize> PlayerInitializer<N> for T where T: Fn(Sender<Message<N>>) -> Player<N> {}

pub fn run<const N: usize>(
    p1_initialize: impl PlayerInitializer<N>,
    p2_initialize: impl PlayerInitializer<N>,
    state: State<N>,
) {
    let (to_game, from_p1) = mpsc::unbounded();
    let p1 = p1_initialize(to_game);

    let (to_game, from_p2) = mpsc::unbounded();
    let p2 = p2_initialize(to_game);

    task::block_on(game_handler(p1, from_p1, p2, from_p2, state));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlayerToken {
    Player1,
    Player2,
}

impl PlayerToken {
    fn other(self) -> Self {
        match self {
            Self::Player1 => Self::Player2,
            Self::Player2 => Self::Player1,
        }
    }
}

#[instrument(level = "trace", skip_all)]
async fn game_handler<const N: usize>(
    mut p1: Player<N>,
    from_p1: Receiver<Message<N>>,
    mut p2: Player<N>,
    from_p2: Receiver<Message<N>>,
    mut state: State<N>,
) {
    use Message::*;
    use PlayerToken::*;

    // Resolve colors, based on selections.
    let p1_color = p1
        .color_select
        .or_else(|| p2.color_select.map(|c| c.other()))
        .unwrap_or(Color::White);
    let p2_color = p1_color.other();

    let mut ply_history: Vec<Ply<N>> = Vec::new();

    let mut from_p1 = from_p1.fuse();
    let mut from_p2 = from_p2.fuse();

    macro_rules! send {
        ($p:expr, $message:expr) => {{
            let result = match $p {
                Player1 => p1.to_player.send($message).await,
                Player2 => p2.to_player.send($message).await,
            };
            if let Err(err) = result {
                error!(?err, player=?$p, "Could not send message to player.");
            }
        }};
    }

    send!(Player1, GameStart(p1_color));
    send!(Player2, GameStart(p2_color));

    print_board(&state, &ply_history);

    macro_rules! player_to_move {
        () => {{
            if p1_color == state.to_move() {
                Player1
            } else {
                Player2
            }
        }};
    }

    send!(player_to_move!(), MoveRequest(state.clone()));

    loop {
        let (from, message) = select! {
            message = from_p1.next().fuse() => (Player1, message),
            message = from_p2.next().fuse() => (Player2, message),
        };

        let message = if let Some(message) = message {
            message
        } else {
            error!(player = ?from, "Player hung up.");
            break;
        };

        trace!(?message, "Message received.");

        match message {
            MoveResponse(ply) => {
                if from != player_to_move!() {
                    warn!("Received a move response from the wrong player.");
                    continue;
                }

                if handle_ply(&mut state, ply, &mut ply_history).is_ok() {
                    print_board(&state, &ply_history);
                    if let Some(resolution) = state.resolution() {
                        send!(from, GameEnd(GameEndType::Resolution(resolution)));
                        send!(from.other(), GameEnd(GameEndType::Resolution(resolution)));
                        break;
                    }
                    send!(from.other(), MoveRequest(state.clone()));
                } else {
                    send!(from, MoveRequest(state.clone()));
                }
            }
            UndoRequest => {
                if ply_history.is_empty() {
                    // No history to undo, so reject the request.
                    send!(from, UndoResponse { accept: false });
                } else {
                    send!(from.other(), UndoRequest);
                }
            }
            UndoRequestWithdrawal => send!(from.other(), UndoRequestWithdrawal),
            UndoResponse { accept } => {
                if accept {
                    if ply_history.is_empty() {
                        error!("Undo request was accepted, but there is no more history.");
                    } else {
                        let ply = ply_history.pop().unwrap();
                        if let Err(err) = state.revert_ply(ply) {
                            error!(?err, "Error undoing ply.");
                        } else {
                            send!(from.other(), message);
                            send!(player_to_move!(), MoveRequest(state.clone()));
                        }
                    }
                } else {
                    send!(from.other(), message);
                }
            }
            DrawRequest => send!(from.other(), DrawRequest),
            DrawRequestWithdrawal => send!(from.other(), DrawRequestWithdrawal),
            DrawResponse { accept } => {
                if accept {
                    send!(from.other(), message);
                    // XXX actually record the drawn game.
                    send!(from, GameEnd(GameEndType::Resolution(Resolution::Draw)));
                    send!(
                        from.other(),
                        GameEnd(GameEndType::Resolution(Resolution::Draw))
                    );
                    break;
                } else {
                    send!(from.other(), message);
                }
            }
            _ => error!(?message, "Game received an unexpected message."),
        }
    }
}

fn handle_ply<const N: usize>(
    state: &mut State<N>,
    ply: Ply<N>,
    ply_history: &mut Vec<Ply<N>>,
) -> Result<(), StateError> {
    if let Err(err) = state.execute_ply(ply) {
        let message = match err {
            StateError::PlyError(PlyError::InvalidCrush) => "Invalid crush.",
            StateError::PlyError(PlyError::InvalidDrops(message)) => message,
            StateError::PlyError(PlyError::OutOfBounds) => "Out of bounds.",
            StateError::InvalidPlace(message) => message,
            StateError::InvalidSlide(message) => message,
            StateError::NoPreviousPlies => panic!("this can't happen here"),
        };
        println!("\nError: {message}");
        return Err(err);
    }
    ply_history.push(ply);
    Ok(())
}

fn print_board<const N: usize>(state: &State<N>, ply_history: &[Ply<N>]) {
    println!("\n--------------------------------------------------");

    println!(
        "\n Player 1: {:>2} flatstone{}, {} capstone{}",
        state.p1_flatstones,
        if state.p1_flatstones != 1 { "s" } else { "" },
        state.p1_capstones,
        if state.p1_capstones != 1 { "s" } else { "" },
    );
    println!(
        " Player 2: {:>2} flatstone{}, {} capstone{}\n",
        state.p2_flatstones,
        if state.p2_flatstones != 1 { "s" } else { "" },
        state.p2_capstones,
        if state.p2_capstones != 1 { "s" } else { "" },
    );

    let board: Vec<Vec<String>> = state
        .board
        .iter()
        .map(|c| c.iter().map(print_stack).collect())
        .collect();

    let column_widths: Vec<usize> = board
        .iter()
        .map(|c| c.iter().map(|r| r.len() + 3).max().unwrap())
        .collect();

    for (i, row) in (0..N)
        .map(|r| board.iter().map(move |c| &c[r]).zip(&column_widths))
        .enumerate()
        .rev()
    {
        let mut line = String::new();
        write!(line, " {}   ", i + 1).unwrap();
        for (stack, width) in row {
            let column = format!("[{stack}]");
            write!(line, "{column:<width$}", width = width).unwrap();
        }
        println!("{line}");
    }

    let mut file_letters = String::new();
    write!(file_letters, "\n     ").unwrap();
    for (f, width) in (0..N)
        .map(|c| char::from_digit(c as u32 + 10, 10 + N as u32).unwrap())
        .zip(&column_widths)
    {
        write!(file_letters, "{:<width$}", format!(" {f}"), width = width).unwrap();
    }
    println!("{file_letters}");

    let turn_number = format!("{}.", state.ply_count / 2 + 1);
    if ply_history.is_empty() {
        println!("\n {turn_number:<3}  --");
    } else {
        let last_turn = if ply_history.len() % 2 == 0 {
            &ply_history[2 * (ply_history.len() / 2 - 1)..]
        } else {
            &ply_history[2 * (ply_history.len() / 2)..]
        };

        let mut turn = String::new();
        write!(
            turn,
            "\n {turn_number:<3}  {:<width$}",
            &*PtnPly::from(&last_turn[0]),
            width = N + 4,
        )
        .unwrap();

        if last_turn.len() == 2 {
            write!(turn, "{}", &*PtnPly::from(&last_turn[1]),).unwrap();
        } else {
            write!(turn, "--").unwrap();
        }

        println!("{turn}");
    }
}

fn print_stack(stack: &Stack) -> String {
    if stack.is_empty() {
        " ".to_owned()
    } else {
        let mut buffer = String::new();
        for (i, piece) in stack.iter().rev().enumerate() {
            if i > 0 {
                write!(buffer, " ").unwrap();
            }
            write!(buffer, "{piece:?}").unwrap();
        }
        buffer
    }
}
