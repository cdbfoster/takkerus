# Takkerus
Takkerus implements a [Tak](http://cheapass.com/node/215) AI and a playable Tak board.  The name comes from a respelling of "tak-rs", after Rust's crate-naming conventions.

## Source
Check out the source by cloning the repository:

    $ git clone https://github.com/cdbfoster/takkerus.git

## Building
It is highly recommended to use Rust's package manager, Cargo, to build Takkerus.  To do so, simply run:

    $ cargo build

from anywhere within the repository.  Cargo will automatically pull in and build the dependencies.

To build and run the program in one step, run:

    $ cargo run --release

to compile the program (In "release" mode, with optimizations.  Highly recommended for this kind of program) and then run it if compilations succeeds.  You can pass command line options to the program by separating them from `cargo run --release` with `--`; for instance, `cargo run --release -- options go here`.

## Using
To use the program, either run `cargo run` like above, or run the compiled program from the output directory:

    $ target/release/takkerus

for example, from the root of the repository.  Use `takkerus --help` to learn more.

### Examples
To play a 5x5 game against a strong bot:

    $ target/release/takkerus

or

    $ target/release/takkerus play -s 5 -p1 human -p2 minimax -d 5
    
To analyze the next best moves for a position given in [TPS](https://www.reddit.com/r/Tak/wiki/tak_positional_system) format:

    $ target/release/takkerus analyze -t "[TPS \"112S,12S,x1,1,x1/2,2221C,22112C,x2/x1,22,2,12,x1/2,22,x1,12,x1/21,x2,21,x1 1 35\"]" -a minimax -d 5

## Contact
Questions and comments can be sent to my email, cdbfoster@gmail.com

© 2016 Chris Foster
