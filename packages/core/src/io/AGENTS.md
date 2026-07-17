# IO Boundary

This directory owns codec selection, reader/writer adapters, and generic printing abstractions around Core trees and values.

## Public Contracts

- Codec selection: `codec_service.rs`
- Reader, writer, decoder, and encoder abstractions: `encoding.rs`
- In-memory adapters: `io_adapters.rs`
- Generic printers: `printer.rs`, `printer_writer.rs`
- Literal formatting helpers: `literal_format.rs`

## Boundary Rules

- Keep format syntax in `../formats/`; this module selects codecs and provides transport-neutral interfaces.
- Preserve canonical format-name normalization in `codec_service.rs` rather than duplicating aliases in callers.
- Do not add filesystem, browser, or CLI transport policy here.
