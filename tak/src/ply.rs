use std::fmt;
use std::mem;
use std::ops::RangeInclusive;

use once_cell::sync::Lazy;
use tracing::{instrument, trace};

use crate::bitmap::Bitmap;
use crate::piece::{Color, Piece, PieceType};
use crate::ptn::PtnPly;
use crate::stack::{Stack, StackBitmap};
use crate::state::{PlyValidation, State};

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum Ply<const N: usize> {
    Place {
        x: u8,
        y: u8,
        piece_type: PieceType,
    },
    Spread {
        x: u8,
        y: u8,
        direction: Direction,
        drops: Drops,
    },
}

impl<const N: usize> Ply<N> {
    #[instrument(level = "trace")]
    pub fn validate(self) -> Result<(), PlyError> {
        match self {
            Ply::Place { x, y, .. } => {
                if x as usize >= N || y as usize >= N {
                    trace!("Out of bounds.");
                    return Err(PlyError::OutOfBounds);
                }
            }
            Ply::Spread {
                x,
                y,
                direction,
                drops,
            } => {
                if x as usize >= N || y as usize >= N {
                    trace!("Out of bounds.");
                    return Err(PlyError::OutOfBounds);
                }

                // The end of the spread must be in bounds.
                let (dx, dy) = direction.to_offset();
                let (tx, ty) = (
                    x as i8 + dx * drops.len() as i8,
                    y as i8 + dy * drops.len() as i8,
                );
                if tx < 0 || tx as usize >= N || ty < 0 || ty as usize >= N {
                    trace!("End of spread is out of bounds.");
                    return Err(PlyError::OutOfBounds);
                }
            }
        }

        Ok(())
    }
}

impl<const N: usize> fmt::Debug for Ply<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptn = PtnPly::from((*self, PlyValidation { is_crush: false }));
        write!(f, "{ptn}")
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Direction {
    North = 0,
    East,
    South,
    West,
}

impl Direction {
    pub fn to_offset(self) -> (i8, i8) {
        match self {
            Direction::North => (0, 1),
            Direction::East => (1, 0),
            Direction::South => (0, -1),
            Direction::West => (-1, 0),
        }
    }
}

impl TryFrom<u8> for Direction {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Direction::North),
            1 => Ok(Direction::East),
            2 => Ok(Direction::South),
            3 => Ok(Direction::West),
            _ => Err("invalid direction value"),
        }
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Drops(u8);

impl Drops {
    pub fn new<const N: usize>(value: u8) -> Result<Self, PlyError> {
        if value == 0 {
            return Err(PlyError::InvalidDrops("Must specify at least one drop."));
        } else if value as usize >= 1 << N {
            return Err(PlyError::InvalidDrops("Illegal carry amount."));
        }

        Ok(Self(value))
    }

    pub fn from_drop_counts<const N: usize>(drops: &[u8]) -> Result<Self, PlyError> {
        if drops.len() >= N {
            return Err(PlyError::InvalidDrops("Too many drops."));
        } else if drops.is_empty() {
            return Err(PlyError::InvalidDrops("Must specify at least one drop."));
        }

        if drops.iter().any(|d| *d == 0) {
            return Err(PlyError::InvalidDrops("Invalid drop amount."));
        }

        if drops.iter().sum::<u8>() as usize > N {
            return Err(PlyError::InvalidDrops("Illegal carry amount."));
        }

        let mut map = 0;
        for drop in drops.iter().rev() {
            map <<= 1;
            map |= 1;
            map <<= drop - 1;
        }

        Ok(Self(map))
    }

    pub fn id(&self) -> usize {
        self.0 as usize
    }

    pub fn iter(&self) -> impl Iterator<Item = u8> {
        struct DropIterator(u8);

        impl Iterator for DropIterator {
            type Item = u8;

            fn next(&mut self) -> Option<Self::Item> {
                if self.0 > 0 {
                    let drop = self.0.trailing_zeros() as u8 + 1;
                    if drop < 8 {
                        self.0 >>= drop;
                    } else {
                        self.0 = 0;
                    }
                    Some(drop)
                } else {
                    None
                }
            }
        }

        DropIterator(self.0)
    }

