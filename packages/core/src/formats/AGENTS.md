# Formats Boundary

This directory owns decoding structured text into `TreeStore` and encoding that tree back into supported formats.

## Public Contracts

- Decode contract: `mod.rs` (`Decode`, `DecodedDocument`)
- Encode contract: `encoder.rs` (`Encode`)
- Language codecs: `decoder_*.rs`, `encoder_*.rs`
- Formatting preferences: `preferences.rs`, `smart_layout.rs`
- Registry-facing format metadata: `../registry/format.rs`, `../registry/format_registry.rs`

## Boundary Rules

- Decode into the canonical `TreeStore`; do not introduce format-private semantic trees.
- Keep syntax-specific parsing and printing in the matching codec file; keep cross-format behavior in shared helpers or preferences.
- Register a supported format through the registry path as well as its codec implementation.
- Preserve the `lite` feature boundary when adding language-specific codecs or encoders.
- Keep file I/O and UI formatting policy outside this directory.
