# neo-rimage vendor notes

Source: <https://github.com/vlad-salone/rimage>, tag `v0.14.0`.

- Version: `0.14.0`
- Revision: `978a87075ad60fbbd033ba851036fbe84e9e0f47`
- License: MIT OR Apache-2.0 (see `LICENSE-MIT` and `LICENSE-APACHE`)

The vendored source is unchanged except for `build.rs`. It also watches
`CARGO_FEATURE_BUILD_BINARY` and generates the Windows VERSION resource only
when rimage's `build-binary` feature is enabled.
Library consumers must not embed the CLI resource because it conflicts with the
Tauri application's own Windows resource during final linking.

The application retains its explicit library feature whitelist and pins
`zune-core = 0.5.1` to match upstream's compatibility requirement. AVIF/SVG/TIFF
decoding, the upstream CLI, and upstream parallel codec features remain disabled.

The neo-rimage local orchestration engine remains outside this vendor directory.
Do not add GUI, JobManager, Tauri, or product-specific behavior to the vendored
codec and operation source.
