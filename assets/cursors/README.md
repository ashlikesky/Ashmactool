# Geometric cursor artwork

Original MIT-licensed paths on a 64 × 64 canvas. `geometric.json` and the SVGs record the selected flat design; the open-hand variant extends that family. The application draws equivalent typed path commands in `src/geometric_paths.rs` through CoreGraphics, with no SVG/JSON parsing at runtime. Keep the artwork and typed paths in sync when editing.

The renderer adapts black/white fills to the selected theme and gives thin symbols a contrast under-stroke for visibility on both backgrounds. The ordinary arrow is retained. Hotspots are defined alongside the typed paths and scaled with the selected logical size.

`scripts/render-cursor-minimal.swift` renders the design overview. `--preview-all DIRECTORY` exports actual runtime cursor frames; `scripts/render-cursor-sheet.swift` assembles those frames into the complete state sheet. Swift is only used by the optional development preview tools, not by the application.
