//! End-to-end sender-side benchmark for VECK, VECK+, VECK* and EFDE.
//!
//! Run one (scheme, curve) pair per invocation; see `benchmarks/scripts/run_all.sh`
//! for the full sweep used in the paper.

mod config;
mod elgamal;
mod encode;
mod mask;
mod report;
mod sample;
mod schemes;
mod srs;
mod timer;
#[cfg(test)]
mod tests;

use ark_bls12_377::Bls12_377;
use ark_bls12_381::Bls12_381;
use ark_bw6_761::BW6_761;
use ark_ec::pairing::Pairing;
use ark_ff::PrimeField;
use config::{Curve, Scheme};
use fde::encrypt::elgamal::MAX_BITS;

/// Number of 32-bit shards an exponential-ElGamal plaintext is split into.
const fn shard_count(modulus_bits: usize) -> usize {
    modulus_bits / MAX_BITS + 1
}

const BLS12_381_SHARDS: usize =
    shard_count(<Bls12_381 as Pairing>::ScalarField::MODULUS_BIT_SIZE as usize);
const BLS12_377_SHARDS: usize =
    shard_count(<Bls12_377 as Pairing>::ScalarField::MODULUS_BIT_SIZE as usize);
const BW6_761_SHARDS: usize =
    shard_count(<BW6_761 as Pairing>::ScalarField::MODULUS_BIT_SIZE as usize);

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cfg = config::parse_args(std::env::args().skip(1).collect())?;

    // VECK* operates on its sampled ElGamal ciphertexts inside the circuit, so
    // they must share the group the commitment lives in, and that group must be
    // the inner curve of the two-chain the proof runs on: BLS12-377 under
    // BW6-761.  The SNARK is a separate binary and is measured on BW6-761.
    if cfg.scheme == Scheme::VeckStar && cfg.curve != Curve::Bls12_377 {
        return Err(
            "VECK* commits and samples on the inner curve of its two-chain; pass \
             --curve bls12-377 (its circuit is measured separately on BW6-761)"
                .to_string(),
        );
    }
    if cfg.curve == Curve::Bls12_377 && cfg.scheme != Scheme::VeckStar {
        return Err(
            "bls12-377 is only defined for veck-star, which is forced onto it by its two-chain"
                .to_string(),
        );
    }
    if cfg.curve == Curve::Bw6_761 && matches!(cfg.scheme, Scheme::Veck | Scheme::VeckPlus) {
        return Err(
            "VECK and VECK+ are only instantiated on BLS12-381 in the paper; pass --curve bls12-381"
                .to_string(),
        );
    }

    let mut writer = report::Writer::create(&cfg.out).map_err(|err| err.to_string())?;
    match cfg.curve {
        Curve::Bls12_381 => schemes::run::<BLS12_381_SHARDS, Bls12_381>(&cfg, &mut writer)?,
        Curve::Bls12_377 => schemes::run::<BLS12_377_SHARDS, Bls12_377>(&cfg, &mut writer)?,
        Curve::Bw6_761 => schemes::run::<BW6_761_SHARDS, BW6_761>(&cfg, &mut writer)?,
    }

    eprintln!("wrote {}", cfg.out.display());
    Ok(())
}
