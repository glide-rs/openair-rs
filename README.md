# openair-rs

[![Build status](https://github.com/dbrgn/openair-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/dbrgn/openair-rs/actions/workflows/ci.yml)
[![Docs][docs-badge]][docs]

A Rust parser for airspace files in OpenAir format (used by flight instruments
like Skytraxx and others).

http://www.winpilot.com/UsersGuide/UserAirspace.asp (see also `FORMAT.txt`)

Docs: https://docs.rs/openair/


## Status

Supported file format features:

- [x] Parse airspace metadata
- [ ] Parse terrain metadata
- [x] Support polygon points
- [x] Support circles
- [x] Support arcs
- [x] Support AY/AF/AG extension records

Label placement hints (AT) and style records (SP, SB) are not supported.


## Implementation Notes

Unfortunately the `OpenAir` format is really underspecified. Every device
uses varying conventions. For example, there is nothing we can use as clear
delimiter for airspaces. Some files delimit airspaces with an empty line,
some with a comment. But on the other hand, some files even place comments
between the coordinates so that they cannot be used as delimiter either.

This parser tries to be very lenient when parsing, based on real life data.
The end of an airspace is reached when the next one starts (with an `AC`
record) or when the file ends.

**NOTE: Altitude levels without a unit specifier (e.g. "1000 GND") will be
treated as feet!)**


## Example

You can try the parser using the example program:

    $ cargo run --example parse_file example_data/Switzerland.txt


## Serde Serialization

To include serialization of all data types, enable the `serde` feature.


## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT) at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.


<!-- Badges -->
[docs]: https://docs.rs/openair/
[docs-badge]: https://img.shields.io/badge/docs-docs.rs-yellow.svg?maxAge=3600
