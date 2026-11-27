# Third-party crates of the fetch program

The crates the fetch program (`abruf/`) adds to those of the service, each with its licence as `cargo metadata` names it. Built: whether it is compiled for the Linux targets of this project; the others stand in `Cargo.lock` only because cargo resolves every target. `webpki-roots` carries CDLA-Permissive-2.0 (the Mozilla root certificates as data), which asks that its text goes with the data. The licence files of every built crate lie under `abruf/lizenzen/<crate>-<version>/`, copied unchanged from the published crate, and travel with the image under `/lizenzen`. The witness `tests/zaeune.rs` holds this list against the lock file and the folder.

| crate | licence | built | repository |
| --- | --- | --- | --- |
| `adler2 2.0.1` | 0BSD OR MIT OR Apache-2.0 | yes | https://github.com/oyvindln/adler2 |
| `base64 0.23.1` | MIT OR Apache-2.0 | yes | https://github.com/marshallpierce/rust-base64 |
| `crc32fast 1.5.6` | MIT OR Apache-2.0 | yes | https://github.com/srijs/rust-crc32fast |
| `csv 1.4.0` | Unlicense/MIT | yes | https://github.com/BurntSushi/rust-csv |
| `csv-core 0.1.13` | Unlicense/MIT | yes | https://github.com/BurntSushi/rust-csv |
| `flate2 1.1.11` | MIT OR Apache-2.0 | yes | https://github.com/rust-lang/flate2-rs |
| `getrandom 0.2.17` | MIT OR Apache-2.0 | yes | https://github.com/rust-random/getrandom |
| `miniz_oxide 0.9.1` | MIT OR Zlib OR Apache-2.0 | yes | https://github.com/Frommi/miniz_oxide |
| `percent-encoding 2.3.2` | MIT OR Apache-2.0 | yes | https://github.com/servo/rust-url |
| `ring 0.17.14` | Apache-2.0 AND ISC | yes | https://github.com/briansmith/ring |
| `rustls 0.23.46` | Apache-2.0 OR ISC OR MIT | yes | https://github.com/rustls/rustls |
| `rustls-pki-types 1.16.0` | MIT OR Apache-2.0 | yes | https://github.com/rustls/pki-types |
| `rustls-webpki 0.103.17` | ISC | yes | https://github.com/rustls/webpki |
| `ryu 1.0.23` | Apache-2.0 OR BSL-1.0 | yes | https://github.com/dtolnay/ryu |
| `simd-adler32 0.3.10` | MIT | yes | https://github.com/mcountryman/simd-adler32 |
| `untrusted 0.9.0` | ISC | yes | https://github.com/briansmith/untrusted |
| `ureq 3.5.2` | MIT OR Apache-2.0 | yes | https://github.com/algesten/ureq |
| `ureq-proto 0.7.1` | MIT OR Apache-2.0 | yes | https://github.com/algesten/ureq-proto |
| `utf8-zero 0.8.1` | MIT OR Apache-2.0 | yes | https://github.com/algesten/utf8-zero |
| `webpki-roots 1.0.12` | CDLA-Permissive-2.0 | yes | https://github.com/rustls/webpki-roots |
| `windows-sys 0.52.0` | MIT OR Apache-2.0 | no (other targets) | https://github.com/microsoft/windows-rs |
| `windows-targets 0.52.6` | MIT OR Apache-2.0 | no (other targets) | https://github.com/microsoft/windows-rs |
| `windows_aarch64_gnullvm 0.52.6` | MIT OR Apache-2.0 | no (other targets) | https://github.com/microsoft/windows-rs |
| `windows_aarch64_msvc 0.52.6` | MIT OR Apache-2.0 | no (other targets) | https://github.com/microsoft/windows-rs |
| `windows_i686_gnu 0.52.6` | MIT OR Apache-2.0 | no (other targets) | https://github.com/microsoft/windows-rs |
| `windows_i686_gnullvm 0.52.6` | MIT OR Apache-2.0 | no (other targets) | https://github.com/microsoft/windows-rs |
| `windows_i686_msvc 0.52.6` | MIT OR Apache-2.0 | no (other targets) | https://github.com/microsoft/windows-rs |
| `windows_x86_64_gnu 0.52.6` | MIT OR Apache-2.0 | no (other targets) | https://github.com/microsoft/windows-rs |
| `windows_x86_64_gnullvm 0.52.6` | MIT OR Apache-2.0 | no (other targets) | https://github.com/microsoft/windows-rs |
| `windows_x86_64_msvc 0.52.6` | MIT OR Apache-2.0 | no (other targets) | https://github.com/microsoft/windows-rs |
| `zlib-rs 0.6.9` | Zlib | no (other targets) | https://github.com/trifectatechfoundation/zlib-rs |
