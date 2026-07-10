# neo-rimage vendor notes

Source: <https://github.com/SalOne22/rimage>

- Version: `0.12.4`
- Revision: `0078ef746d75fef5db137018b6bf5b6eb7d715e5`
- License: MIT OR Apache-2.0 (see `LICENSE-MIT` and `LICENSE-APACHE`)

The vendored source is unchanged except for `build.rs`. The Windows VERSION
resource is now generated only when rimage's `build-binary` feature is enabled.
Library consumers must not embed the CLI resource because it conflicts with the
Tauri application's own Windows resource during final linking.

The neo-rimage local orchestration engine remains outside this vendor directory.
Do not add GUI, JobManager, Tauri, or product-specific behavior to the vendored
codec and operation source.
