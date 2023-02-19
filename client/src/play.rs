use std::mem;

use async_std::prelude::*;
use async_std::task::{self, JoinHandle};
use futures::channel::mpsc::{self, UnboundedReceiver as Receiver, UnboundedSender as Sender};
use futures::{select, FutureExt, SinkExt};
use tracing::{debug, error, instrument, trace, warn};

use tak::{Color, Ply, PlyError, PtnError, PtnGame, PtnHeader, Resolution, State, StateError};

use crate::args::{Game, PlayConfig, Player as PlayerArgs};

use crate::player::{ai, human};

pub struct Player<const N: usize> {
    pub name: Option<String>,
    pub to_player: Sender<Message<N>>,
    pub task: JoinHandle<()>,
    pub color_select: Option<Color>,
}

pub trait PlayerInitializer<const N: usize>: Fn(Sender<Message<N>>) -> Player<N> {}

impl<T, const N: usize> PlayerInitializer<N> for T where T: Fn(Sender<Message<N>>) -> Player<N> {}

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

use self::GameEnd as GameEndType;

#[derive(Debug)]
pub enum GameEnd {
    Resolution(Resolution),
    //Resignation(Color),
}

pub fn run_game(mut config: PlayConfig) {
    let game = if let Some(load) = &config.load {
        match PtnGame::from_file(load) {
            Ok(game) => {
                if let Some(size_config) =
                    game.get_header("Size").map(|h| format!("size={}", h.value))
                {
                    match size_config.parse::<Game>() {
                        Ok(new_game) => config.game.size = new_game.size,
                        Err(err) => {
                            error!(error = %err, "Could not read PTN file.");
                            return;
                        }
                    }
                }

                if let Some(komi_config) =
                    game.get_header("Komi").map(|h| format!("komi={}", h.value))
                {
                    match komi_config.parse::<Game>() {
                        Ok(new_game) => config.game.komi = new_game.komi,
                        Err(err) => {
                            error!(error = %err, "Could not read PTN file.");
                            return;
                        }
                    }
                }

                Some(game)
            }
            Err(err) => {
                error!(error = ?err, "Could not load PTN file.");
                return;
            }
        }
    } else {
        None
    };

    match config.game.size {
        3 => run_game_sized::<3>(config, game),
        4 => run_game_sized::<4>(config, game),
        5 => run_game_sized::<5>(config, game),
        6 => run_game_sized::<6>(config, game),
        7 => run_game_sized::<7>(config, game),
        8 => run_game_sized::<8>(config, game),
        _ => unreachable!(),
    }
}

fn run_game_sized<const N: usize>(config: PlayConfig, game: Option<PtnGame>) {
    // Make sure any state we load from a file is valid.
    if let Some(game) = &game {
        if let Err(err) = game.validate::<N>() {
            error!(error = ?err, "Game state is invalid.");
            return;
        }
    }

    let p1_initialize = initialize_player::<N>(&config.p1);
    let p2_initialize = initialize_player::<N>(&config.p2);

    let (to_game, from_p1) = mpsc::unbounded();
    let p1 = p1_initialize(to_game);

    let (to_game, from_p2) = mpsc::unbounded();
    let p2 = p2_initialize(to_game);

    mem::drop(p1_initialize);
    mem::drop(p2_initialize);

    let mut game = game.unwrap_or_else(|| PtnGame {
        headers: vec![
            PtnHeader::new("Site", "Local"),
            PtnHeader::new("Player1", p1.name.as_deref().unwrap_or("Anonymous")),
            PtnHeader::new("Player2", p2.name.as_deref().unwrap_or("Anonymous")),
        ],
        ..Default::default()
    });

    // Ensure that all games have valid Size and Komi headers.
    game.add_header("Size", N);
    game.add_header("Komi", config.game.komi);

    task::block_on(game_handler(p1, from_p1, p2, from_p2, config, game));
}

fn initialize_player<const N: usize>(player: &PlayerArgs) -> impl PlayerInitializer<N> + '_ {
    match player {
        PlayerArgs::Human(config) => {
            Box::new(|to_game| human::initialize(Some(config.name.clone()), to_game))
                as Box<dyn PlayerInitializer<N>>
        }
        PlayerArgs::Ai(config) => Box::new(|to_game| {
            ai::initialize(
                config.depth_limit,
                config.time_limit,
                config.predict_time,
                to_game,
            )
        }) as Box<dyn PlayerInitializer<N>>,
    }
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

macro_rules! state {
    ($game:expr) => {{
        let state: State<N> = $game.clone().try_into().expect("cannot retrieve state");
        state
    }};
}

macro_rules! plies {
    ($game:expr) => {{
        let plies: Vec<Ply<N>> = $game.get_plies().expect("cannot retrieve state");
        plies
    }};
}

