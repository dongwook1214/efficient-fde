# End-to-end benchmark

Everything the *sender* has to do to sell a file of `ell` field elements, for the
four schemes, on both curves, for `ell` from `2^10` to `2^20`.

## What is measured

Every scheme is expressed as the same sequence of stages, so the CSV columns line
up across schemes:

| stage | meaning |
| ----- | ------- |
| `encode` | Reed–Solomon expansion `ell -> m = ceil(beta * ell)`.  The file lives on the subgroup `D_ell`, the codeword on `D_m'` with `m' = 2^ceil(log2 m)`; both halves are FFTs (`Evaluations::interpolate` and `evaluate_over_domain`).  The full `m'`-point codeword is systematic (`D_ell <= D_m'`), but only its first `m` points are transmitted, and those do not contain every file symbol, so the transmitted code is not systematic.  For base VECK, which does not code, only the interpolation runs. |
| `commit` | the KZG commitment `C_phi` to the degree-`(ell-1)` message polynomial. |
| `encrypt` | whatever the sender applies to **every** transmitted symbol: exponential ElGamal for VECK and VECK+, a Poseidon PRF mask for VECK\* and for us. |
| `sample` | Fiat–Shamir derivation of the `R` checked positions, including the one pass that hashes the `m` transmitted symbols into the seed. |
| `subset` | interpolation of the sampled polynomial `f_S` and its commitment. |
| `sample_crypto` | per-sample public-key work: range proofs for VECK+, the in-circuit ElGamal ciphertexts for VECK\*, nothing for us.  For base VECK this is the range proofs over the *whole* file. |
| `kzg_proof` | quotient, its commitment, the opening at `alpha`, and (VECK, VECK+) the DLEQ proof. |
| `verify` | the buyer's checks — run and asserted. |

The Groth16 part of VECK\* and of our scheme is measured separately by the Go
drivers and joined in by `scripts/aggregate.py`, because it depends only on `R`,
never on the file size.

## Which scheme runs on which curve

| scheme | BLS12-381 | BLS12-377 | BW6-761 |
| ------ | --------- | --------- | ------- |
| VECK\_EL | yes | — | — |
| VECK+\_EL | yes | — | — |
| VECK\*\_EL | — | yes | — |
| ours | yes | — | yes |

Undefined combinations are refused.

VECK\*\_EL operates on its sampled ElGamal ciphertexts inside the circuit, so
they have to share the group its commitment lives in, and that group has to be
the inner curve of the two-chain the proof runs on.  Its KZG layer is therefore
measured on **BLS12-377** while its circuit is measured on **BW6-761** — one
scheme, two curves, one per layer.  `ours` needs no two-chain; its BW6-761 row
exists so that the two schemes can be compared on the same SNARK curve.

## Redundancy

The sweep is parameterised by the codeword expansion `beta`, not by the sample
count.  `beta` comes from `compute_beta(R, lambda + grinding)` with `lambda = 128`
and `grinding = 32`, i.e. the grinding-aware condition
`q_S ((beta+1)/2beta)^R <= 2^-128` with `q_S = 2^32`; inverting it gives the
smallest `R` reaching each target:

| `beta` | 1.1 | 1.25 | 1.5 | 2 |
| --- | --- | --- | --- | --- |
| `R` | 2384 | 1053 | 609 | 386 |
| measured for | all | all | all | all |

Every scheme is measured at every `beta`.

VECK\* at `R = 2384` needs 9,573,767 constraints, against 477,208 for ours.
Groth16 setup there takes tens of minutes and a multi-gigabyte proving key;
`-compile-only` reports the constraint count without it.

`--subsets` overrides the list and `--grinding 0` drops the grinding margin.
Because `f_S` has degree `R + 1` once blinded, a row is skipped unless
`R + 2 < ell`: `R = 2384` starts at `ell = 2^12` and `R = 1053` at `ell = 2^11`.

## Running it

