# Math Primitive Reference

> **Crate:** `commonware-math`

Algebraic structures for cryptographic operations: fields, polynomials, and NTT (Number Theoretic Transform).

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Field implementation | `search_code(query="Goldilocks", crate="commonware-math", version=PINNED)` |
| Polynomial API | `search_code(query="pub struct Poly", crate="commonware-math", version=PINNED)` |
| NTT module | `search_code(query="ntt", crate="commonware-math", version=PINNED)` |
| Algebraic traits | `search_code(query="pub trait", crate="commonware-math", version=PINNED)` |
| Module structure | `list_files(crate="commonware-math", version=PINNED)` |

Note: `pub fn ntt` returns nothing — NTT is a module (`pub mod ntt`), not a standalone function. Use `ntt` as the query.

## When to Use This Primitive

### Use commonware-math when:
- Implementing polynomial commitment schemes
- Building secret sharing protocols (Shamir's, VSS)
- Performing polynomial interpolation/evaluation
- Working with finite field arithmetic for cryptography
- Implementing erasure coding internals

### Don't use directly when:
- Using higher-level cryptographic primitives (use `commonware-cryptography` instead)
- Doing standard hashing or signing (use the appropriate crypto primitive)
- Working with erasure coding at the API level (use `commonware-coding`)

## Key Components

### Goldilocks Field (`F`)

A fast prime field with modulus `2^64 - 2^32 + 1`.

**Why Goldilocks?**
- Fits in 64-bit integers for fast arithmetic
- Special form enables efficient modular reduction
- NTT-friendly: has high 2-adic order (2^32)
- Used in modern ZK systems (Plonky2, etc.)

**When to use**: implementing polynomial arithmetic, building secret sharing schemes, finite field operations in cryptographic protocols.

The `F` struct supports standard arithmetic operations (addition, subtraction, multiplication) via operator overloads, and provides `inverse()` for multiplicative inverses. Create elements with `F::from(42u64)`. Fetch the full API: `search_code(query="Goldilocks", crate="commonware-math", version=PINNED)`.

### Polynomials (`Poly`)

Polynomial arithmetic with coefficients in a field.

**When to use**: polynomial evaluation at points, Lagrange interpolation (reconstructing secrets), polynomial commitment schemes, secret sharing (polynomial at x=0 is the secret).

A `Poly` is created from a coefficient vector `[a_0, a_1, ..., a_n]` representing `a_0 + a_1*x + a_2*x^2 + ...`. Key operations: `evaluate(&point)` for single-point evaluation, and `interpolate(&points)` for Lagrange interpolation from a set of `(x, y)` pairs. Fetch the full API: `search_code(query="pub struct Poly", crate="commonware-math", version=PINNED)`.

### Number Theoretic Transform (NTT)

Fast polynomial multiplication using FFT over finite fields.

**When to use**: fast polynomial multiplication (O(n log n) vs O(n^2)), batch polynomial operations, erasure coding computations.

**Why NTT over FFT?** Works over finite fields (exact arithmetic, no floating point), required for cryptographic applications, and the Goldilocks field has roots of unity for efficient NTT.

The NTT module provides forward transform (`ntt`) and inverse transform (`intt`). Transform coefficient vectors to the NTT domain, perform pointwise multiplication, then transform back. The module also provides a `Matrix` type for batch operations. Fetch the implementation: `search_code(query="ntt", crate="commonware-math", version=PINNED)`.

## Algebraic Traits

| Trait | Purpose | Examples |
|-------|---------|----------|
| `Additive` | Addition, subtraction, zero | Field elements |
| `Multiplicative` | Multiplication, division, one | Field elements |
| `Ring` | Both additive and multiplicative | Integers mod n |
| `Field` | Ring with multiplicative inverses | Goldilocks field |
| `CryptoGroup` | Cryptographic group operations | Elliptic curve points |

## Use Cases in Commonware

### In Secret Sharing (DKG)
The math primitive powers polynomial-based secret sharing:
1. Generate random polynomial with secret as constant term
2. Evaluate polynomial at participant indices
3. Participants can interpolate to recover secret

### In Erasure Coding
Reed-Solomon encoding uses polynomial evaluation:
1. Treat data as polynomial coefficients
2. Evaluate at more points than original data
3. Any k points can reconstruct via interpolation

### In Threshold Signatures
Combines with BLS for threshold schemes:
1. Secret key is polynomial evaluation at x=0
2. Key shares are evaluations at other points
3. Lagrange interpolation combines partial signatures
