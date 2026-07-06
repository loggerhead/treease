use crate::wasm_types::PathSeg;

use crate::language::SemType;
use crate::tree::tree_node::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GraphKind {
    Scalar,
    Object,
    Table,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CellBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct BoxArgs {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub corner_radius: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

impl Default for TextAlign {
    fn default() -> Self {
        Self::Left
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextVerticalAlign {
    Top,
    Middle,
    Bottom,
}

impl Default for TextVerticalAlign {
    fn default() -> Self {
        Self::Middle
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct TextArgs<'a> {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub text: &'a str,
    pub text_align: TextAlign,
    pub text_vertical_align: TextVerticalAlign,
    pub editable: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct BezierArgs {
    pub from_x: i32,
    pub from_y: i32,
    pub c1x: i32,
    pub c1y: i32,
    pub c2x: i32,
    pub c2y: i32,
    pub to_x: i32,
    pub to_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphCell<'a> {
    pub text: &'a str,
    pub sem_type: Option<SemType>,
    pub path: &'a [PathSeg<'a>],
    pub value: &'a str,
    pub editable: bool,
    pub bounds: CellBounds,
    pub text_bounds: CellBounds,
    pub box_args: BoxArgs,
    pub text_args: TextArgs<'a>,
    pub source: Option<NodeId>,
    pub format_text: &'a str,
}

impl<'a> Default for GraphCell<'a> {
    fn default() -> Self {
        Self {
            text: "",
            sem_type: None,
            path: &[],
            value: "",
            editable: false,
            bounds: CellBounds::default(),
            text_bounds: CellBounds::default(),
            box_args: BoxArgs::default(),
            text_args: TextArgs::default(),
            source: None,
            format_text: "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphRow<'a> {
    pub index: i32,
    pub bounds: CellBounds,
    pub abs_bounds: CellBounds,
    pub cell_bounds: CellBounds,
    pub box_args: BoxArgs,
    pub cell_box_args: BoxArgs,
    pub cells: &'a [GraphCell<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphTable<'a> {
    pub columns: &'a [GraphCell<'a>],
    pub column_widths: &'a [i32],
    pub rows: &'a [GraphRow<'a>],
    pub width: i32,
    pub total_height: i32,
    pub view_height: i32,
    pub header_height: i32,
    pub row_height: i32,
    pub key: &'a str,
    pub count: i32,
    pub source: Option<NodeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphNodeKey<'a> {
    pub kind: GraphKind,
    pub path: &'a [PathSeg<'a>],
    pub path_key: &'a str,
    pub stable_id: u64,
    pub stable_id_text: &'a str,
}

impl<'a> Default for GraphNodeKey<'a> {
    fn default() -> Self {
        Self {
            kind: GraphKind::Scalar,
            path: &[],
            path_key: "",
            stable_id: 0,
            stable_id_text: "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphNode<'a> {
    pub render_handle: u32,
    pub stable_id: u64,
    pub key: GraphNodeKey<'a>,
    pub kind: GraphKind,
    pub depth: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub box_args: BoxArgs,
    pub path: &'a [PathSeg<'a>],
    pub meta: GraphCell<'a>,
    pub rows: &'a [GraphRow<'a>],
    pub table: Option<GraphTable<'a>>,
    pub source: NodeId,
    pub preorder_first: u32,
    pub preorder_last: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphEdge {
    pub from_render_handle: u32,
    pub from_row: i32,
    pub to_render_handle: u32,
    pub to_row: i32,
    pub bezier_args: BezierArgs,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphModel<'a> {
    pub nodes: Vec<GraphNode<'a>>,
    pub edges: Vec<GraphEdge>,
}