```bash
./benchmarks/scripts/run_all.sh              # everything, then aggregate + plot
SKIP_SNARK=1 ./benchmarks/scripts/run_all.sh # Rust side only
MAX_LOG=14 ./benchmarks/scripts/run_all.sh   # a quick pass
```

Single runs:

```bash
cd benchmarks/kzg
cargo run --release -- --scheme ours --curve bls12-381 --min-log 10 --max-log 20 \
    --subsets 2384,1053,609,386 --out ../results/kzg_ours_bls12-381.csv

cd EFDE-SNARK/bls12-381 && go run -tags r609 . -csv ../../benchmarks/results/snark.csv
```

The Go drivers take `-cores` (default `runtime.NumCPU()`), recorded in
`snark.csv`; pass `-cores 32` to match the reference VECK\* driver's hard-coded
`GOMAXPROCS(32)`.

`--help` lists every option.  The powers of tau are generated once into
`benchmarks/kzg/.cache/srs/<curve>/` and reused.

## Extrapolation

Two stages touch every transmitted symbol with public-key operations:

* base VECK range-proves all `8 * ell` 32-bit shards of the file, and
* VECK+ ElGamal-encrypts the whole `m`-symbol codeword.

At `ell = 2^20` those cost days and hours respectively.  Both are exactly linear
in the number of symbols and embarrassingly parallel, so by default the harness
measures them once on a bounded prefix (`--max-measured-log`, default 14 for VECK
and 16 for VECK+) and scales the per-symbol cost.  Rows produced this way carry
`extrapolated=true` and are drawn dashed in the figure.  `--no-extrapolate`
measures everything, at the cost of a multi-day run.

The linearity assumption is checkable:

```bash
for cap in 10 11 12; do
  ./target/release/efde-bench --scheme veck-plus --curve bls12-381 \
      --min-log 14 --max-log 14 --subsets 609 --max-measured-log $cap \
      --no-verify --out /tmp/lin_$cap.csv
done   # compare encrypt_ms/m across the three
```

On the reference run the per-symbol cost moved by 4.5% across a 4x change in the
prefix, and by 0.6% between the two largest prefixes.

Nothing else is extrapolated: encoding, commitment, the masking of the codeword,
sampling, the subset polynomial, the quotient and every opening are measured at
the full file size for every row.  VECK+ rows still verify when extrapolated,
because only the whole-codeword encryption *timing* is scaled — the sampled
ciphertexts, range proofs and DLEQ are real.  VECK rows do not: there the buyer
receives the whole file, so with nothing materialised there is nothing to check.

## Outputs

```
benchmarks/results/
  kzg_<scheme>_<curve>.csv     per-stage timings from the Rust driver
  snark.csv                    Groth16 + CP-Link timings from the Go drivers
  end_to_end.csv               the join, with total sender and verifier times
  pgfplots/proving_time.tex    \addplot blocks
  pgfplots/snark_table.tex     rows for the SNARK resource table
  figures/end_to_end.{pdf,png} preview figure
```

## Repetition

Each stage runs until it has `--repeat` samples (default 5) *or* has spent
`--repeat-budget-ms` (default 2000), whichever comes first, and always at least
once.  The reported figure is the **median**.  In practice the millisecond-scale
stages get all five samples and the whole-codeword encryption gets one; the
`encrypt` row of VECK and VECK+ is measured exactly once by construction, since it
is the cached linear reference the larger sizes scale from.

`spread_pct` in the CSV reports how much of `prove_total_ms` is measurement
spread: the absolute spreads of the repeated stages, summed, over the total.
`aggregate.py` flags any row above 5%.  A *relative* per-stage spread would be
uninformative here — deriving `R` sample indices takes 20 microseconds and varies
by 200%, and contributes nothing to any total.

## What each scheme's row contains

A blank cell is *structurally* zero — the scheme has no such step.  `--` means the
stage does not exist for that scheme.

