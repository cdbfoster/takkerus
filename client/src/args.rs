use std::ops::Deref;
use std::time::Duration;

use clap::error::ErrorKind as ClapErrorKind;
use clap::{
    Arg, ArgAction, ArgGroup, ArgMatches, Args as ArgsTrait, FromArgMatches, Parser, Subcommand,
};

use tak::{Color, Komi};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Presents a playable interface for human and AI players.
    Play(PlayConfig),
    /// Analyzes a given position.
    Analyze(AnalyzeConfig),
    /// Runs in TEI mode, using the limited subset of TEI that is supported by Racetrack. (https://github.com/MortenLohne/racetrack)
    Tei(TeiConfig),
}

#[derive(ArgsTrait, Clone, Debug)]
pub struct PlayConfig {
    #[command(flatten)]
    pub game: Game,

    /// A PTN file to load and play from. Headers in this file override `game` options.
    #[arg(short, long, verbatim_doc_comment)]
    pub load: Option<String>,

    /// A file to save the game to, in PTN format.
    #[arg(short, long, verbatim_doc_comment)]
    pub file: Option<String>,

    #[command(flatten)]
    pub p1: Player1,

    #[command(flatten)]
    pub p2: Player2,
}

#[derive(ArgsTrait, Clone, Debug)]
#[command(group(ArgGroup::new("input").required(true).args(["file", "tps"])))]
pub struct AnalyzeConfig {
    /// The name of a file in PTN format to analyze.
    #[arg(short, long, verbatim_doc_comment)]
    pub file: Option<String>,

    /// A position in TPS format to analyze. Must include the TPS tag in the form "[TPS \"...\"]".
    #[arg(short, long, verbatim_doc_comment)]
    pub tps: Option<String>,

    #[command(flatten)]
    pub ai: Ai,
}

#[derive(Clone, Debug)]
pub struct TeiConfig {
    pub ai: Ai,
}

#[derive(Clone, Debug)]
pub struct Player1(Player);

impl Default for Player1 {
    fn default() -> Self {
        Self(Player::Human(Human::default()))
    }
}

#[derive(Clone, Debug)]
pub struct Player2(Player);

impl Default for Player2 {
    fn default() -> Self {
        Self(Player::Ai(Ai::default()))
    }
}

