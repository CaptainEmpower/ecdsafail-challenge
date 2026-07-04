"""Independent reference arithmetic for stating claims about replayed op-streams.

When a proof compares a replayed output against an expected value, the *reference*
must be computed a different way than the implementation, or the proof is circular
(see `DESIGN.md` §6). These are plain little-endian bit-list (`z3.Bool`) primitives —
ripple-carry add, borrow subtract, unsigned compare, and the modular `x±y`/`2x mod p`
reductions built from them (textbook reduce-by-conditional-(sub/add), structurally
unlike the emitted Solinas `+c`/overflow path they check).

Bit lists are little-endian: index 0 is the least-significant bit.
"""
from z3 import And, BoolRef, BoolVal, If, Not, Or, Xor


def const_bits(value: int, n: int) -> list:
    """Little-endian list of `n` z3 Bool constants for the integer `value`."""
    return [BoolVal((value >> i) & 1 == 1) for i in range(n)]


def add_bits(x: list, y: list):
    """Ripple-carry add of two equal-length little-endian bit lists.

    Returns `(sum_bits, carry_out)` — `sum_bits` has the input length, `carry_out`
    is the (n+1)-th bit."""
    if len(x) != len(y):
        raise ValueError("add_bits: operand widths differ")
    carry = BoolVal(False)
    out = []
    for xi, yi in zip(x, y):
        out.append(Xor(Xor(xi, yi), carry))
        carry = Or(And(xi, yi), And(xi, carry), And(yi, carry))
    return out, carry


def sub_bits(x: list, y: list):
    """Ripple-borrow subtract `x - y` (little-endian, equal length).

    Returns `(diff_bits, borrow_out)`; `borrow_out` is true iff `x < y` (unsigned)."""
    if len(x) != len(y):
        raise ValueError("sub_bits: operand widths differ")
    borrow = BoolVal(False)
    out = []
    for xi, yi in zip(x, y):
        out.append(Xor(Xor(xi, yi), borrow))
        borrow = Or(And(Not(xi), yi), And(Not(xi), borrow), And(yi, borrow))
    return out, borrow


def ult(x: list, y: list) -> BoolRef:
    """True iff `x < y` (unsigned, little-endian bit lists)."""
    _, borrow = sub_bits(x, y)
    return borrow


def bits_eq(x: list, y: list) -> BoolRef:
    """True iff the two equal-length bit lists are equal."""
    if len(x) != len(y):
        raise ValueError("bits_eq: operand widths differ")
    return And(*[xi == yi for xi, yi in zip(x, y)])


def mod_add(x: list, y: list, p: int) -> list:
    """`(x + y) mod p` for `x, y ∈ [0, p)`, as an n-bit little-endian list.

    Ripple-carry to an (n+1)-bit sum `s ∈ [0, 2p)`, then subtract `p` once iff
    `s >= p`. Fully reduced into `[0, p)`."""
    n = len(x)
    sum_bits, carry = add_bits(x, y)
    s = sum_bits + [carry]  # (n+1)-bit sum
    p_ext = const_bits(p, n + 1)
    s_lt_p = ult(s, p_ext)
    s_minus_p, _ = sub_bits(s, p_ext)
    return [If(s_lt_p, s[i], s_minus_p[i]) for i in range(n)]


def mod_sub(x: list, y: list, p: int) -> list:
    """`(x - y) mod p` for `x, y ∈ [0, p)`, as an n-bit little-endian list.

    Borrow-subtract; if it underflows (`x < y`), add `p` back. Fully reduced."""
    n = len(x)
    diff, borrow = sub_bits(x, y)
    corrected, _ = add_bits(diff, const_bits(p, n))
    return [If(borrow, corrected[i], diff[i]) for i in range(n)]


def mod_double_canonical(x: list, p: int) -> list:
    """Canonical `(2·x) mod p ∈ [0, p)` for `x ∈ [0, p)` (i.e. `mod_add(x, x, p)`)."""
    return mod_add(x, x, p)


def mod_reduce_once(x: list, p: int) -> list:
    """Canonical rep of `x ∈ [0, 2^n)` under a single conditional subtract of `p`.

    Returns `x` if `x < p` else `x − p`. Correct (lands in `[0, p)`) when `x < 2p`,
    which holds for any `x < 2^n` given `p > 2^{n-1}` (as for secp256k1). Lets a claim
    compare a possibly-unreduced output against a canonical reference with a single
    equality instead of an `x ∈ {r, r+p}` disjunction."""
    n = len(x)
    p_bits = const_bits(p, n)
    x_lt_p = ult(x, p_bits)
    x_minus_p, _ = sub_bits(x, p_bits)
    return [If(x_lt_p, x[i], x_minus_p[i]) for i in range(n)]


__all__ = [
    "const_bits",
    "add_bits",
    "sub_bits",
    "ult",
    "bits_eq",
    "mod_add",
    "mod_sub",
    "mod_double_canonical",
    "mod_reduce_once",
]