    pub fn last(&self) -> usize {
        let len = self.len();
        if len > 1 {
            (self.0 << (self.0.leading_zeros() + 1)).leading_zeros() as usize + 1
        } else {
            self.carry()
        }
    }

    pub fn len(&self) -> usize {
        self.0.count_ones() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn carry(&self) -> usize {
        (8 - self.0.leading_zeros()) as usize
    }
}

impl fmt::Debug for Drops {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let drops = self.iter().collect::<Vec<_>>();
        f.debug_tuple("Drops").field(&self.0).field(&drops).finish()
    }
}

impl From<Drops> for u8 {
    fn from(drops: Drops) -> Self {
        drops.0
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum PlyError {
    OutOfBounds,
    InvalidDrops(&'static str),
}

pub mod generation {
    use super::*;

    pub(crate) use self::spread_maps::spread_map;

    pub fn placements<const N: usize>(
        locations: Bitmap<N>,
        piece_type: PieceType,
    ) -> impl Iterator<Item = Ply<N>> {
        locations
            .bits()
            .map(|b| b.coordinates())
            .map(move |(x, y)| Ply::Place {
                x: x as u8,
                y: y as u8,
                piece_type,
            })
    }

    pub fn spreads<const N: usize>(
        state: &State<N>,
        locations: Bitmap<N>,
    ) -> impl Iterator<Item = Ply<N>> + '_ {
        use PieceType::*;

        locations
            .bits()
            .map(|b| b.coordinates())
            .flat_map(move |(x, y)| {
                let stack = &state.board[x][y];
                let top_piece = stack.top().unwrap();

                [
                    Direction::North,
                    Direction::East,
                    Direction::South,
                    Direction::West,
                ]
                .into_iter()
                .flat_map(move |direction| {
                    let (dx, dy) = direction.to_offset();
                    let (mut tx, mut ty) = (x as i8, y as i8);
                    let mut distance = 0;

                    let pickup_size = N.min(stack.len());

                    // Cast until the edge of the board or until (and including) a blocking piece.
                    for _ in 0..pickup_size {
                        tx += dx;
                        ty += dy;
                        if tx < 0 || tx >= N as i8 || ty < 0 || ty >= N as i8 {
                            break;
                        }

                        distance += 1;
                        let target_type = state.board[tx as usize][ty as usize].top_piece_type();

                        if matches!(target_type, Some(StandingStone | Capstone)) {
                            break;
                        }
                    }

                    (1..1 << pickup_size)
                        .map(|value| Drops::new::<N>(value as u8).expect("invalid drops"))
                        .filter(move |drops| drops.len() <= distance)
                        .filter(move |drops| {
                            let tx = x as i8 + drops.len() as i8 * dx;
                            let ty = y as i8 + drops.len() as i8 * dy;
                            let target_type =
                                state.board[tx as usize][ty as usize].top_piece_type();

                            // Allow this drop combo if the target is a flatstone or empty.
                            let unblocked = target_type.is_none() || target_type == Some(Flatstone);

                            // Allow this drop combo if the target is a standing stone, and we're
                            // dropping a capstone by itself onto it.
                            let crush = target_type == Some(StandingStone)
                                && top_piece.piece_type() == Capstone
                                && drops.last() == 1;

                            unblocked || crush
                        })
                        .map(move |drops| Ply::Spread {
                            x: x as u8,
                            y: y as u8,
                            direction,
                            drops,
                        })
                })
            })
    }

    mod spread_maps {
        use super::*;

        #[derive(Debug)]
        struct SpreadMaps<const N: usize> {
            stack_size: Vec<StackSize<N>>,
        }

        #[derive(Debug)]
        struct StackSize<const N: usize> {
            configuration: Vec<Configuration<N>>,
        }

        #[derive(Debug)]
        struct Configuration<const N: usize> {
            spread: Vec<Spread<N>>,
        }

        #[derive(Debug)]
        struct Spread<const N: usize> {
            direction: Vec<SpreadMap<N>>,
        }

        #[derive(Clone, Copy, Debug)]
        pub(crate) struct SpreadMap<const N: usize> {
            pub(crate) endpoint: Bitmap<N>,
            pub(crate) player: Bitmap<N>,
            pub(crate) opponent: Bitmap<N>,
        }

        impl<const N: usize> SpreadMaps<N> {
            fn new() -> Self {
                Self {
                    stack_size: StackSize::<N>::range().map(StackSize::new).collect(),
                }
            }
        }

        impl<const N: usize> StackSize<N> {
            fn new(stack_size: usize) -> Self {
                debug_assert!(StackSize::<N>::range().contains(&stack_size));

                Self {
                    configuration: Configuration::<N>::range(stack_size)
                        .map(|configuration| Configuration::new(stack_size, configuration))
                        .collect(),
                }
            }

            fn range() -> RangeInclusive<usize> {
                RangeInclusive::new(
                    // Smallest stack size to consider is 1.
                    1,
                    // Largest is the board size + 1 (a full carry + the piece that gets revealed underneath).
                    N + 1,
                )
            }
        }

        impl<const N: usize> Configuration<N> {
            fn new(stack_size: usize, configuration: usize) -> Self {
                debug_assert!(StackSize::<N>::range().contains(&stack_size));
                debug_assert!(Configuration::<N>::range(stack_size).contains(&configuration));

                Self {
                    spread: Spread::<N>::range(stack_size)
                        .map(|spread| Drops::new::<N>(spread as u8).expect("invalid drops"))
                        .map(|drops| Spread::new(stack_size, configuration, drops))
                        .collect(),
                }
            }

            fn range(stack_size: usize) -> RangeInclusive<usize> {
                // A range that walks every combination of player and opponent stones
                // in a stack of stack_size height.
                RangeInclusive::new(
                    // Begin with a single player stone on top of all opponent stones.
                    // i.e. stack_size = 3 => range.start = 0b100
                    1 << (stack_size - 1),
                    // Increment configurations until the stack is all player stones.
                    // i.e. stack_size = 3 => range.end = 0b111
                    (1 << stack_size) - 1,
                )
            }
        }

        impl<const N: usize> Spread<N> {
            fn new(stack_size: usize, configuration: usize, drops: Drops) -> Self {
                debug_assert!(StackSize::<N>::range().contains(&stack_size));
                debug_assert!(Configuration::<N>::range(stack_size).contains(&configuration));
                debug_assert!(Spread::<N>::range(stack_size).contains(&(u8::from(drops) as usize)));

                Self {
                    direction: [
                        Direction::North,
                        Direction::East,
                        Direction::South,
                        Direction::West,
                    ]
                    .into_iter()
                    .map(|direction| SpreadMap::new(stack_size, configuration, drops, direction))
                    .collect(),
                }
            }

            fn range(stack_size: usize) -> RangeInclusive<usize> {
                // A range that walks every drop pattern for a stack of stack_size - 1
                // height (for carries that leave one stone behind), followed by every drop
                // pattern for a stack of stack_size height (for carries that leave no stones behind).
                RangeInclusive::new(
                    // Begin with dumping all but one stone on the very next square.
                    // i.e. stack_size = 3 => range.start = 0b10
                    1 << (stack_size.max(2) - 2),
                    // Increment until we drop one stone on every square. If we'd run into the board
                    // edge, don't generate the very last one.
                    // i.e. stack_size = 3, N = 4 => range.end = 0b111
                    // i.e. stack_size = 4, N = 4 => range.end = 0b1110
                    (1 << stack_size).min((1 << N) - 1) - 1,
                )
            }
        }

        impl<const N: usize> SpreadMap<N> {
            fn new(
                stack_size: usize,
                configuration: usize,
                drops: Drops,
                direction: Direction,
            ) -> Self {
                debug_assert!(StackSize::<N>::range().contains(&stack_size));
                debug_assert!(Configuration::<N>::range(stack_size).contains(&configuration));
                debug_assert!(Spread::<N>::range(stack_size).contains(&(u8::from(drops) as usize)));

                let (x, y) = match direction {
                    Direction::North | Direction::East => (0, 0),
                    Direction::South | Direction::West => (N - 1, N - 1),
                };

                let mut player = Bitmap::empty();
                let mut opponent = Bitmap::empty();
                let mut endpoint = Bitmap::empty();

                fn mark_square<const N: usize>(
                    stack: Stack,
                    x: usize,
                    y: usize,
                    player: &mut Bitmap<N>,
                    opponent: &mut Bitmap<N>,
                    endpoint: &mut Bitmap<N>,
                ) {
                    if let Some(top) = stack.top() {
                        match top.color() {
                            Color::White => player.set(x, y),
                            Color::Black => opponent.set(x, y),
                        }

                        if top.piece_type() == PieceType::Capstone {
                            endpoint.set(x, y);
                        }
                    }
                }

                let player_bitmap =
                    ((configuration as u16).reverse_bits() >> (16 - stack_size)) as StackBitmap;

                let mut stack = Stack::from_player_bitmap(
                    stack_size,
                    player_bitmap,
                    Piece::new(PieceType::Capstone, Color::White),
                );

                let carry_total = drops.iter().sum::<u8>() as usize;
                let mut carry = stack.take(carry_total);

                mark_square(stack, x, y, &mut player, &mut opponent, &mut endpoint);

                let (dx, dy) = direction.to_offset();
                let (mut tx, mut ty) = (x as i8, y as i8);
                for drop in drops.iter() {
                    tx += dx;
                    ty += dy;

                    mark_square(
                        carry.drop(drop as usize),
                        tx as usize,
                        ty as usize,
                        &mut player,
                        &mut opponent,
                        &mut endpoint,
                    );
                }

                Self {
                    endpoint,
                    player,
                    opponent,
                }
            }
        }

        static SPREAD_MAPS_3S: Lazy<SpreadMaps<3>> = Lazy::new(SpreadMaps::<3>::new);
        static SPREAD_MAPS_4S: Lazy<SpreadMaps<4>> = Lazy::new(SpreadMaps::<4>::new);
        static SPREAD_MAPS_5S: Lazy<SpreadMaps<5>> = Lazy::new(SpreadMaps::<5>::new);
        static SPREAD_MAPS_6S: Lazy<SpreadMaps<6>> = Lazy::new(SpreadMaps::<6>::new);
        static SPREAD_MAPS_7S: Lazy<SpreadMaps<7>> = Lazy::new(SpreadMaps::<7>::new);
        static SPREAD_MAPS_8S: Lazy<SpreadMaps<8>> = Lazy::new(SpreadMaps::<8>::new);

        fn cast_ply<const N: usize, const M: usize>(ply: &Ply<N>) -> &Ply<M> {
            debug_assert_eq!(N, M);
            unsafe { mem::transmute(ply) }
        }

        fn cast_spread_map<const N: usize, const M: usize>(
            spread_map: SpreadMap<N>,
        ) -> SpreadMap<M> {
            debug_assert_eq!(N, M);
            unsafe { mem::transmute(spread_map) }
        }

        pub(crate) fn spread_map<const N: usize>(stack: &Stack, ply: &Ply<N>) -> SpreadMap<N> {
            let stack_size = match ply {
                Ply::Spread { drops, .. } => (drops.carry() + 1).min(stack.len()),
                _ => panic!("can't get spread map of a placement ply: {ply:?}"),
            };
            let player_bitmaps = stack.get_player_bitmaps();
            let player_bitmap = match stack
                .top()
                .expect("must be at least one piece in the stack")
                .color()
            {
                Color::White => player_bitmaps.0,
                Color::Black => player_bitmaps.1,
            };

            match N {
                3 => cast_spread_map(spread_map_sized(
                    stack_size,
                    player_bitmap,
                    cast_ply(ply),
                    &*SPREAD_MAPS_3S,
                )),
                4 => cast_spread_map(spread_map_sized(
                    stack_size,
                    player_bitmap,
                    cast_ply(ply),
                    &*SPREAD_MAPS_4S,
                )),
                5 => cast_spread_map(spread_map_sized(
                    stack_size,
                    player_bitmap,
                    cast_ply(ply),
                    &*SPREAD_MAPS_5S,
                )),
                6 => cast_spread_map(spread_map_sized(
                    stack_size,
                    player_bitmap,
                    cast_ply(ply),
                    &*SPREAD_MAPS_6S,
                )),
                7 => cast_spread_map(spread_map_sized(
                    stack_size,
                    player_bitmap,
                    cast_ply(ply),
                    &*SPREAD_MAPS_7S,
                )),
                8 => cast_spread_map(spread_map_sized(
                    stack_size,
                    player_bitmap,
                    cast_ply(ply),
                    &*SPREAD_MAPS_8S,
                )),
                _ => unreachable!(),
            }
        }

        fn spread_map_sized<const N: usize>(
            stack_size: usize,
            player_bitmap: StackBitmap,
            ply: &Ply<N>,
            spread_maps: &SpreadMaps<N>,
        ) -> SpreadMap<N> {
            let (x, y, direction, drops) = match *ply {
                Ply::Spread {
                    x,
                    y,
                    direction,
                    drops,
                    ..
                } => (x, y, direction, drops),
                _ => unreachable!("the ply should never be a placement here"),
            };

            let stack_size_index = stack_size - StackSize::<N>::range().start();
            let configuration_index = ((player_bitmap as u16) << (16 - stack_size)).reverse_bits()
                as usize
                - Configuration::<N>::range(stack_size).start();
            let spread_index = u8::from(drops) as usize - Spread::<N>::range(stack_size).start();

            let stack_size = &spread_maps.stack_size[stack_size_index];
            let configuration = &stack_size.configuration[configuration_index];
            let spread = &configuration.spread[spread_index];
            let mut map = spread.direction[direction as usize];

            match direction {
                Direction::North | Direction::East => {
                    let x_shift = x as usize;
                    let y_shift = y as usize * N;
                    map.endpoint = (map.endpoint >> x_shift) << y_shift;
                    map.player = (map.player >> x_shift) << y_shift;
                    map.opponent = (map.opponent >> x_shift) << y_shift;
                }
                Direction::South | Direction::West => {
                    let x_shift = N - 1 - x as usize;
                    let y_shift = (N - 1 - y as usize) * N;
                    map.endpoint = (map.endpoint << x_shift) >> y_shift;
                    map.player = (map.player << x_shift) >> y_shift;
                    map.opponent = (map.opponent << x_shift) >> y_shift;
                }
            }

            map
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn print_spread_maps_size() {
                macro_rules! spread_count {
                    ($maps:ident) => {{
                        $maps
                            .stack_size
                            .iter()
                            .flat_map(|stack_size| &stack_size.configuration)
                            .flat_map(|configuration| &configuration.spread)
                            .count()
                    }};
                }

                let count = spread_count!(SPREAD_MAPS_3S);
                let maps = count * 4;
                let bytes = maps * std::mem::size_of::<SpreadMap<3>>();
                println!("SPREAD_MAPS_3S: {count} spreads, {maps} maps, {bytes} bytes");

                let count = spread_count!(SPREAD_MAPS_4S);
                let maps = count * 4;
                let bytes = maps * std::mem::size_of::<SpreadMap<4>>();
                println!("SPREAD_MAPS_4S: {count} spreads, {maps} maps, {bytes} bytes");

                let count = spread_count!(SPREAD_MAPS_5S);
                let maps = count * 4;
                let bytes = maps * std::mem::size_of::<SpreadMap<5>>();
                println!("SPREAD_MAPS_5S: {count} spreads, {maps} maps, {bytes} bytes");

                let count = spread_count!(SPREAD_MAPS_6S);
                let maps = count * 4;
                let bytes = maps * std::mem::size_of::<SpreadMap<6>>();
                println!("SPREAD_MAPS_6S: {count} spreads, {maps} maps, {bytes} bytes");

                let count = spread_count!(SPREAD_MAPS_7S);
                let maps = count * 4;
                let bytes = maps * std::mem::size_of::<SpreadMap<7>>();
                println!("SPREAD_MAPS_7S: {count} spreads, {maps} maps, {bytes} bytes");

                let count = spread_count!(SPREAD_MAPS_8S);
                let maps = count * 4;
                let bytes = maps * std::mem::size_of::<SpreadMap<8>>();
                println!("SPREAD_MAPS_8S: {count} spreads, {maps} maps, {bytes} bytes");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use generation::spread_map;

    #[test]
    fn drops() {
        let drops = Drops::from_drop_counts::<6>(&[3, 2, 1]).unwrap();
        let mut d = drops.iter();

        assert_eq!(d.next(), Some(3));
        assert_eq!(d.next(), Some(2));
        assert_eq!(d.next(), Some(1));
        assert_eq!(d.next(), None);
    }

    #[test]
    fn drops_invalid_carry() {
        assert!(Drops::from_drop_counts::<6>(&[3, 3, 1]).is_err());
    }

    #[test]
    fn drops_invalid_drop() {
        assert!(Drops::from_drop_counts::<6>(&[3, 2, 0, 1]).is_err());
        assert!(Drops::from_drop_counts::<6>(&[3, 2, 1, 0]).is_err());
        assert!(Drops::from_drop_counts::<6>(&[0, 3, 2, 1]).is_err());
    }

    #[test]
    fn drops_last() {
        assert_eq!(Drops::from_drop_counts::<6>(&[3, 2, 1]).unwrap().last(), 1);
        assert_eq!(Drops::from_drop_counts::<6>(&[1, 2, 3]).unwrap().last(), 3);
        assert_eq!(Drops::from_drop_counts::<6>(&[3]).unwrap().last(), 3);
        assert_eq!(Drops::from_drop_counts::<6>(&[1]).unwrap().last(), 1);
    }

    #[test]
    fn spread_maps() {
        let stack =
            Stack::from_player_bitmap(3, 0b101, Piece::new(PieceType::Flatstone, Color::Black));
        let ply = Ply::<5>::Spread {
            x: 1,
            y: 1,
            direction: Direction::East,
            drops: Drops::new::<5>(0b111).unwrap(),
        };
        let map = spread_map(&stack, &ply);

        assert_eq!(map.endpoint, 0b00000_00000_00000_00001_00000.into());
        assert_eq!(map.player, 0b00000_00000_00000_00101_00000.into());
        assert_eq!(map.opponent, 0b00000_00000_00000_00010_00000.into());

        let stack =
            Stack::from_player_bitmap(5, 0b01101, Piece::new(PieceType::Flatstone, Color::White));
        let ply = Ply::<6>::Spread {
            x: 3,
            y: 4,
            direction: Direction::South,
            drops: Drops::new::<6>(0b111).unwrap(),
        };
        let map = spread_map(&stack, &ply);

        assert_eq!(
            map.endpoint,
            0b000000_000000_000000_000000_000100_000000.into(),
        );
        assert_eq!(
            map.player,
            0b000000_000100_000100_000000_000100_000000.into(),
        );
        assert_eq!(
            map.opponent,
            0b000000_000000_000000_000100_000000_000000.into(),
        );
    }
}
