# Changelog

This project follows semantic versioning.

Possible log types:

- `[added]` for new features.
- `[changed]` for changes in existing functionality.
- `[deprecated]` for once-stable features removed in upcoming releases.
- `[removed]` for deprecated features removed in this release.
- `[fixed]` for any bug fixes.
- `[security]` to invite users to upgrade in case of vulnerabilities.


### Unreleased

- ...

### v0.6.1 (2026-09-27)

- [fixed] Ignore display records in header-based airspace separation (#55)

### v0.6.0 (2026-08-22)

- [added] Add support for OpenAir v2 `AY` airspace types (#52)
- [added] Add `Airspace::normalize_legacy_class()` (#52)
- [changed] Align `AC` airspace classes with the OpenAir v2 specification (#52)
- [changed] Expose `ActivationTimes` fields (#53)

### v0.5.0 (2026-04-19)

- [added] Implement writing functionality for OpenAir files (#43)
- [added] Add support for activation times extension records (#38)
- [added] Implement header-based airspace separation (#44)
- [changed] Make `Airspace::name` optional (#48)
- [changed] Update to Rust Edition 2024 (#39)
- [changed] Replace `regex` dependency with manual parsers for coordinate and altitude parsing

### v0.4.0 (2025-10-18)

- [added] Add support for AC UNC (Unclassified) (#19)
- [added] Add support for AX transponder code extension records (#13)
- [changed] Ignore unknown A* extension records (#20)
- [changed] Drop `lazy_static` dependency (#16)
- [fixed] Add missing pub keywords to public structs (#15)
- [fixed] Fix `Coord::parse_component()` to correctly parse DDM format (#17)

### v0.3.2 (2024-10-12)

- [fixed] Keep serializing `Class::Ctr` as `CTR`

### v0.3.1 (2024-10-12)

- [fixed] Fix missing example in crates.io release

### v0.3.0 (2024-10-12)

- [added] Allow altitude as "ft MSL" (#9)
- [changed] Rename `Class::CTR` to `Class::Ctr`
- [changed] Update to Rust 2021 edition

### v0.2.0 (2019-06-06)

- [changed] Improved parsing support

### v0.1.4 (2019-04-28)

- [added] Support for serde serialization

### v0.1.3 (2019-04-26)

- [added] Class: Support more airspace classes
- [added] Altitude: Add SFC as alias for GND
- [added] Coord: Allow period as separator
- [changed] Ignore empty lines

### v0.1.2 (2019-04-26)

- First crates.io release
