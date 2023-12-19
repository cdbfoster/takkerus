use std::mem;
use std::slice;

use once_cell::sync::Lazy;
use rand::{self, Error, Fill, Rng};

use crate::piece::{Color::*, Piece};
use crate::stack::Stack;
use crate::state::State;

pub type ZobristHash = u64;

struct ZobristKeys<const N: usize> {
    black_to_move: ZobristHash,
    /// 6 possible top pieces.
    top_pieces: [[[ZobristHash; 6]; N]; N],
    /// All possible arrangements of pieces in the top 8 pieces of a stack.
    stack_pieces: [[[ZobristHash; 256]; N]; N],
    /// All possible stack heights in 8s.
    stack_heights: [[[ZobristHash; 101]; N]; N],
}

impl<const N: usize> ZobristKeys<N> {
    fn new() -> Self {
        #[cfg(not(feature = "fixed-rng"))]
        let mut rng = rand::thread_rng();

        #[cfg(feature = "fixed-rng")]
        let mut rng = {
            use rand::rngs::StdRng;
            use rand::SeedableRng;
            use std::env;
            use std::sync::Mutex;
            use tracing::info;

            static RNG: Lazy<Mutex<StdRng>> = Lazy::new(|| {
                let seed = if let Ok(seed) = env::var("FIXED_RNG_SEED") {
                    seed.parse::<u64>().expect("could not parse random seed")
                } else {
                    u64::from_be_bytes([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08])
                };

                info!(?seed, "Initializing Zobrist hashing rng.");

                Mutex::new(StdRng::seed_from_u64(seed))
            });

            RNG.lock().unwrap()
        };

        let mut keys = Self::default();
        rng.fill(&mut keys);

        keys
    }
}

impl<const N: usize> Default for ZobristKeys<N> {
    fn default() -> Self {
        Self {
            black_to_move: 0,
            top_pieces: [[[0; 6]; N]; N],
            stack_pieces: [[[0; 256]; N]; N],
            stack_heights: [[[0; 101]; N]; N],
        }
    }
}

impl<const N: usize> Fill for ZobristKeys<N> {
    fn try_fill<R: Rng + ?Sized>(&mut self, rng: &mut R) -> Result<(), Error> {
        let ptr = self as *mut ZobristKeys<N> as *mut ZobristHash;
        let count = mem::size_of::<ZobristKeys<N>>() / mem::size_of::<ZobristHash>();

        let hashes = unsafe { slice::from_raw_parts_mut(ptr, count) };
        hashes.try_fill(rng)
    }
}

static ZOBRIST_KEYS_3S: Lazy<ZobristKeys<3>> = Lazy::new(ZobristKeys::<3>::new);
static ZOBRIST_KEYS_4S: Lazy<ZobristKeys<4>> = Lazy::new(ZobristKeys::<4>::new);
static ZOBRIST_KEYS_5S: Lazy<ZobristKeys<5>> = Lazy::new(ZobristKeys::<5>::new);
static ZOBRIST_KEYS_6S: Lazy<ZobristKeys<6>> = Lazy::new(ZobristKeys::<6>::new);
static ZOBRIST_KEYS_7S: Lazy<ZobristKeys<7>> = Lazy::new(ZobristKeys::<7>::new);
static ZOBRIST_KEYS_8S: Lazy<ZobristKeys<8>> = Lazy::new(ZobristKeys::<8>::new);

pub fn zobrist_advance_move<const N: usize>() -> ZobristHash {
    match N {
        3 => ZOBRIST_KEYS_3S.black_to_move,
        4 => ZOBRIST_KEYS_4S.black_to_move,
        5 => ZOBRIST_KEYS_5S.black_to_move,
        6 => ZOBRIST_KEYS_6S.black_to_move,
        7 => ZOBRIST_KEYS_7S.black_to_move,
        8 => ZOBRIST_KEYS_8S.black_to_move,
        _ => unreachable!(),
    }
}

pub fn zobrist_hash_state<const N: usize>(state: &State<N>) -> ZobristHash {
    match N {
        3 => zobrist_hash_state_sized(cast_size(state), &*ZOBRIST_KEYS_3S),
        4 => zobrist_hash_state_sized(cast_size(state), &*ZOBRIST_KEYS_4S),
        5 => zobrist_hash_state_sized(cast_size(state), &*ZOBRIST_KEYS_5S),
        6 => zobrist_hash_state_sized(cast_size(state), &*ZOBRIST_KEYS_6S),
        7 => zobrist_hash_state_sized(cast_size(state), &*ZOBRIST_KEYS_7S),
        8 => zobrist_hash_state_sized(cast_size(state), &*ZOBRIST_KEYS_8S),
        _ => unreachable!(),
    }
}

pub fn zobrist_hash_stack<const N: usize>(stack: Stack, x: usize, y: usize) -> ZobristHash {
    match N {
        3 => zobrist_hash_stack_sized(stack, x, y, &*ZOBRIST_KEYS_3S),
        4 => zobrist_hash_stack_sized(stack, x, y, &*ZOBRIST_KEYS_4S),
        5 => zobrist_hash_stack_sized(stack, x, y, &*ZOBRIST_KEYS_5S),
        6 => zobrist_hash_stack_sized(stack, x, y, &*ZOBRIST_KEYS_6S),
        7 => zobrist_hash_stack_sized(stack, x, y, &*ZOBRIST_KEYS_7S),
        8 => zobrist_hash_stack_sized(stack, x, y, &*ZOBRIST_KEYS_8S),
        _ => unreachable!(),
    }
}

fn zobrist_hash_state_sized<const N: usize>(
    state: &State<N>,
    keys: &ZobristKeys<N>,
) -> ZobristHash {
    let mut hash = ZobristHash::default();

    if state.to_move() == Black {
        hash ^= keys.black_to_move;
    }

    for x in 0..N {
        for y in 0..N {
            hash ^= zobrist_hash_stack_sized(state.board[x][y], x, y, keys);
        }
    }

    hash
}

fn zobrist_hash_stack_sized<const N: usize>(
    stack: Stack,
    x: usize,
    y: usize,
    keys: &ZobristKeys<N>,
) -> ZobristHash {
    let mut hash = 0;

    if let Some(top_piece) = stack.top() {
        let (_, stack_map) = stack.get_player_bitmaps();
        hash ^= keys.top_pieces[x][y][piece_index(top_piece)];
        hash ^= keys.stack_heights[x][y][stack.len()];
        hash ^= keys.stack_pieces[x][y][(stack_map & 0xFF) as usize];
    }

    hash
}

fn cast_size<const N: usize, const M: usize>(state: &State<N>) -> &State<M> {
    debug_assert_eq!(N, M);
    unsafe { mem::transmute(state) }
}

/// 0 - White flatstone
/// 1 - Black flatstone
/// 2 - White standing stone
/// 3 - Black standing stone
/// 4 - White capstone
/// 5 - Black capstone
fn piece_index(piece: Piece) -> usize {
    ((piece.piece_type() as usize >> 5) << 1) + piece.color() as usize - 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piece::PieceType::*;

    #[test]
    fn correct_piece_indices() {
        assert_eq!(piece_index(Piece::new(Flatstone, White)), 0);
        assert_eq!(piece_index(Piece::new(Flatstone, Black)), 1);
        assert_eq!(piece_index(Piece::new(StandingStone, White)), 2);
        assert_eq!(piece_index(Piece::new(StandingStone, Black)), 3);
        assert_eq!(piece_index(Piece::new(Capstone, White)), 4);
        assert_eq!(piece_index(Piece::new(Capstone, Black)), 5);
    }
}
