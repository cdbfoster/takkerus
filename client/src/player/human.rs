use std::io::{self, Write};
use std::mem;
use std::sync::Mutex;

use async_std::io::{stdin, BufReader};
use async_std::prelude::*;
use async_std::task::{self, JoinHandle};
use futures::channel::mpsc::{self, UnboundedReceiver as Receiver, UnboundedSender as Sender};
use futures::{select, FutureExt, SinkExt};
use once_cell::sync::Lazy;
use tracing::{debug, error, instrument, trace};

use tak::Ply;

use crate::play::{Message, Player};

static SETUP: Lazy<Mutex<Setup>> = Lazy::new(|| Mutex::new(Setup::default()));

#[derive(Default)]
struct Setup {
    human_count: usize,
    focus_sender: Option<Sender<Sender<String>>>,
    stdin_coordinator: Option<JoinHandle<()>>,
}

pub fn initialize<const N: usize>(name: Option<String>, to_game: Sender<Message<N>>) -> Player<N> {
    let mut setup = SETUP.lock().unwrap();

    setup.human_count += 1;
    trace!(
        human_count = setup.human_count,
        "Initializing a human player."
    );

    if setup.stdin_coordinator.is_none() {
        trace!("Spawning the stdin coordinator.");
        let (focus_sender, focus_receiver) = mpsc::unbounded();
        setup.focus_sender = Some(focus_sender);
        setup.stdin_coordinator = Some(task::spawn(stdin_coordinator(focus_receiver)));
    }

    let (to_player, from_game) = mpsc::unbounded();
    let (input_sender, input_receiver) = mpsc::unbounded();

    let player_info = PlayerSetup {
        name: name.clone(),
        human_number: setup.human_count,
        input_sender,
        input_receiver,
        focus_sender: setup.focus_sender.clone().unwrap(),
    };

    Player {
        name,
        to_player,
        task: task::spawn(message_handler::<N>(player_info, to_game, from_game)),
        color_select: None,
    }
}

#[derive(PartialEq, Eq)]
enum ResponseState {
    /// We are waiting for input from the player.
    AwaitingInput,
    /// The player is waiting for a response from the opponent.
    Requested,
}

struct PlayerSetup {
    name: Option<String>,
    human_number: usize,
    /// This will be given to the stdin coordinator to take input focus.
    input_sender: Sender<String>,
    /// stdin input comes through here via the stdin coordinator.
    input_receiver: Receiver<String>,
    /// Used to take input focus from the stdin coordinator.
    focus_sender: Sender<Sender<String>>,
}

