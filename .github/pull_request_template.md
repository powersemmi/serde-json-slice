# Description

What the change does and why. Link the issue it addresses.

Fixes # (issue number)

## Type of change

- [ ] Documentation
- [ ] Bug fix (a non-breaking change that resolves an issue)
- [ ] New feature (a non-breaking change that adds functionality)
- [ ] Breaking change (removing or renaming public API, changing a signature, tightening bounds,
      or raising the MSRV; a minor version bump before 1.0)

## Checklist

- [ ] `cargo fmt --check` and `cargo clippy --all-targets` with all features and with none pass
      with no warnings
- [ ] `cargo test --all-features` passes
- [ ] The crate builds for `thumbv7em-none-eabihf` with no features
- [ ] Output stays byte-identical to `serde_json`, and new behaviour has tests
- [ ] Public items carry rustdoc with a compiling `# Examples` doctest
