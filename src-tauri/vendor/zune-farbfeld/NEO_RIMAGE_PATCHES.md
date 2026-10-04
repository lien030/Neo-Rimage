# neo-rimage vendor notes

Source: the published `zune-farbfeld 0.5.2` crate, unchanged except for the decoder fixes and tests listed below and two trailing-whitespace removals in README/encoder documentation.

- Archive: <https://static.crates.io/crates/zune-farbfeld/zune-farbfeld-0.5.2.crate>
- SHA-256: `46689bf6c90702cba18ef876cc0d29affa8cda58e82a1246fced3a820e0e4c3d`.
- Upstream: <https://github.com/etemesi254/zune-image/tree/a019244c4a8a4eb713e85fa77f9e18f841d6271b/crates/zune-farbfeld>. The published crate records this commit with `dirty: true`, so the archive above is the exact source baseline.
- License: MIT OR Apache-2.0 OR Zlib; all three original license files are retained.

Local changes in `src/decoder.rs`:

- Preserve the public byte count returned by `output_buffer_size`, but convert it to a `u16` sample count when checking, allocating, and filling pixel buffers.
- Use checked output sizing before allocation, rather than saturating dimension multiplication.
- Make repeated header reads idempotent, including a header read followed by full decoding.
- Cover RGBA16 endianness/alpha, exact and oversized buffers, undersized/truncated inputs, repeated header reads, and size overflow with unit tests.

Encoder behavior and the public API remain unchanged. The application selects this source through its root Cargo `[patch.crates-io]` entry; no native dependency or toolchain change is needed. Remove the patch when an upstream release containing these fixes is adopted and the same regressions pass.
