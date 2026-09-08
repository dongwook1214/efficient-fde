//go:build r2384

package main

// R = 2384 samples, i.e. a codeword expansion of beta = 1.1.
//
// 9,573,767 constraints: Groth16 setup needs tens of minutes and a
// multi-gigabyte CRS.  `-compile-only` reports the constraint count without it.
const N = 2384
