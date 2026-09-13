# Static Render binary vector

`nonempty-render.hex` is the 9,024-byte output of the independent test-only
`fcbc_render_reference_writer::write_nonempty_render` with the checked-in project
PNG, lossless WebP, TrueType font, and opaque negative-test asset. Its manifest
pins the decoded SHA-256, all 15 section offsets/lengths/CRCs, five resource
records and their original bytes/metadata, and the eight nonempty Render tables.
The writer test compares every byte; the independent loader and product loader
read the checked-in file without generating their input.

The scene contains each Core Node/Geometry kind, all seven Path commands, all
four Paint kinds, a Stroke, a Clip, and one GlyphRun. Node roots exercise
Constant, SegmentTrack, Piecewise, and Expression descriptors. The mutation
manifest contains absolute patches, including recomputed section CRCs for deep
mutations; its checksum-only case deliberately leaves the payload/CRC mismatch.
Both loaders and CLI `inspect --render --json` execute the declared cases.

The semantic oracle fixes two queries, at 0 and 0.25 seconds. The analytic Line
has identity transform and speed 2. Its shape/Image Note is at 0.5 seconds, and
its Text Note is at 2 seconds. Both presentation axes equal the Note distance,
so the shape attachment translates by `(1,1)` then `(0.5,0.5)`, while the Text
attachment translates by `(4,4)` then `(3.5,3.5)`. Node local matrices are identity.
RoundedRect opacity is the linear Track's value, 0 then 0.25; Circle opacity is
the constant-one Piecewise. Layer and full Node ancestry keys use the section-14
stable-ID algorithm. The JSON records typed IDs/kinds and raw binary64 bits for
matrices, bounds, paints, stroke, opacity, Image rectangles, and final glyph
positions, together with clip and isolation chains. Queries run forward and
backward against the same loaded chart.

The 12×12 raster oracle at time zero is derived from section 15's 8×8 sample
grid, without reading renderer output. The viewport is 12×12 in sRGB. Rect,
RoundedRect, Circle, Ellipse, Polyline, and Polygon have zero-area fills; Line
has zero width. Both Arc commands have equal start/end angles, so Path contains
only its connector and closing line and has zero-area fill. The remaining
coverage is:

- Image occupies `[1,2] × [1,2]`, clipped by the unit circle at `(1,1)`. Each
  covered sample reads one of the four fixed PNG texels using nearest sampling,
  with the image's top-down Y coordinate. Other samples remain transparent.
- The centered square glyph has local outline coordinates `[-500,500]²` and
  `unitsPerEm=1000`. Its size is 4, so after attachment it covers `[2,6]²` in
  opaque white. Its isolated Group applies `screen` to the completed offscreen
  buffer. The two covered regions do not overlap at any sample.

The vector pins the decoded RGBA8 length/SHA and section-15.5 tolerances. The
shaping input `A`, `simple-ltr-1`, glyph 1, and metric bits live in the vector,
outside FCBC. Glyph 0/range/face and source-text-tail mutations exercise the
binary boundary. This corpus complements the nondegenerate source raster and
geometry tests; its degenerate shapes do not prove general shape coverage.
