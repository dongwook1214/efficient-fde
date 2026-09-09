# Baselines

Vendored copies of the reference implementations compared against, so that the
evaluation reproduces from a single clone.  Nothing here is our work; the licences
and attributions of the originals apply.

| Directory          | Upstream | Used for |
| ------------------ | -------- | -------- |
| `fde/`             | [PopcornPaws/fde](https://github.com/PopcornPaws/fde) — Tas, Nikolaenko, Seres, Melczer, Zhang, Kelkar, Bonneau, *Atomic BlockChain Data Exchange with Fairness* ([ePrint 2024/418](https://eprint.iacr.org/2024/418)) | exponential ElGamal, the shard range proofs, DLEQ proofs, and the KZG helpers used by VECK\_EL and VECK+\_EL |
| `veck-star-snark/` | the Go/gnark circuit shipped with VECK\*\_EL (arXiv:2506.14944) | the Groth16 circuit of VECK\*\_EL on BW6-761 with native BLS12-377 G1 arithmetic |

## Changes to `fde/`

1. **Random sampling and general division.**  Upstream selects the "random" subset
   as an FFT *subdomain*, which admits `divide_by_vanishing_poly` in O(n).  A
   genuinely random subset has an arbitrary vanishing polynomial, so
   `src/veck/kzg/elgamal/divide.rs` (Newton-inversion Euclidean division with a
   blocked path for low-degree divisors) was added and is used instead.
2. **Trimmed.**  `benches/` (Criterion harnesses requiring a checked-in
   `powers.bin`) and `contracts/` (the Solidity settlement layer) were removed,
   together with the corresponding `[[bench]]` entries and the `criterion`
   dev-dependency.  The library sources are otherwise unmodified.
3. `src/commit/powers_cache.rs` was added so the powers of tau can be generated
   once and shared with the other crates.

`fde/src/veck/kzg/elgamal/divide.rs` is not on the measured path: the harness
routes every scheme's `(phi - f_S) / Z_S` through `EFDE-KZG`'s
`subset_quotient_with_vanishing_poly`, so all four schemes share the same division
code and dispatch threshold.  The file is kept so the vendored crate still builds.

The orchestration of the baseline protocols lives in `benchmarks/kzg`.  It drives
the primitives above directly rather than calling upstream's `Proof::new_v2`,
whose prover and verifier assume the FFT-subdomain sampling of point 1 and no
longer verify under random sampling.  Every scheme in the harness runs its
verifier and asserts that it accepts.

## Changes to `veck-star-snark/`

Benchmark plumbing only: `const N` moved into build-tag-selected `params_r*.go`
files, `bench.go` added for CSV output, and `main` honours `runtime.NumCPU()`
instead of a hard-coded `GOMAXPROCS(32)`.  The circuit is untouched.