#[instrument(level = "trace", skip_all, fields(human_number = player_setup.human_number))]
async fn message_handler<const N: usize>(
    player_setup: PlayerSetup,
    mut to_game: Sender<Message<N>>,
    from_game: Receiver<Message<N>>,
) {
    use Message::*;
    use ResponseState::*;

    let mut move_status = None;
    let mut draw_status = None;
    let mut undo_status = None;

    let PlayerSetup {
        name,
        human_number,
        input_sender,
        mut input_receiver,
        mut focus_sender,
    } = player_setup;

    let mut from_game = from_game.fuse();

    macro_rules! claim_input_focus {
        () => {{
            trace!("Claiming input focus.");
            if let Err(err) = focus_sender.send(input_sender.clone()).await {
                error!(?err, "Could not send focus request to stdin coordinator.");
            }
        }};
    }

    macro_rules! send {
        ($message:expr) => {{
            if let Err(err) = to_game.send($message).await {
                error!(?err, "Could not send message to game.");
            }
        }};
    }

    macro_rules! awaiting_input {
        () => {{
            move_status == Some(AwaitingInput)
                || draw_status == Some(AwaitingInput)
                || undo_status == Some(AwaitingInput)
        }};
    }

    loop {
        select! {
            message = from_game.next().fuse() => {
                match message {
                    Some(GameStart(color)) => {
                        trace!(assigned_color = ?color, "Game start received.");
                    }
                    Some(GameEnd(end)) => {
                        trace!(?end, "Game end received; exiting.");
                        break;
                    }
                    Some(MoveRequest(_state)) => {
                        trace!("Move request received.");
                        move_status = Some(AwaitingInput);

                        if undo_status == Some(Requested) {
                            undo_status = None;
                            println!("\nYour opponent ignored your undo request.");
                        }

                        if draw_status == Some(Requested) {
                            draw_status = None;
                            println!("\nYour opponent ignored your draw request.");
                        }

                        claim_input_focus!();
                    }
                    Some(UndoRequest) => {
                        trace!("Undo request received.");
                        undo_status = Some(AwaitingInput);
                        claim_input_focus!();
                        println!("\nYour opponent requests an undo.");
                        println!("Enter \"accept\" or \"reject\".");
                    }
                    Some(UndoRequestWithdrawal) => {
                        trace!("Undo request withdrawal received.");
                        undo_status = None;
                        println!("\nYour opponent withdrew their undo request.");
                    }
                    Some(UndoResponse { accept }) => {
                        trace!(?accept, "Undo response received.");
                        undo_status = None;

                        if awaiting_input!() {
                            claim_input_focus!();
                        }

                        if accept {
                            println!("\nYour opponent accepted your undo request.");
                        } else {
                            // Passive voice because we don't know if it was the opponent or the game.
                            println!("\nYour undo request was rejected.");
                        }
                    }
                    Some(DrawRequest) => {
                        trace!("Draw request received.");
                        draw_status = Some(AwaitingInput);
                        claim_input_focus!();
                        println!("\nYour opponent requests a draw.");
                        println!("Enter \"accept\" or \"reject\".");
                    }
                    Some(DrawRequestWithdrawal) => {
                        trace!("Draw request withdrawal received.");
                        draw_status = None;
                        println!("\nYour opponent withdrew their draw request.");
                    }
                    Some(DrawResponse { accept }) => {
                        trace!(?accept, "Draw response received.");
                        draw_status = None;

                        if awaiting_input!() {
                            claim_input_focus!();
                        }

                        if accept {
                            println!("\nYour opponent accepted your draw request.");
                        } else {
                            println!("\nYour opponent rejected your draw request.");
                        }
                    }
                    None => error!("Game hung up."),
                    message => error!(?message, "Unexpected message."),
                }
            }
            input = input_receiver.next().fuse() => {
                if input.is_none() {
                    error!("Input sender died?");
                }

                let input = input.unwrap();
                let input = input.trim();

                if draw_status == Some(Requested) {
                    if input == "cancel" {
                        draw_status = None;
                        send!(DrawRequestWithdrawal);
                        println!("\nDraw request withdrawn.");
                    }
                } else if undo_status == Some(Requested) {
                    if input == "cancel" {
                        undo_status = None;
                        send!(UndoRequestWithdrawal);
                        println!("\nUndo request withdrawn.");
                    }
                } else if draw_status == Some(AwaitingInput)
                    && (input == "accept" || input == "reject")
                {
                    if input == "accept" {
                        trace!("Sending draw accept.");
                        draw_status = None;
                        send!(DrawResponse { accept: true });
                    } else if input == "reject" {
                        trace!("Sending draw reject.");
                        draw_status = None;
                        send!(DrawResponse { accept: false });
                    }
                } else if undo_status == Some(AwaitingInput)
                    && (input == "accept" || input == "reject")
                {
                    if input == "accept" {
                        trace!("Sending undo accept.");
                        undo_status = None;
                        send!(UndoResponse { accept: true });
                    } else if input == "reject" {
                        trace!("Sending undo reject.");
                        undo_status = None;
                        send!(UndoResponse { accept: false });
                    }
                } else if !input.is_empty() {
                    if move_status == Some(AwaitingInput) {
                        if let Ok(ply) = input.parse::<Ply<N>>() {
                            trace!(?ply, "Sending move response.");
                            send!(MoveResponse(ply));
                            move_status = None;
                        }
                    }

                    if input == "draw" && draw_status.is_none() {
                        draw_status = Some(Requested);
                        send!(DrawRequest);
                    }

                    if input == "undo" && undo_status.is_none() {
                        undo_status = Some(Requested);
                        send!(UndoRequest);
                    }
                }
            }
        }

        if awaiting_input!() {
            let human_count = SETUP.lock().unwrap().human_count;

            let prompt = if human_count == 1 {
                print!("\nEntry");
                true
            } else {
                let requested = undo_status == Some(Requested) || draw_status == Some(Requested);

                if !requested {
                    if let Some(name) = &name {
                        print!("\n{name} (Player {human_number})");
                    } else {
                        print!("\nPlayer {human_number}");
                    }
                }

                !requested
            };

            if prompt {
                if draw_status == Some(Requested) {
                    print!(" (\"cancel\" to withdraw your draw request): ");
                } else if undo_status == Some(Requested) {
                    print!(" (\"cancel\" to withdraw your undo request): ");
                } else if draw_status == Some(AwaitingInput) {
                    print!(" (\"accept\" or \"reject\" the draw request): ");
                } else if undo_status == Some(AwaitingInput) {
                    print!(" (\"accept\" or \"reject\" the undo request): ");
                } else {
                    print!(": ");
                }
            }

            io::stdout().flush().ok();
        }
    }

    // Must drop this to remove another copy of the focus sender.
    mem::drop(focus_sender);

    let handle = SETUP.lock().ok().and_then(|mut setup| {
        setup.human_count -= 1;
        // If this is the last human player exiting, we need to take the
        // handle to the stdin coordinator and await it.
        if setup.human_count == 0 {
            debug!("This was the last human player; taking the stdin coordinator.");
            mem::drop(mem::take(&mut setup.focus_sender));
            mem::take(&mut setup.stdin_coordinator)
        } else {
            None
        }
    });

    // If we successfully took the stdin coordinator handle, await it.
    if let Some(handle) = handle {
        handle.await;
        debug!("The stdin coordinator shut down successfully.");
    }
}

async fn stdin_coordinator(mut focus_receiver: Receiver<Sender<String>>) {
    let mut focus: Option<Sender<String>> = None;
    let mut reader = BufReader::new(stdin()).lines().fuse();

    loop {
        select! {
            new_focus = focus_receiver.next().fuse() => {
                focus = new_focus;
                if focus.is_none() {
                    debug!("All focus senders hung up; exiting.");
                    break;
                }
            }
            input = reader.next().fuse() => {
                if input.is_none() {
                    error!("stdin closed; exiting.");
                    break;
                }

                match input.unwrap() {
                    Ok(input) => {
                        if let Some(focus) = &mut focus {
                            if let Err(err) = focus.send(input).await {
                                error!(?err, "Could not send input to focused receiver.");
                            }
                        }
                    }
                    Err(err) => {
                        error!(?err, "Could not read from stdin.");
                    }
                }
            }
        }
    }
}
