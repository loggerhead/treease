/**
 * Serde-compatible plain-TS types replacing the @thi.ng/wasm-api C-ABI types.
 *
 * All values here match the serde-deserialized output from wasm-bindgen
 * functions. No C memory layout, no WasmStringSlice — just plain objects.
 *
 * Enum values MUST match the Rust #[repr(i32)] / #[repr(u8)] definitions:
 *   - TreeKind, SemType, GraphKind → wasm_types.rs #[repr(i32)]
 *   - PathSegTag → wasm_types.rs #[repr(u8)]
 */

/** Tree structure kind. Matches `wasm_types::TreeKind` (repr i32). */
export enum TreeKind {
	SEQUENCE = 0,
	MAPPING = 1,
	SCALAR = 2,
	ALIAS = 3,
	UNKNOWN = 4,
}

/** Semantic type of a leaf node. Matches `wasm_types::SemType` (repr i32). */
export enum SemType {
	MAP = 0,
	SEQ = 1,
	STR = 2,
	INT = 3,
	FLOAT = 4,
	BOOLEAN = 5,
	NIL = 6,
	UNKNOWN = 255,
}

/** Graph element kind. Matches `wasm_types::GraphKind` (repr i32). */
export enum GraphKind {
	SCALAR = 0,
	OBJECT = 1,
	TABLE = 2,
}

/** Path segment discriminator. Matches `wasm_types::PathSegTag` (repr u8). */
export enum PathSegTag {
	KEY = 0,
	INDEX = 1,
}

/** A single segment of a tree path. */
export interface PathSeg {
	/** @see PathSegTag */
	tag: number;
	/** Key value for KEY segments; empty string for INDEX segments. */
	key: string;
	/** Index for INDEX segments; 0 for KEY segments. */
	index: number;
}

/** Byte-span + line/column for a node range in the source text. */
export interface PathSpan {
	startByte: number;
	endByte: number;
	row: number;
	column: number;
}

/** A tree node — plain object, no WasmStringSlice. */
export interface TreeNode {
	kind: number;
	semType: number;
	/** Tree-sitter node type label, e.g. "block_mapping_pair". */
	tag: string;
	/** Scalar / key / alias value as string. */
	value: string;
	children: TreeNode[];
}
