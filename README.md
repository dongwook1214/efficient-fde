# Efficient-FDE

Reference implementation and benchmark suite for **Efficient Fair Data Exchange**.

```
Efficient-fde/
├── EFDE-KZG/            # KZG layer (Rust, arkworks)
│   ├── bls12-381/       #   BLS12-381 instantiation
│   └── bw6-761/         #   BW6-761  instantiation
├── EFDE-SNARK/          # Groth16 circuit + CP-Link (Go, gnark)
│   ├── bls12-381/
│   └── bw6-761/
├── baselines/           # vendored reference implementations
│   ├── fde/             #   VECK_EL / VECK+_EL primitives (Rust)
│   └── veck-star-snark/ #   VECK*_EL circuit (Go)
└── benchmarks/          # the end-to-end evaluation
    ├── kzg/             #   encoding + encryption + KZG, all schemes, both curves
    ├── scripts/         #   run_all.sh, aggregate.py, plot.py
    └── results/         #   CSVs, pgfplots blocks, figures
```

## Quickstart

```bash
./benchmarks/scripts/run_all.sh              # full sweep, ell = 2^10 .. 2^20
MAX_LOG=14 ./benchmarks/scripts/run_all.sh   # short pass
```

Results land in `benchmarks/results/`.  See [`benchmarks/README.md`](benchmarks/README.md)
for what each stage measures, which scheme runs on which curve, and which numbers
are extrapolated.

## EFDE-KZG (Rust)

The polynomial commitment, the subset division, and the verifiable-encryption
checks, built on [arkworks](https://arkworks.rs/) (`ark-poly-commit`, `ark-ec`,
`ark-poly`).

| Variant     | Curve     | Dependency      |
| ----------- | --------- | --------------- |
| `bls12-381` | BLS12-381 | `ark-bls12-381` |
| `bw6-761`   | BW6-761   | `ark-bw6-761`   |

The benchmark driver additionally instantiates BLS12-377 (`ark-bls12-377`) for
VECK\*\_EL's KZG layer; see `benchmarks/README.md`.

Both directories contain the same library: `commit`, `divide` and `veck` are
generic over `ark_ec::pairing::Pairing`, and only the crates' own tests and
`main.rs` pin a curve.

```bash
cd EFDE-KZG/bls12-381   # or bw6-761
cargo test --release -- --nocapture
cargo run --release -- setup-cache --range 1048576   # pre-generate powers of tau
```

## EFDE-SNARK (Go)

A Groth16 circuit in [gnark](https://github.com/consensys/gnark) proving the
encryption relation `CT[i] == X[i] + Poseidon2(SK, SRPrime[i])` together with the
linear combination `sum_i X[i]*L[i] == U`.  `SK` and `U` are committed witnesses
linked outside the circuit via **CP-Link** (a Kiltz–Wee QA-NIZK; a dummy instance
is used for benchmarking), so the circuit contains no elliptic-curve gadget and
runs on BLS12-381 rather than on a 2-chain.

| Variant     | Curve     | Package                      |
| ----------- | --------- | ---------------------------- |
| `bls12-381` | BLS12-381 | `gnark-crypto/ecc/bls12-381` |
| `bw6-761`   | BW6-761   | `gnark-crypto/ecc/bw6-761`   |

The number of sampled positions `R` is a compile-time constant chosen by build tag:

```bash
cd EFDE-SNARK/bls12-381
go run -tags r609 . -csv ../../benchmarks/results/snark.csv
```

Available tags: `r2384`, `r1053`, `r609`, `r386`.

## Baselines

`baselines/` holds vendored copies of the implementations compared against, so the
evaluation reproduces from one clone.  See [`baselines/README.md`](baselines/README.md)
for provenance and the list of changes.