| stage | VECK | VECK+ | VECK* | ours |
| --- | --- | --- | --- | --- |
| `encode` | interpolate `phi` only (`beta = 1`, no expansion) | interpolate + expand to `m` | interpolate + expand to `m` | interpolate + expand to `m` |
| `commit` | `C_phi` | `C_phi` | `C_phi` | `C_phi` |
| `encrypt` | ElGamal of all `ell` symbols (8 shards + 1 full each) | ElGamal of all `m` codeword symbols | Poseidon mask of all `m` symbols | Poseidon mask of all `m` symbols |
| `sample` | -- | `R` positions of `m` | `R` positions of `m` | `R` positions of `m` |
| `subset` | -- | interpolate `f_S`, commit (**unblinded**) | interpolate `f_S` + blinder, commit | interpolate `f_S` + blinder, commit |
| `sample_crypto` | range proofs for all `8*ell` shards | range proofs for the `8R` sampled shards | one ElGamal ciphertext per sampled symbol, no shard split | -- |
| `kzg_proof` | open `phi` at `alpha`, DLEQ over `ell` ciphertexts | quotient, commit, open `f_S`, DLEQ over `R` | quotient, commit, open `f_S` | quotient, commit, open `f_S`, `U_alpha` |
| SNARK (Go) | -- | -- | Groth16 prove | Groth16 prove + CP-Link prove |
| `verify` | opening, DLEQ, shard sums, all range proofs | subset pairing, opening, DLEQ, shard sums, `R` range proofs | subset pairing, opening, Groth16 verify | subset pairing, opening, Groth16 verify, CP-Link verify |

## What no row contains

Excluded from every scheme, so the comparison is like-for-like:

* **SRS generation and load.**  Reported separately as `srs_load_ms`; never in
  `prove_total_ms`.
* **Groth16 setup, circuit compilation and CRS serialisation.**  Reported by the
  Go driver as `setup_ms` / `compile_ms` / `crs_bytes`; one-time per circuit.
  CP-Link setup likewise.
* **The buyer's decryption.**  For VECK and VECK+ this means brute-forcing a
  32-bit discrete log per shard — `8*ell` of them for VECK.  For VECK\* and for us
  the buyer subtracts the PRF stream.
* **Reed–Solomon decoding**, serialisation, network transfer, peak memory, and the
  settlement contract (adaptor signature, on-chain `Ver_key`).
* **The file commitment `C_phi`.**  It is an input to `Enc`, published once and
  amortised over all buyers, so `commit_ms` is reported but not summed into
  `prove_total_ms` (see `PROVE_STAGES` in `scripts/aggregate.py`).

Per-scheme notes:

* **VECK** is extrapolated above `ell = 2^14`: its encryption, range proofs and the
  two size-`ell` DLEQ multi-scalar multiplications are scaled from a measured
  prefix, and those rows are not verified because no ciphertexts are materialised.
  In extrapolated rows the DLEQ MSM is timed over SRS points rather than real
  ciphertexts, which costs the same for the same number of bases.
* **VECK+** is extrapolated above `ell = 2^16`, but only its whole-codeword
  ElGamal: the sampled ciphertexts, range proofs, DLEQ and KZG work are real, and
  those rows still verify.  The re-encryption of the `R` sampled positions is
  performed but not timed, since a streaming prover keeps them from the first pass.
* **VECK\* is not charged for the 32-bit shard split, on either side.**
  Exponential ElGamal forces VECK and VECK+ to split each scalar into `N` 32-bit
  pieces, at `N + 1` ciphertexts per value plus a split-scalar consistency check on
  the verifier, which ties what the buyer decrypts (the shards) to what the KZG
  proof binds (the value).  VECK\*'s sampled ciphertexts are never decrypted that
  way: they are inputs to its SNARK, which proves the encryption relation itself,
  so neither the split nor the check has anything to do.  With both removed,
  VECK\* and our scheme do the same two pairing checks at this layer; their KZG
  verification times still differ because the curves differ (BLS12-377 against
  BLS12-381 / BW6-761), and the rest of the difference lives in the SNARK.
