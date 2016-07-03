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
// Copyright 2016 Chris Foster
//

use std::collections::HashMap;
use std::env::Args;
use std::fmt;
use std::iter::Peekable;

#[derive(Clone)]
pub enum Type {
    Flag(&'static str),
    Option(&'static str, &'static str, u8),
}

pub fn collect_next(source: &mut Peekable<Args>, arguments: &[Type]) -> Result<HashMap<&'static str, Vec<String>>, ArgumentError> {
    let mut results = HashMap::new();

    loop {
        let match_peek = {
            let peek = source.peek();

            if peek.is_none() {
                break;
            }

            let peek_string = peek.unwrap().clone();

            let mut match_peek = None;

            for argument in arguments.iter() {
                match argument {
                    &Type::Flag(string) => if peek_string == string {
                        match_peek = Some(argument.clone());
                        break;
                    },
                    &Type::Option(string_1, string_2, _) => {
                        if peek_string == string_1 || peek_string == string_2 {
                            match_peek = Some(argument.clone());
                            break;
                        }
                    },
                }
            }

            match_peek
        };

        if match_peek.is_none() {
            break;
        }

        let source_string = source.next().unwrap();

        match match_peek.unwrap() {
            Type::Flag(string) => {
                results.insert(string, Vec::new());
            },
            Type::Option(string_1, string_2, expected) => {
                let mut parameters = Vec::new();
                for found in 0..expected {
                    {
                        let peek = source.peek();

                        if peek.is_none() {
                            return Err(ArgumentError::NotEnoughParameters(
                                if source_string == string_1 {
                                    string_1
                                } else {
                                    string_2
                                },
                                expected,
                                found,
                            ));
                        }

                        parameters.push(peek.unwrap().clone());
                    }

                    source.next();
                }

                results.insert(string_2, parameters);
            },
        }

        if results.len() == arguments.len() {
            break;
        }
    }

    Ok(results)
}

pub enum ArgumentError {
    NotEnoughParameters(&'static str, u8, u8),
}

impl fmt::Display for ArgumentError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &ArgumentError::NotEnoughParameters(option, expected, found) => {
                write!(f, "Not enough parameters for option '{}'.  Expected {}, found {}.", option, expected, found)
            },
        }
    }
}
