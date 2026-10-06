# Toolbox 0.4.2

A focused follow-up to 0.4.1: top-k selection now breaks same-file/same-line/name ties by the UTF-8 byte offset and kind. A JavaScript fixture with repeated declarations reproduced the incorrect cutoff (offset 64 instead of 0). The regression now requires the first declaration across independently seeded indexes. The filtering-before-cloning optimization and external-edit refresh remain intact.

Validation: cargo fmt, Clippy with warnings denied, complete locked workspace tests.
