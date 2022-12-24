use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Play {
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
        p1: Player,

        /// Player 2 options. These are the same as the options for Player 1.
        #[arg(
            long,
            default_value = "type=ai,time=60,predict_time=true",
            verbatim_doc_comment
        )]
        p2: Player,

        /// Game options.
        ///
        /// Parameters:
        ///   size=int     - The size of the board. (3 - 8)
        ///   komi=decimal - A bonus to apply to black's flat count in games that go to flats.
        ///                  (-5.0 - +5.0, in 0.5 increments)
        #[arg(short, long, default_value = "size=6", verbatim_doc_comment)]
        game: Game,
    },
    Analyze {
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
        ai: Ai,
    },
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
    pub predict_time: Option<bool>,
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
            .transpose()?;

        if predict_time == Some(true) && time_limit.is_none() {
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
    pub half_komi: i8,
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
            .map(|&f| {
                let half_komi = if let Some(period) = f.find('.') {
                    let full = 2 * f[..period]
                        .parse::<i8>()
                        .map_err(|_| format!("invalid value for komi: {f}"))?;
                    let half = match &f[period + 1..] {
                        "0" => 0,
                        "5" => 1,
                        _ => return Err("only half komi are supported (*.0 or *.5)".to_owned()),
                    };
                    let sign = if full >= 0 { 1 } else { -1 };
                    full + sign * half
                } else {
                    2 * f
                        .parse::<i8>()
                        .map_err(|_| format!("invalid value for komi: {f}"))?
                };
                Ok(half_komi)
            })
            .transpose()?
            .unwrap_or(0);
        if !(-10..=10).contains(&half_komi) {
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
