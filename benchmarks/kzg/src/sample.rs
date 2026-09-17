//! Fiat--Shamir sampling of the `R` codeword positions the buyer will check.
//!
//! The challenge is bound to the whole transmitted ciphertext, so absorbing the
//! transcript is linear in the codeword.  That pass runs once; the rejection
//! sampling that follows never touches it again, drawing its candidates from
//! `SHA-256(seed || counter)` instead, four per hash.  Re-absorbing the
//! transcript per candidate would make an `O(R \cdot m)` stage out of an
//! `O(m + R)` one.

use ark_ff::PrimeField;
use ark_serialize::CanonicalSerialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

/// Bytes buffered before each `update`.  One call per field element spends more
/// on call overhead than on hashing at these element sizes.
const BLOCK_BYTES: usize = 16 * 1024;

/// Absorbs a transcript into a Fiat--Shamir seed.
pub struct Transcript {
    hasher: Sha256,
    block: Vec<u8>,
}

impl Transcript {
    pub fn new(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update((domain.len() as u64).to_le_bytes());
        hasher.update(domain);
        Self {
            hasher,
            block: Vec::with_capacity(BLOCK_BYTES + 128),
        }
    }

    fn flush_if_full(&mut self) {
        if self.block.len() >= BLOCK_BYTES {
            self.hasher.update(&self.block);
            self.block.clear();
        }
    }

    pub fn absorb<T: CanonicalSerialize>(&mut self, item: &T) {
        item.serialize_compressed(&mut self.block)
            .expect("serialisation should not fail");
        self.flush_if_full();
    }

    /// Length-prefixed, so two vectors cannot be concatenated into one.
    pub fn absorb_many<T: CanonicalSerialize>(&mut self, items: &[T]) {
        self.absorb_u64(items.len() as u64);
        for item in items {
            item.serialize_compressed(&mut self.block)
                .expect("serialisation should not fail");
            self.flush_if_full();
        }
    }

    pub fn absorb_u64(&mut self, value: u64) {
        self.block.extend_from_slice(&value.to_le_bytes());
        self.flush_if_full();
    }

    pub fn finalize(mut self) -> [u8; 32] {
        self.hasher.update(&self.block);
        self.hasher.finalize().into()
    }
}

/// Derive `count` distinct positions in `0..len` from an absorbed transcript.
///
/// Each `SHA-256(tag || seed || counter)` yields four 64-bit candidates.
/// Candidates at or above `zone` are discarded so that the reduction mod `len`
/// stays uniform.
pub fn derive_positions(seed: &[u8; 32], len: usize, count: usize) -> Vec<usize> {
    assert!(count <= len, "cannot sample more positions than the codeword has");
    if count == 0 {
        return Vec::new();
    }
    let modulus = len as u64;
    let zone = (u64::MAX / modulus) * modulus;

    let mut chosen = HashSet::with_capacity(count);
    let mut counter: u64 = 0;
    while chosen.len() < count {
        let mut hasher = Sha256::new();
        hasher.update(b"fde:subset");
        hasher.update(seed);
        hasher.update(counter.to_le_bytes());
        let digest = hasher.finalize();
        counter += 1;
        for word in digest.chunks_exact(8) {
            let candidate = u64::from_le_bytes(word.try_into().expect("eight bytes"));
            if candidate < zone {
                chosen.insert((candidate % modulus) as usize);
                if chosen.len() == count {
                    break;
                }
            }
        }
    }

    let mut positions: Vec<usize> = chosen.into_iter().collect();
    positions.sort_unstable();
    positions
}

/// Derive the evaluation point `alpha` from a transcript.
pub fn challenge_scalar<F: PrimeField>(seed: &[u8; 32], label: &[u8]) -> F {
    let mut hasher = Sha256::new();
    hasher.update(seed);
    hasher.update(label);
    F::from_le_bytes_mod_order(&hasher.finalize())
}

/// Barycentric Lagrange basis of the sampled point set, evaluated at `alpha`.
///
/// The sampled positions are an arbitrary subset of the codeword domain, not a
/// subgroup, so the FFT shortcut used by the reference implementations does not
/// apply and we evaluate the basis directly, exactly as the SNARK host code does.
pub fn lagrange_coefficients<F: PrimeField>(points: &[F], alpha: F) -> Vec<F> {
    let n = points.len();
    let mut weights = vec![F::one(); n];
    let mut alpha_diffs = vec![F::zero(); n];

    for i in 0..n {
        alpha_diffs[i] = alpha - points[i];
        let mut acc = F::one();
        for j in 0..n {
            if i != j {
                acc *= points[i] - points[j];
            }
        }
        weights[i] = acc;
    }

    // w_i = 1 / prod_{j != i} (x_i - x_j), and 1 / (alpha - x_i).
    ark_ff::batch_inversion(&mut weights);
    ark_ff::batch_inversion(&mut alpha_diffs);

    let mut scaled = vec![F::zero(); n];
    let mut sum = F::zero();
    for i in 0..n {
        scaled[i] = weights[i] * alpha_diffs[i];
        sum += scaled[i];
    }

    let sum_inv = sum
        .inverse()
        .expect("barycentric denominator must be non-zero");
    scaled.iter().map(|value| *value * sum_inv).collect()
}
