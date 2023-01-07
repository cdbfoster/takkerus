use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use clap::{ArgGroup, Args as ArgsTrait, Parser, Subcommand};

use tak::HalfKomi;

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
}

#[derive(ArgsTrait, Clone, Debug)]
pub struct PlayConfig {
    /// Player 1 options.
    ///
    /// General parameters:
    ///   type=string       - The type of the player. (human, ai, or playtak)
    ///
    /// Human parameters:
    ///   name=string       - The player's name.
    ///
    /// AI parameters:
    ///   depth=int         - The maximum depth of the move search.
    ///   time=int          - The maximum number of seconds to spend considering a response.
    ///   predict_time=bool - Stop the search early if the next depth is predicted to take longer
    ///                       than the time limit. Time limit must be set. (false or true)
    #[arg(long, default_value = "type=human", verbatim_doc_comment)]
    pub p1: Player,

    /// Player 2 options. These are the same as the options for Player 1.
    #[arg(
        long,
        default_value = "type=ai,time=60,predict_time=true",
        verbatim_doc_comment
    )]
    pub p2: Player,

    /// Game options.
    ///
    /// Parameters:
    ///   size=int     - The size of the board. (3 - 8)
    ///   komi=decimal - A bonus to apply to black's flat count in games that go to flats.
    ///                  (-5.0 - +5.0, in 0.5 increments)
    #[arg(short, long, default_value = "size=6", verbatim_doc_comment)]
    pub game: Game,

    /// A PTN file to load and play from. Headers in this file override `game` options.
    #[arg(short, long, verbatim_doc_comment)]
    pub load: Option<String>,

    /// A file to save the game to, in PTN format.
    #[arg(short, long, verbatim_doc_comment)]
    pub file: Option<String>,
}

#[derive(ArgsTrait, Clone, Debug)]
#[command(group(ArgGroup::new("input").required(true).args(["file", "tps"])))]
pub struct AnalyzeConfig {
    /// Analysis options.
    ///
    /// Parameters:
    ///   depth=int         - The maximum depth of the move search.
    ///   time=int          - The maximum number of seconds to spend considering a response.
    ///   predict_time=bool - Stop the search early if the next depth is predicted to take longer
    ///                       than the time limit. Time limit must be set. (false or true)
    #[arg(
        long,
        default_value = "time=60,predict_time=true",
        verbatim_doc_comment
    )]
    pub ai: Ai,

    /// The name of a file in PTN format to analyze.
    #[arg(short, long, verbatim_doc_comment)]
    pub file: Option<String>,

    /// A position in TPS format to analyze. Must include the TPS tag in the form "[TPS \"...\"]".
    #[arg(short, long, verbatim_doc_comment)]
    pub tps: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Player {
    Human(Human),
    Ai(Ai),
}

impl FromStr for Player {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fields = parse_map(s)?;

        match *fields
            .get("type")
            .ok_or_else(|| "no type field".to_owned())?
        {
            "human" => Ok(Self::Human(Human::from_str(s)?)),
            "ai" => Ok(Self::Ai(Ai::from_str(s)?)),
            unknown => Err(format!("unknown player type: {unknown}")),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Human {
    pub name: String,
}

impl FromStr for Human {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fields = parse_map(s)?;

        let name = fields
            .get("name")
            .map(|&f| f.to_owned())
            .unwrap_or_else(|| "Anonymous".to_owned());

        Ok(Self { name })
    }
}

#[derive(Clone, Debug)]
pub struct Ai {
    pub depth_limit: Option<u32>,
    pub time_limit: Option<Duration>,
    pub predict_time: bool,
}

impl FromStr for Ai {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fields = parse_map(s)?;

        let depth_limit = fields
            .get("depth")
            .map(|&f| {
                f.parse::<u32>()
                    .map_err(|_| format!("invalid value for depth: {f}"))
            })
            .transpose()?;

        let time_limit = fields
            .get("time")
            .map(|&f| {
                f.parse::<u64>()
                    .map_err(|_| format!("invalid value for time: {f}"))
                    .map(Duration::from_secs)
            })
            .transpose()?;

        let predict_time = fields
            .get("predict_time")
            .map(|&f| {
                f.parse::<bool>()
                    .map_err(|_| format!("invalid value for predict_time: {f}"))
            })
            .transpose()?
            .unwrap_or_default();

        if predict_time && time_limit.is_none() {
            return Err("time limit must be set to use predict_time".to_owned());
        }

        Ok(Self {
            depth_limit,
            time_limit,
            predict_time,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Game {
    pub size: usize,
    pub half_komi: HalfKomi,
}

impl FromStr for Game {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fields = parse_map(s)?;

        let size = fields
            .get("size")
            .map(|&f| {
                f.parse::<usize>()
                    .map_err(|_| format!("invalid value for size: {f}"))
            })
            .transpose()?
            .unwrap_or(6);
        if !(3..=8).contains(&size) {
            return Err(format!("invalid value for size (3 - 8): {size}"));
        }

        let half_komi = fields
            .get("komi")
            .map(|&f| f.parse::<HalfKomi>())
            .transpose()?
            .unwrap_or(HalfKomi(0));
        if !(-10..=10).contains(&*half_komi) {
            return Err("komi values must be between -5.0 and +5.0".to_owned());
        }

        Ok(Game { size, half_komi })
    }
}

fn parse_map(string: &str) -> Result<HashMap<&str, &str>, String> {
    string
        .split(',')
        .map(|field| field.trim())
        .map(|field| field.split('=').map(|part| part.trim()))
        .map(|mut field_part| {
            let key = field_part
                .next()
                .ok_or_else(|| "no key for field".to_owned())?;
            let value = field_part
                .next()
                .ok_or_else(|| format!("no value for key: {key}"))?;
            Ok((key, value))
        })
        .collect()
}
