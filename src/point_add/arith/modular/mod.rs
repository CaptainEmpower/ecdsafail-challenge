//! Modular arithmetic: add/sub/neg (qq and qb), doubling/halving, shifts, and
//! the controlled (cmod_*) variants. All operate over secp256k1's prime via
//! Solinas reduction on "extended" (n+1)-wide registers; bit n is a transient
//! overflow/sign ancilla allocated for the duration of a mod-op.
use super::*;

mod add;
mod scale;
mod shift;
mod sub;
#[cfg(test)]
mod test_util;

pub(crate) use add::*;
pub(crate) use scale::*;
pub(crate) use shift::*;
pub(crate) use sub::*;
