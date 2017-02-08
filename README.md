# Takkerus
Takkerus implements a [Tak](http://cheapass.com/node/215) AI and a playable Tak board.  The name comes from a respelling of "tak-rs", after [Rust](https://www.rust-lang.org/)'s crate-naming conventions.

## Source
Check out the source by cloning the repository:

    $ git clone https://github.com/cdbfoster/takkerus.git
    
If you don't want to use `git`, or would prefer a .zip, you can download one from [here](https://github.com/cdbfoster/takkerus/archive/master.zip).

## Rust
Takkerus requires the nightly release channel.  If using `rustup`, run:

    $ rustup install nightly
    
followed by running:

    $ rustup default nightly

to set the global default toolchain, or:

    $ rustup override set nightly

from inside the root of the repository to set the toolchain locally.

If not using `rustup`, you will need to install nightly Rust via a standalone installer.  Windows and Mac users can find the correct installer for their platform [here](https://www.rust-lang.org/en-US/other-installers.html#standalone) (probably the **x86_64-pc-windows-gnu .msi** for Windows or the **x86_64-apple-darwin .pkg** for Mac).  Linux users can use their distribution's package manager.

**Note**: Windows users that aren't using `rustup` will have to uninstall other versions of Rust in order for the nightly version to work.

## Building
Use Rust's package manager, Cargo, to build Takkerus.  To do so, simply run:

    $ cargo build --release

from anywhere within the repository.  Cargo will automatically pull in and build the dependencies.

To build and run the program in one step, run:

    $ cargo run --release

You can pass command line options to the program by separating them from `cargo run --release` with `--`; for instance, `cargo run --release -- options go here`.

## Use
To use the program, either run `cargo run --release` like above, or run the compiled program from the output directory:

    $ target/release/takkerus

for example, from the root of the repository (Or `target/debug/takkerus` if Cargo was run without the `--release` flag, above).  Use `takkerus --help` to learn more.

### Examples
To play a 5x5 game against a strong bot:

    $ takkerus

which is equivalent to:

    $ takkerus play -s 5 --p1 human --p2 pvsearch -g 60
    
To analyze the next best moves for a position given in a [PTN file](https://www.reddit.com/r/Tak/wiki/ptn_file_format):

    $ takkerus analyze -f my_ptn_file

## Contact
Questions and comments can be sent to my email, cdbfoster@gmail.com

© 2016-2017 Chris Foster
