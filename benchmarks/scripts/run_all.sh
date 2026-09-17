#!/usr/bin/env bash
#
# Full benchmark sweep used in the paper.
#
#   ./benchmarks/scripts/run_all.sh
#
# Environment overrides:
#   MIN_LOG / MAX_LOG   file-size exponents            (default 10 / 20)
#   SUBSETS             sample counts R                (default 2384,1053,609,386)
#   RESULTS             output directory               (default benchmarks/results)
#   VECK_CAP            measured prefix for base VECK  (default 14)
#   VECK_PLUS_CAP       measured prefix for VECK+      (default 16)
#   SKIP_SNARK=1        skip the Go drivers
#   SKIP_KZG=1          skip the Rust driver
#
# The `*_CAP` values bound how much of the payload the *linear* public-key stages
# are actually run on; larger files reuse the measured per-symbol cost.  Set both
# to a value >= MAX_LOG (or pass --no-extrapolate) for a fully measured, multi-day
# run.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
RESULTS=${RESULTS:-$ROOT/benchmarks/results}
MIN_LOG=${MIN_LOG:-10}
MAX_LOG=${MAX_LOG:-20}
SUBSETS=${SUBSETS:-2384,1053,609,386}
VECK_CAP=${VECK_CAP:-14}
VECK_PLUS_CAP=${VECK_PLUS_CAP:-16}

mkdir -p "$RESULTS"

if [ "${SKIP_KZG:-0}" != "1" ]; then
  echo "=== encoding / encryption / KZG (Rust) ==========================="
  cd "$ROOT/benchmarks/kzg"
  cargo build --release

  run_kzg() {
    local scheme=$1 curve=$2
    shift 2
    echo "--- $scheme on $curve"
    ./target/release/efde-bench \
      --scheme "$scheme" --curve "$curve" \
      --min-log "$MIN_LOG" --max-log "$MAX_LOG" --subsets "$SUBSETS" \
      --out "$RESULTS/kzg_${scheme}_${curve}.csv" "$@"
  }

  # BLS12-381: every scheme that can live on a plain pairing-friendly curve.
  run_kzg ours       bls12-381
  run_kzg veck-plus  bls12-381 --max-measured-log "$VECK_PLUS_CAP"
  run_kzg veck       bls12-381 --max-measured-log "$VECK_CAP"
  # BW6-761: ours, for the same-SNARK-curve comparison.  VECK*'s in-circuit
  # ElGamal forces its commitment onto the inner curve of that two-chain, so its
  # KZG layer is measured on BLS12-377 while its circuit is measured on BW6-761.
  run_kzg ours       bw6-761
  run_kzg veck-star  bls12-377
fi

if [ "${SKIP_SNARK:-0}" != "1" ]; then
  echo "=== Groth16 + CP-link (Go) ======================================="
  SNARK_CSV="$RESULTS/snark.csv"
  rm -f "$SNARK_CSV"
  for tag in ${SUBSETS//,/ }; do
    for dir in "$ROOT/EFDE-SNARK/bls12-381" "$ROOT/EFDE-SNARK/bw6-761" "$ROOT/baselines/veck-star-snark"; do
      echo "--- $(basename "$(dirname "$dir")")/$(basename "$dir") at R=$tag"
      (cd "$dir" && go run -tags "r$tag" . -csv "$SNARK_CSV")
    done
  done
fi

echo "=== aggregate ===================================================="
python3 "$ROOT/benchmarks/scripts/aggregate.py" --results "$RESULTS"
python3 "$ROOT/benchmarks/scripts/plot.py" || echo "(matplotlib not installed; skipped the preview figure)"