#[instrument(level = "trace", skip_all)]
async fn game_handler<const N: usize>(
    mut p1: Player<N>,
    from_p1: Receiver<Message<N>>,
    mut p2: Player<N>,
    from_p2: Receiver<Message<N>>,
    config: PlayConfig,
    mut game: PtnGame,
) {
    use Message::*;
    use PlayerToken::*;

    // Resolve colors, based on selections.
    if p1.color_select.is_some() && p2.color_select.is_some() && p1.color_select == p2.color_select
    {
        error!(p1 = ?p1.color_select, p2 = ?p2.color_select, "Both players requested the same color.");
        return;
    }

    let p1_color = p1
        .color_select
        .or_else(|| p2.color_select.map(|c| c.other()))
        .unwrap_or(Color::White);
    let p2_color = p1_color.other();

    macro_rules! save_game {
        () => {{
            if let Some(filename) = &config.file {
                if let Err(err) = game.to_file(filename) {
                    error!(error = ?err, "Could not save PTN file.");
                } else {
                    debug!(?filename, "PTN file saved.");
                }
            }
        }};
    }
    save_game!();

    let mut from_p1 = from_p1.fuse();
    let mut from_p2 = from_p2.fuse();

    macro_rules! send {
        ($p:expr, $message:expr) => {{
            let result = match $p {
                Player1 => p1.to_player.send($message).await,
                Player2 => p2.to_player.send($message).await,
            };
            if let Err(err) = result {
                error!(?err, player = ?$p, "Could not send message to player.");
            }
        }};
    }

    send!(Player1, GameStart(p1_color));
    send!(Player2, GameStart(p2_color));

    print_board::<N>(&game);

    macro_rules! player_to_move {
        ($state:expr) => {{
            if p1_color == $state.to_move() {
                Player1
            } else {
                Player2
            }
        }};
    }

    macro_rules! game_resolution {
        ($resolution:ident) => {{
            print_resolution($resolution);
            send!(Player1, GameEnd(GameEndType::Resolution($resolution)));
            send!(Player2, GameEnd(GameEndType::Resolution($resolution)));
        }};
    }

    {
        let state = state!(game);

        if let Some(resolution) = state.resolution() {
            game_resolution!(resolution);
            return;
        }

        let player_to_move = player_to_move!(state);
        send!(player_to_move, MoveRequest(state));
    }

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
                if from != player_to_move!(state!(game)) {
                    warn!("Received a move response from the wrong player.");
                    continue;
                }

                if handle_ply(&mut game, ply).is_ok() {
                    save_game!();
                    print_board::<N>(&game);
                    let state = state!(game);
                    if let Some(resolution) = state.resolution() {
                        game_resolution!(resolution);
                        break;
                    }
                    send!(from.other(), MoveRequest(state));
                } else {
                    send!(from, MoveRequest(state!(game)));
                }
            }
            UndoRequest => {
                if plies!(game).is_empty() {
                    // No history to undo, so reject the request.
                    send!(from, UndoResponse { accept: false });
                } else {
                    send!(from.other(), UndoRequest);
                }
            }
            UndoRequestWithdrawal => send!(from.other(), UndoRequestWithdrawal),
            UndoResponse { accept } => {
                if accept {
                    if plies!(game).is_empty() {
                        error!("Undo request was accepted, but there is no more history.");
                    } else {
                        game.remove_last_ply::<N>()
                            .expect("could not remove last ply");
                        save_game!();
                        let state = state!(game);
                        let player_to_move = player_to_move!(state);
                        send!(from.other(), message);
                        send!(player_to_move, MoveRequest(state));
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

    save_game!();
}

fn handle_ply<const N: usize>(game: &mut PtnGame, ply: Ply<N>) -> Result<(), StateError> {
    if let Err(err) = game.add_ply(ply) {
        match err {
            PtnError::StateError(err) => {
                let message = match err {
                    StateError::PlyError(PlyError::InvalidCrush) => "Invalid crush.",
                    StateError::PlyError(PlyError::InvalidDrops(message)) => message,
                    StateError::PlyError(PlyError::OutOfBounds) => "Out of bounds.",
                    StateError::InvalidPlace(message) => message,
                    StateError::InvalidSpread(message) => message,
                    StateError::NoPreviousPlies => unreachable!(),
                };
                println!("\nError: {message}");
                return Err(err);
            }
            _ => panic!("cannot add ply"),
        }
    }
    Ok(())
}

fn print_board<const N: usize>(game: &PtnGame) {
    let state = state!(game);

    println!("\n--------------------------------------------------");

    println!("\n{state}");

    if let Some(last_turn) = game.turns.last() {
        let turn_number = format!("{}.", last_turn.number);

        let p1_move = match &last_turn.p1_move.ply {
            Some(ply) => ply.to_string(),
            None => "--".to_owned(),
        };

        let p2_move = match &last_turn.p2_move.ply {
            Some(ply) => ply.to_string(),
            None => "--".to_owned(),
        };

        println!(
            "\n  {turn_number:<3}  {:<width$}{}",
            p1_move,
            p2_move,
            width = N + 4
        );
    } else {
        println!("\n  1.   --");
    }
}

fn print_resolution(resolution: Resolution) {
    println!("\nGame over.");
    match resolution {
        Resolution::Road(color) => {
            println!(
                "\n{color:?} wins by road: {}",
                match color {
                    Color::White => "R-0",
                    Color::Black => "0-R",
                }
            );
        }
        Resolution::Flats {
            color,
            spread,
            komi,
        } => {
            let full_komi = komi.as_half_komi().abs() / 2;
            let remainder = (komi.as_half_komi().abs() % 2) * 5;
            println!(
                "\n{color:?} wins by flats: {}",
                match color {
                    Color::White => "F-0",
                    Color::Black => "0-F",
                },
            );
            println!(
                "  Spread: {spread}{}{full_komi}.{remainder}",
                match color {
                    Color::White => "-",
                    Color::Black => "+",
                }
            );
        }
        Resolution::Draw => {
            println!("\nDraw: ½-½");
        }
    }
}
