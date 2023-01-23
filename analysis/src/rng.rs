use std::sync::{Mutex, MutexGuard};

use once_cell::sync::Lazy;
use rand::{self, Rng};
use rand_core::{impls, RngCore};

#[derive(Clone, Copy)]
pub struct JKiss32Rng {
    x: u32,
    y: u32,
    z: u32,
    w: u32,
    c: bool,
}

impl JKiss32Rng {
    pub fn new() -> JKiss32Rng {
        #[cfg(not(feature = "fixed-rng"))]
        let mut rng = rand::thread_rng();

        #[cfg(feature = "fixed-rng")]
        let mut rng = {
            use rand::rngs::StdRng;
            use rand::SeedableRng;
            use std::env;
            use tracing::info;

            let seed = if let Ok(seed) = env::var("FIXED_RNG_SEED") {
                seed.parse::<u64>().expect("could not parse random seed")
            } else {
                u64::from_be_bytes([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08])
            };

            info!(?seed, "Initializing search rng.");

            StdRng::seed_from_u64(seed)
        };

        JKiss32Rng {
            x: rng.gen(),
            y: rng.gen(),
            z: rng.gen(),
            w: rng.gen(),
            c: false,
        }
    }
}

impl Default for JKiss32Rng {
    fn default() -> Self {
        Self::new()
    }
}

impl RngCore for JKiss32Rng {
    fn next_u32(&mut self) -> u32 {
        self.y ^= self.y << 5;
        self.y ^= self.y >> 7;
        self.y ^= self.y << 22;
        let t = self.z.wrapping_add(self.w).wrapping_add(self.c as u32) as i32;
        self.z = self.w;
        self.c = t < 0;
        self.w = (t & 0x7FFFFFFF) as u32;
        self.x = self.x.wrapping_add(1411392427);
        self.x.wrapping_add(self.y).wrapping_add(self.w)
    }

    fn next_u64(&mut self) -> u64 {
        impls::next_u64_via_u32(self)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        impls::fill_bytes_via_next(self, dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

pub(crate) fn get_rng() -> MutexGuard<'static, JKiss32Rng> {
    static RNG: Lazy<Mutex<JKiss32Rng>> = Lazy::new(Default::default);
    RNG.lock().unwrap()
}
