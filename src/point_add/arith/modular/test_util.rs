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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::point_add::SECP256K1_P;

    /// The shared helpers are self-consistent: `rand_lt_p` stays in `[0, p)` and
    /// `set256`/`get256` round-trip a value bit-exactly on a lane.
    #[test]
    fn helpers_are_self_consistent() {
        for seed in 0..32u64 {
            assert!(rand_lt_p(seed) < SECP256K1_P, "rand_lt_p returned >= p");
        }
        let mut b = B::new();
        let reg = b.alloc_qubits(256);
        let nq = b.next_qubit as usize;
        let nb = b.next_bit as usize;
        let mut seed = sha3::Shake128::default();
        sha3::digest::Update::update(&mut seed, b"test_util-roundtrip");
        let mut xof = sha3::digest::ExtendableOutput::finalize_xof(seed);
        let mut sim = Simulator::new(nq, nb, &mut xof);
        let vals: Vec<U256> = (0..64).map(|s| rand_lt_p(s as u64 + 1)).collect();
        for (s, &v) in vals.iter().enumerate() {
            set256(&mut sim, &reg, v, s);
        }
        for (s, &v) in vals.iter().enumerate() {
            assert_eq!(
                get256(&sim, &reg, s),
                v,
                "set256/get256 round-trip failed, lane {s}"
            );
        }
    }
}