* **VECK\*** is charged the same whole-codeword Poseidon mask as we are, using our
  PRF rather than its MiMC; the reference implementation does not benchmark that
  stage.  Its KZG layer and sampled ElGamal run on BLS12-377, the inner curve of
  the two-chain its BW6-761 circuit needs (see the curve table above).
* **The VECK\* circuit is untouched.**  Its `Circuit`, `Define` and range checks
  are byte-identical to the upstream source, so its constraint counts are the
  originals.  Our circuits drop the upstream range check `SK < |Jubjub|` (or the
  embedded Edwards order on BW6-761), a leftover of in-circuit ElGamal: `SK` is
  uniform in the scalar field and only enters Poseidon2 and the CP-Link.  The
  other edits are benchmark plumbing:
  `const N` moved into build-tag-selected `params_r*.go` files, the durations
  already being printed are also stored in a `metrics` struct, `main` parses flags
  and appends a CSV row, and `-cores` replaces the hard-coded `GOMAXPROCS`.
* **Ours** double-counts `R` symbols of masking: the Go driver recomputes
  Poseidon2 for the `R` circuit inputs on the host.  Against `m = beta * ell` this
  is under a percent of the stage at every size in the sweep.

## Tests

```bash
cd benchmarks/kzg && cargo test --release
```

The suite tampers with the quotient commitment, the opening, the opened value, the
file commitment, a sampled codeword symbol, an ElGamal ciphertext, a shard
ciphertext and a range proof, and requires each to be rejected.  It also checks
that the full `m'`-point codeword is systematic (file symbols reappear at stride
`m'/ell`), that the barycentric Lagrange basis reproduces `f(alpha)` on a
non-subgroup point set — the DLEQ is unsound otherwise — and that each default
sample count is the smallest `R` reaching its target `beta`.

`EFDE-KZG` has its own suite (`cd EFDE-KZG/bls12-381 && cargo test --release`),
including `divide::test::both_strategies_agree`, which requires the two division
strategies to return identical quotients *and* remainders for divisor degrees 64
through 2048.

The Go drivers are covered by `go vet`.

## Caveats

* The host-side PRF is a width-2 Poseidon permutation with 8 full and 50 partial
  rounds and an `x^5` S-box, written out directly in `mask.rs` — the same width
  and round counts as the Poseidon2 in the gnark circuits, which differs only in
  its linear layer (a 2x2 matrix at this width either way).
* For VECK\* and for us the subset polynomial is blinded with a degree-1 multiple
  of the vanishing polynomial, as in `EFDE-KZG`'s own benchmark, so the opened
  value differs from `sum_i L_i(alpha) x_i` by `t(alpha) Z_S(alpha)`.  The work is
  identical either way, but a deployment has to reconcile that term with the
  circuit's `U`.
* VECK+'s subset polynomial is deliberately *not* blinded: its DLEQ proof is only
  sound if the opened value equals `sum_i L_i(alpha) x_i` exactly.
* `LOW_DEGREE_DIVISOR_LIMIT` in `EFDE-KZG/*/src/divide.rs` decides which division
  strategy `(phi - f_S) / Z_S` uses.  It is a cache property, so re-measure it
  before quoting `kzg_proof_ms` on new hardware:
  `cargo test --release -- --ignored divide_threshold_probe --nocapture`.  The
  measured crossover is between 1280 and 1536 and the constant is 1280, which puts
  `R = 1053`, `609` and `386` on the blocked path and `R = 2384` on the plain
  Newton one.
* `R + 2 <= ell` is required for the subset relation to be non-degenerate.
* The CP-Link layer is the Kiltz–Wee QA-NIZK stand-in from the reference
  implementation, run on dummy commitments; see `EFDE-SNARK/*/main.go`.
