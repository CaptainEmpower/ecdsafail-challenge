//! Shared `#[cfg(test)]` helpers for the modular submodule tests: 256-bit
//! secp256k1-prime value generation and masked-multi-shot register load/read.
use super::*;
use crate::point_add::SECP256K1_P;
use crate::sim::Simulator;
use sha3::digest::XofReader;

fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Deterministic value in `[0, p)`.
pub(super) fn rand_lt_p(seed: u64) -> U256 {
    let v = U256::from_limbs([
        splitmix(seed),
        splitmix(seed ^ 0xA),
        splitmix(seed ^ 0x14),
        splitmix(seed ^ 0x1E),
    ]);
    v % SECP256K1_P
}

/// Load `v` into `qs` on shot lane `s` (low bit = qs[0]).
pub(super) fn set256<R: XofReader>(sim: &mut Simulator<'_, R>, qs: &[QubitId], v: U256, s: usize) {
    for (i, &q) in qs.iter().enumerate() {
        if v.bit(i) {
            *sim.qubit_mut(q) |= 1u64 << s;
        }
    }
}

/// Read the value held by `qs` on shot lane `s`.
pub(super) fn get256<R: XofReader>(sim: &Simulator<'_, R>, qs: &[QubitId], s: usize) -> U256 {
    let mut v = U256::ZERO;
    for (i, &q) in qs.iter().enumerate() {
        if (sim.qubit(q) >> s) & 1 == 1 {
            v |= U256::from(1u64) << i;
        }
    }
    v
}