macro_rules! impl_args_for_player {
    ($t:ty, $c:expr) => {
        impl ArgsTrait for $t {
            fn augment_args(cmd: clap::Command) -> clap::Command {
                Player::augment_args($c, cmd)
            }

            fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
                Self::augment_args(cmd)
            }
        }

        impl FromArgMatches for $t {
            fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
                Player::from_arg_matches($c, matches).map(Self)
            }

            fn from_arg_matches_mut(matches: &mut ArgMatches) -> Result<Self, clap::Error> {
                Self::from_arg_matches(matches)
            }

            fn update_from_arg_matches(
                &mut self,
                _matches: &ArgMatches,
            ) -> Result<(), clap::Error> {
                unimplemented!()
            }
        }

        impl Deref for $t {
            type Target = Player;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

impl_args_for_player!(Player1, Color::White);
impl_args_for_player!(Player2, Color::Black);

#[derive(Clone, Debug)]
pub enum Player {
    Human(Human),
    Ai(Ai),
}

impl Player {
    fn augment_args(color: Color, cmd: clap::Command) -> clap::Command {
        let field = match color {
            Color::White => "p1",
            Color::Black => "p2",
        };

        let number = match color {
            Color::White => 1,
            Color::Black => 2,
        };

        let color_word = match color {
            Color::White => "white",
            Color::Black => "black",
        };

        let default = match color {
            Color::White => "[default: type=human name=Anonymous]",
            Color::Black => "[default: type=ai time=20 early_stop=true]",
        };

        cmd.arg(
            Arg::new(field)
                .help(format!(
                    r#"Player {number} options ({color_word}).

General options:
  type=string       - The type of the player. (human or ai)

Human options:
{}

AI options:
{}

{default}"#,
                    Human::help(),
                    Ai::help(),
                ))
                .long(field)
                .value_name("OPTIONS")
                .num_args(1..)
                .action(ArgAction::Append),
        )
    }

    fn from_arg_matches(color: Color, matches: &ArgMatches) -> Result<Self, clap::Error> {
        let field = match color {
            Color::White => "p1",
            Color::Black => "p2",
        };

        if let Some(options) = matches.get_many::<String>(field) {
            for option in options {
                let (key, value) = option.split_once('=').ok_or_else(|| {
                    clap::Error::raw(
                        ClapErrorKind::InvalidValue,
                        format!("option must be in the form \"key=value\": {:?}", *option),
                    )
                })?;

                if key == "type" {
                    return match value {
                        "human" => Human::from_arg_matches(field, matches).map(Self::Human),
                        "ai" => Ai::from_arg_matches(
                            field,
                            matches,
                            Ai {
                                time_limit: Some(Duration::from_secs(20)),
                                ..Ai::default()
                            },
                        )
                        .map(Self::Ai),
                        _ => Err(clap::Error::raw(
                            ClapErrorKind::InvalidValue,
                            format!("invalid value for type: {value:?}"),
                        )),
                    };
                }
            }

            return Err(clap::Error::raw(
                ClapErrorKind::MissingRequiredArgument,
                "missing player type",
            ));
        }

        match color {
            Color::White => Ok(Player1::default().0),
            Color::Black => Ok(Player2::default().0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Human {
    pub name: String,
}

impl Human {
    fn help() -> String {
        "  name=string       - The player's name.".to_owned()
    }

    fn from_arg_matches(field: &str, matches: &ArgMatches) -> Result<Self, clap::Error> {
        let mut human = Self::default();

        if let Some(options) = matches.get_many::<String>(field) {
            for option in options {
                let (key, value) = option.split_once('=').ok_or_else(|| {
                    clap::Error::raw(
                        ClapErrorKind::InvalidValue,
                        format!("option must be in the form \"key=value\": {:?}", *option),
                    )
                })?;

                if key == "name" {
                    human.name = value.to_owned();
                }
            }
        }

        Ok(human)
    }
}

impl Default for Human {
    fn default() -> Self {
        Self {
            name: "Anonymous".to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Ai {
    pub depth_limit: Option<u32>,
    pub time_limit: Option<Duration>,
    pub early_stop: bool,
    pub exact_eval: bool,
    pub threads: usize,
    pub model_file: Option<String>,
}

impl Ai {
    fn help() -> String {
        r#"  depth=int         - The maximum depth of the move search.
  time=int          - The maximum number of seconds to spend considering a response.
  early_stop=bool   - Stop the search early if the next depth is predicted to take longer
                      than the time limit. `time` or `tc` must be set. (false or true)
  exact=bool        - If true, don't use search enhancements that produce inexact results.
                      This generally makes a search slower and a bot weaker, but can be used
                      if the accuracy of results is a priority over playing strength.
  threads=int       - The number of worker threads to spawn for analysis.
  model=path        - The path of a JSON file to load as the evaluator model."#
            .to_owned()
    }

    fn from_arg_matches(
        field: &str,
        matches: &ArgMatches,
        default: Self,
    ) -> Result<Self, clap::Error> {
        if let Some(options) = matches.get_many::<String>(field) {
            let mut ai = Self {
                depth_limit: None,
                time_limit: None,
                early_stop: false,
                exact_eval: false,
                threads: 1,
                model_file: None,
            };

            for option in options {
                let (key, value) = option.split_once('=').ok_or_else(|| {
                    clap::Error::raw(
                        ClapErrorKind::InvalidValue,
                        format!("option must be in the form \"key=value\": {:?}", *option),
                    )
                })?;

                match key {
                    "depth" => {
                        ai.depth_limit = Some(value.parse::<u32>().map_err(|_| {
                            clap::Error::raw(
                                ClapErrorKind::InvalidValue,
                                format!("invalid value for depth: {value:?}"),
                            )
                        })?);
                    }
                    "time" => {
                        ai.time_limit = Some(Duration::from_secs_f32(
                            value.parse::<f32>().map_err(|_| {
                                clap::Error::raw(
                                    ClapErrorKind::InvalidValue,
                                    format!("invalid value for time: {value:?}"),
                                )
                            })?,
                        ));
                    }
                    "early_stop" => {
                        ai.early_stop = value.parse::<bool>().map_err(|_| {
                            clap::Error::raw(
                                ClapErrorKind::InvalidValue,
                                format!("invalid value for early_stop: {value:?}"),
                            )
                        })?;
                    }
                    "exact" => {
                        ai.exact_eval = value.parse::<bool>().map_err(|_| {
                            clap::Error::raw(
                                ClapErrorKind::InvalidValue,
                                format!("invalid value for exact: {value:?}"),
                            )
                        })?;
                    }
                    "threads" => {
                        ai.threads = value.parse::<usize>().map_err(|_| {
                            clap::Error::raw(
                                ClapErrorKind::InvalidValue,
                                format!("invalid value for threads: {value:?}"),
                            )
                        })?;
                    }
                    "model" => {
                        ai.model_file = Some(value.to_owned());
                    }
                    _ => (),
                }
            }

            Ok(ai)
        } else {
            Ok(default)
        }
    }
}

impl Default for Ai {
    fn default() -> Self {
        Self {
            depth_limit: None,
            time_limit: Some(Duration::from_secs(60)),
            early_stop: true,
            exact_eval: false,
            threads: 1,
            model_file: None,
        }
    }
}

impl ArgsTrait for Ai {
    fn augment_args(cmd: clap::Command) -> clap::Command {
        cmd.arg(
            Arg::new("ai")
                .help(format!(
                    "Analysis options.\n\nOptions:\n{}\n\n[default: time=60 early_stop=true]",
                    Self::help()
                ))
                .long("ai")
                .value_name("OPTIONS")
                .num_args(1..)
                .action(ArgAction::Append),
        )
    }

    fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
        Self::augment_args(cmd)
    }
}

impl FromArgMatches for Ai {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        Self::from_arg_matches("ai", matches, Self::default())
    }

    fn from_arg_matches_mut(matches: &mut ArgMatches) -> Result<Self, clap::Error> {
        <Self as FromArgMatches>::from_arg_matches(matches)
    }

    fn update_from_arg_matches(&mut self, _matches: &ArgMatches) -> Result<(), clap::Error> {
        unimplemented!()
    }
}

impl ArgsTrait for TeiConfig {
    fn augment_args(cmd: clap::Command) -> clap::Command {
        cmd.arg(
            Arg::new("ai")
                .help(format!(
                    "Analysis options.\n\nOptions:\n{}\n\n[default: threads=1]",
                    Ai::help()
                ))
                .long("ai")
                .value_name("OPTIONS")
                .num_args(1..)
                .action(ArgAction::Append),
        )
    }

    fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
        Self::augment_args(cmd)
    }
}

impl FromArgMatches for TeiConfig {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        Ai::from_arg_matches(
            "ai",
            matches,
            Ai {
                depth_limit: None,
                time_limit: None,
                early_stop: true,
                exact_eval: false,
                threads: 1,
                model_file: None,
            },
        )
        .map(|ai| Self { ai })
    }

    fn from_arg_matches_mut(matches: &mut ArgMatches) -> Result<Self, clap::Error> {
        Self::from_arg_matches(matches)
    }

    fn update_from_arg_matches(&mut self, _matches: &ArgMatches) -> Result<(), clap::Error> {
        unimplemented!()
    }
}

impl Deref for TeiConfig {
    type Target = Ai;

    fn deref(&self) -> &Self::Target {
        &self.ai
    }
}

#[derive(Clone, Debug)]
pub struct Game {
    pub size: usize,
    pub komi: Komi,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            size: 6,
            komi: Default::default(),
        }
    }
}

impl ArgsTrait for Game {
    fn augment_args(cmd: clap::Command) -> clap::Command {
        cmd.arg(
            Arg::new("game")
                .help(
                    r#"Game options.

Options:
  size=int          - The size of the board. (3 - 8)
  komi=decimal      - A bonus to apply to black's flat count in games that go to flats.
                      (-5.0 - +5.0, in 0.5 increments)

[default: size=6]"#,
                )
                .short('g')
                .long("game")
                .value_name("OPTIONS")
                .num_args(1..)
                .action(ArgAction::Append),
        )
    }

    fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
        Self::augment_args(cmd)
    }
}

impl FromArgMatches for Game {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        let mut game = Self::default();

        if let Some(options) = matches.get_many::<String>("game") {
            for option in options {
                let (key, value) = option.split_once('=').ok_or_else(|| {
                    clap::Error::raw(
                        ClapErrorKind::InvalidValue,
                        format!("option must be in the form \"key=value\": {:?}", *option),
                    )
                })?;

                match key {
                    "size" => {
                        game.size = value.parse::<usize>().map_err(|_| {
                            clap::Error::raw(
                                ClapErrorKind::InvalidValue,
                                format!("invalid value for size: {value:?}"),
                            )
                        })?;
                    }
                    "komi" => {
                        game.komi = value.parse::<Komi>().map_err(|_| {
                            clap::Error::raw(
                                ClapErrorKind::InvalidValue,
                                format!("invalid value for komi: {value:?}"),
                            )
                        })?;
                    }
                    _ => {
                        return Err(clap::Error::raw(
                            ClapErrorKind::UnknownArgument,
                            format!("unknown option for game: {key:?}"),
                        ))
                    }
                }
            }
        }

        Ok(game)
    }

    fn from_arg_matches_mut(matches: &mut ArgMatches) -> Result<Self, clap::Error> {
        Self::from_arg_matches(matches)
    }

    fn update_from_arg_matches(&mut self, _matches: &ArgMatches) -> Result<(), clap::Error> {
        unimplemented!()
    }
}
