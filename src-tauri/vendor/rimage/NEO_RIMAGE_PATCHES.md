# neo-rimage vendor notes

Source: <https://github.com/vlad-salone/rimage>, tag `v0.14.0`.
Revision: `978a87075ad60fbbd033ba851036fbe84e9e0f47`.
License: MIT OR Apache-2.0 (see `LICENSE-MIT` and `LICENSE-APACHE`).

Local integration changes:

- `build.rs` emits the Windows CLI resource only with `build-binary`.
- `ravif` defaults are disabled to avoid implicitly enabling codec threading.
- TIFF enables Deflate, Fax, JPEG and LZW.
- AVIF exposes sequence-header and container NCLX inspection, bounded reads and a native pixel limit; dav1d uses one thread and one-frame delay.
- TIFF exposes header/page inspection and bounded decode buffers/pixels. Float32 layouts are sized as four bytes per sample.
- Packed 1/2/4-bit grayscale TIFF samples are expanded to 8-bit; 8-bit YCbCr uses full-range BT.601 RGB conversion so JPEG-compressed TIFF can reach existing encoders. Unsupported custom YCbCr coefficients/reference ranges are rejected.
- SVG exposes the shared font parser options and rendering of a caller-parsed tree.
- Newly enabled codec tests generate their own fixtures instead of referencing files absent from the vendor snapshot.

The application explicitly enables AVIF, TIFF, SVG and limits alongside its existing features. The upstream CLI and rimage `threads` feature remain disabled. Existing zune codec threading is not claimed to be disabled. `zune-core = 0.5.1` remains pinned for upstream compatibility.

Job scheduling, memory admission, file fingerprints, strict SVG resource resolution and product-specific input restrictions live in the application, not this vendor directory. Preparation never decodes pixels; execution uses the manager-admitted plan. No GUI, JobManager or Tauri policy belongs here.
