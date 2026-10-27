# JWT provider compatibility fixtures

Issue #948 preserves SPEC #580's HS256/shared-secret and Claims contract while
replacing jsonwebtoken's RustCrypto provider with its official AWS-LC provider.
The secret below is public test data, unrelated to any llmlb installation:
`public-issue-948-compatibility-fixture-not-a-real-secret`.

`provider-compat.json` was generated on 2026-10-26 by `generate.rs`, using
jsonwebtoken **11.1.0**, its **rust_crypto** feature and unchanged default
**use_pem** feature, on Rust 1.95.0 / Windows x86_64 MSVC. It contains admin and
viewer tokens with all five Claims fields. Expiration is 2100-01-01 UTC so the
compatibility test is independent of the generation date. Application-generated
tokens still expire after 24 hours; the unit suite checks that separately.

## Reproduce independently

Copy `generate.rs` to `src/main.rs` in a temporary directory outside the workspace
and create this manifest there:

```toml
[package]
name = "jwt-provider-fixture"
version = "0.0.0"
edition = "2021"
[workspace]
[dependencies]
jsonwebtoken = { version = "=11.1.0", features = ["rust_crypto"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Run `cargo run --manifest-path <temporary>/Cargo.toml` to generate the fixtures.
Pass a fixture JSON path after `--` to verify its signatures and all claims.
Verification requires exactly one admin fixture and one viewer fixture; empty,
incomplete or duplicate-role input fails without reporting PASS.
Use a second temporary crate with only `features = ["aws_lc_rs"]` to generate
new-provider tokens. Match the application lock by also adding
`aws-lc-rs = { version = "=1.18.1", features = ["prebuilt-nasm"] }` and
`aws-lc-sys = "=0.45.0"` to that temporary manifest;
an unconstrained resolution can select newer native toolchain requirements.
The application's existing rustls dependencies already enable `prebuilt-nasm`,
which is needed for this Windows fixture tool when NASM is unavailable.
Pass its output to the old-provider verifier. Also
pass the old-provider fixtures to the new-provider verifier. Never enable both
providers in one crate. No old provider or comparison lockfile belongs in the
application dependency graph.

The regular `unit_tests` JWT test decodes both old fixtures through `verify_jwt`
and checks every claim. It then requires the new provider's HS256 encoding to be
byte-identical to the legacy token. Since HS256 signatures are deterministic,
this tests both wire-format directions on every supported CI platform without
retaining vulnerable comparison dependencies in the application.

Missing expiry, expired tokens, different secrets, tampered signatures, malformed
tokens, HS384/HS512 and unsigned `none` tokens are rejected by the same JWT suite.
