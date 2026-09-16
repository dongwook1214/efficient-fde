//! Before/after for the `subset_ms` stage of `build_subset`.
//!
//!     cargo run --release --example subset_breakdown
//!
//! The old path is kept here verbatim so the two can be timed side by side and
//! checked against each other; equality is asserted, so a wrong answer cannot be
//! reported as a speedup.

use std::time::Instant;

use ark_bls12_381::{Bls12_381, Fr};
use ark_ff::{Field, UniformRand, Zero};
use ark_poly::univariate::DensePolynomial;
use ark_poly::{
    DenseUVPolynomial, EvaluationDomain, Evaluations, GeneralEvaluationDomain, Polynomial,
};

use efde_kzg::commit::kzg::Powers;
use efde_kzg::veck::{interpolate_indices_with_vanishing, to_vanishing_poly, vanishing_poly_dense};

const SUBSETS: [usize; 4] = [386, 609, 1053, 2384];
const LOG_ELL: u32 = 16;
const BETA: f64 = 1.100022;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// The interpolation `interpolate_points` used to run: per point, one dense
/// polynomial division, one evaluation, one inversion and two allocations.
fn interpolate_points_naive(
    points: &[Fr],
    values: &[Fr],
    vanishing: &DensePolynomial<Fr>,
) -> DensePolynomial<Fr> {
    let mut result = DensePolynomial::zero();
    for (&point, &value) in points.iter().zip(values.iter()) {
        let divisor = DensePolynomial::from_coefficients_slice(&[-point, Fr::from(1u64)]);
        let numerator = vanishing / &divisor;
        let denominator = numerator.evaluate(&point);
        let scale = value * denominator.inverse().unwrap();
        let scaled = DensePolynomial::from_coefficients_vec(
            numerator.coeffs.iter().map(|c| *c * scale).collect(),
        );
        result += &scaled;
    }
    result
}

fn main() {
    let rng = &mut ark_std::test_rng();
    let ell = 1usize << LOG_ELL;
    let m = (BETA * ell as f64).ceil() as usize;
    let code_len = m.next_power_of_two();
    let domain = GeneralEvaluationDomain::<Fr>::new(code_len).unwrap();
    let evals: Vec<Fr> = (0..code_len).map(|_| Fr::rand(rng)).collect();
    let codeword = Evaluations::from_vec_and_domain(evals, domain);

    println!("ell = 2^{}, beta = {:.3}   (times in ms)", LOG_ELL, BETA);
    println!(
        "{:>6} | {:>9} {:>9} {:>9} | {:>9} {:>9} {:>9} | {:>7} {:>6} {:>8}",
        "R", "Z_S x2", "interp", "OLD", "Z_S", "interp", "NEW", "blind", "MSM", "speedup"
    );

    for r in SUBSETS {
        let mut positions: Vec<usize> = (0..r).map(|i| (i * 7919 + 13) % m).collect();
        positions.sort_unstable();
        positions.dedup();
        assert_eq!(positions.len(), r);

        let powers = Powers::<Bls12_381>::unsafe_setup(Fr::rand(rng), r + 4);
        let points: Vec<Fr> = positions.iter().map(|&i| domain.element(i)).collect();
        let values: Vec<Fr> = positions.iter().map(|&i| codeword.evals[i]).collect();

        // ---- old: sparse Z_S, built once here and again inside interpolate_points
        let t = Instant::now();
        let vanishing_sparse = DensePolynomial::from(to_vanishing_poly(positions.clone(), domain));
        let t_old_van = 2.0 * ms(t);

        let t = Instant::now();
        let old = interpolate_points_naive(&points, &values, &vanishing_sparse);
        let t_old_interp = ms(t);

        // ---- new: dense Z_S, built once and handed to the interpolation
        let t = Instant::now();
        let vanishing = vanishing_poly_dense(&positions, domain);
        let t_new_van = ms(t);
        assert_eq!(vanishing, vanishing_sparse, "Z_S mismatch at R={}", r);

        let t = Instant::now();
        let new = interpolate_indices_with_vanishing(&codeword, &positions, &vanishing);
        let t_new_interp = ms(t);
        assert_eq!(new, old, "interpolation mismatch at R={}", r);

        // ---- unchanged stages
        let t = Instant::now();
        let blinder = DensePolynomial::from_coefficients_vec(vec![Fr::rand(rng), Fr::rand(rng)]);
        let blinded = new.clone() + &blinder * &vanishing;
        let t_blind = ms(t);

        let t = Instant::now();
        let _ = powers.commit_g1(&blinded);
        let t_msm = ms(t);

        let old_total = t_old_van + t_old_interp + t_blind + t_msm;
        let new_total = t_new_van + t_new_interp + t_blind + t_msm;
        println!(
            "{:>6} | {:>9.1} {:>9.1} {:>9.1} | {:>9.1} {:>9.1} {:>9.1} | {:>7.1} {:>6.1} {:>7.1}x",
            r,
            t_old_van,
            t_old_interp,
            old_total,
            t_new_van,
            t_new_interp,
            new_total,
            t_blind,
            t_msm,
            old_total / new_total
        );
    }
}
