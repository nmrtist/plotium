# Synthetic dense 3D Bruker fixture

This fixture contains artificial integer signals, not experimental acquisition
data. Its logical shape is 5 × 3 × 4, with QF indirect dimensions and a complex
direct dimension. `ser` stores little-endian i32 real/imaginary pairs without
block padding; the group delay is zero.

The unequal dimensions exercise axis ordering, plane bounds, signed transforms,
complex retention and phase adjustment. Processing tests independently decode
`ser` and evaluate direct DFT/cosine sums for all 60 logical points. Core, desktop,
CLI and MCP tests reuse the same fixture for navigation and project roundtrips.
