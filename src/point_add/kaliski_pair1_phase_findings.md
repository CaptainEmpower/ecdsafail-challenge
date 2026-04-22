# Pair1 phase findings

Using a main-like replay on the actual first strict failing batch for `k = 4`
(batch index 10), the phase mask inside pair1 behaves as follows:

- after inverse wrapper entry / body start (`after_inv`):
  `0x0000040000000000`
- after `pair1_mul1`:
  `0x0000040000000000`
- after the long `pair1_halve` chain:
  `0x0000000000000000`
- after `pair1_mul2`:
  `0x0000040000000000`

## Interpretation
This is the first really useful fine-grained phase fact:
- the phase mask is already present entering the pair1 body,
- the `pair1_halve` chain cancels it,
- and `pair1_mul2` reintroduces it.

So the remaining bug hunt should focus on the phase relation between:
- the inverse wrapper state handed to the body,
- the `pair1_halve` correction path,
- and `pair1_mul2 = mod_mul_add_into_acc_schoolbook(b, &ty, &lam, &tx, p)`.

In particular, `pair1_mul2` is now the strongest concrete suspect for where the
specialized prefix stops being phase-compatible with the generic scaffold on the
strict failing batch.
