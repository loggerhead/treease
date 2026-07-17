---
summary: "Common YAML subset, fixture classification, and rare dataset boundaries."
read_when:
  - A task involves YAML support boundaries, fixture classification, or the rare dataset
  - Explaining why a category of YAML input is supported or restricted
---

# Common YAML Subset

This document defines the common YAML subset that Treease prioritizes in fixtures and implementation.

## Purpose

- Cover the YAML structures that users most commonly write by hand.
- Keep `test/fixtures/yaml/` limited to common-subset examples.
- Move specification-level, advanced, and edge-case syntax to `test/fixtures/yaml-rare/` so it does not continually pollute the standard regression suite.

## Common YAML Subset

The current common subset includes:

- Single-document input.
- Basic multi-document separation: `---`.
- block mapping: `key: value`.
- block sequence: `- item`.
- Standard nesting of block mappings and block sequences.
- plain scalar, single quoted scalar, double quoted scalar.
- Common block scalars: `|`, `>`.
- Empty documents and empty streams.
- Inline comments and standalone comment lines.

## Outside the Common Subset

The following syntax is not currently part of the standard YAML subset:

- `%YAML`, `%TAG`, and reserved directives.
- Explicit tags: `!!str`, `!!int`, `!!map`, `!!seq`, `!!set`, `!!omap`, `!!binary`, and others.
- Custom tags and tag shorthands.
- anchor / alias.
- Explicit mapping syntax: `? key` / `: value`.
- Multi-document combinations that depend on complex stream rules.
- Extreme empty-key, empty-value, and complex node-property combinations intended for specification coverage.

## Current Failure-Set Classification

The repository no longer maintains a centralized YAML failure-list file. Whether a YAML fixture belongs in the standard regression suite is determined directly by its directory:

- Keep in `test/fixtures/yaml/`: only examples that are still in the common subset and still need to be fixed.
- Move to `test/fixtures/yaml-rare/`: any example that depends on the advanced features above is classified as rare syntax.

The only failing examples still retained in the standard YAML directory are empty streams:

- `AVM7.1.yaml`
- `empty-stream.1.yaml`

All other currently failing YAML examples are classified as rare-syntax examples and moved to `test/fixtures/yaml-rare/`.
