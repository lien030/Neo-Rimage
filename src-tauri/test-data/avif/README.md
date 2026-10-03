# AVIF rejection fixtures

Unmodified test images from AOMediaCodec/libavif, commit `e4db889c6b574bf221a380e9bddde0273c942023`, directory `tests/data`.
Upstream documents both files as licensed under the libavif license reproduced in LICENSE.

- `colors-animated-8bpc.avif`: generated colour animation; tests rejection before decoded frame allocation.
- `sofa_grid1x5_420.avif`: personal photo encoded as a 1×5 grid; tests unsupported grid rejection.

SHA-256:

```text
2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d  colors-animated-8bpc.avif
c9e04ff9d90d7093454750fa33b7543ee5479e0cfb151e2c3d2ce6a16c1651c1  sofa_grid1x5_420.avif
```

Source and attribution: https://github.com/AOMediaCodec/libavif/blob/e4db889c6b574bf221a380e9bddde0273c942023/tests/data/README.md

These files are test data only and are not packaged in the application.
