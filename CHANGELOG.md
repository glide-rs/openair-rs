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
