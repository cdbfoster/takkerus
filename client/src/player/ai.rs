use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_std::prelude::*;
use async_std::task;
use futures::channel::mpsc::{self, UnboundedReceiver as Receiver, UnboundedSender as Sender};
use futures::{select, FutureExt, SinkExt};
use tracing::{error, trace, warn};

use analysis::{self, analyze, AnalysisConfig, PersistentState};

use crate::play::{Message, Player};

pub fn initialize<const N: usize>(
    depth_limit: Option<u32>,
    time_limit: Option<Duration>,
    predict_time: bool,
    to_game: Sender<Message<N>>,
) -> Player<N> {
    let name = Some(format!("Takkerus v{}", analysis::version()));

    trace!(?name, "Initializing an AI player.");

    let (to_player, from_game) = mpsc::unbounded();

    Player {
        name,
        to_player,
        task: task::spawn(message_handler::<N>(
            depth_limit,
            time_limit,
            predict_time,
            to_game,
            from_game,
        )),
        color_select: None,
    }
}

async fn message_handler<const N: usize>(
    depth_limit: Option<u32>,
    time_limit: Option<Duration>,
    predict_time: bool,
    mut to_game: Sender<Message<N>>,
    from_game: Receiver<Message<N>>,
) {
    use Message::*;

    let persistent_state = Arc::new(PersistentState::<N>::default());

    let mut interrupt: Option<Arc<AtomicBool>> = None;
    let (analysis_sender, analysis_receiver) = mpsc::unbounded();

    let mut from_game = from_game.fuse();
    let mut analysis_receiver = analysis_receiver.fuse();

    loop {
        select! {
            message = from_game.next().fuse() => {
                match message {
                    Some(GameStart(color)) => {
                        trace!(assigned_color = ?color, "Game start received.");
                    }
                    Some(GameEnd(end)) => {
                        trace!(?end, "Game end received; exiting.");

                        if interrupt.is_some() {
                            warn!("Analysis was in progress when the game ended.");
                            interrupt.unwrap().store(true, Ordering::Relaxed);
                        }

                        break;
                    }
                    Some(MoveRequest(state)) => {
                        trace!("Move request received.");
                        if interrupt.is_some() {
                            error!("Move request received while analyzing.");
                        }

                        let persistent_state = persistent_state.clone();

                        let interrupted = Arc::new(AtomicBool::new(false));
                        interrupt = Some(interrupted.clone());

                        let mut analysis_sender = analysis_sender.clone();

                        task::spawn_blocking(move || {
                            println!("\nAnalyzing...");

                            let analysis = {
                                let analysis_config = AnalysisConfig {
                                    depth_limit,
                                    time_limit,
                                    predict_time,
                                    interrupted,
                                    persistent_state: Some(&persistent_state),
                                    ..Default::default()
                                };

                                trace!("Analyzing state.");
                                analyze(analysis_config, &state)
                            };

                            let _result = task::block_on(analysis_sender.send(analysis));
                        });
                    }
                    _ => (),
                }
            }
            next_analysis = analysis_receiver.next().fuse() => {
                if next_analysis.is_none() {
                    error!("Analysis sender died?");
                }

                let next_analysis = next_analysis.unwrap();

                if let Some(&next_move) = next_analysis.principal_variation.first() {
                    let message = MoveResponse(next_move);
                    if let Err(err) = to_game.send(message).await {
                        error!(?err, "Could not send message to game.");
                    }
                } else {
                    error!("Returned analysis contained no moves.");
                }

                interrupt = None;
            }
        }
    }
}
