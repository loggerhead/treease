#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraversePreferences {
    pub dont_follow_alias: bool,
    pub include_map_keys: bool,
    pub dont_auto_create: bool,
    pub dont_include_map_values: bool,
    pub optional_traverse: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlattenPreferences {
    pub depth: i32,
}

impl Default for FlattenPreferences {
    fn default() -> Self {
        Self { depth: -1 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveDescentPreferences {
    pub traverse_preferences: TraversePreferences,
    pub recurse_array: bool,
}

impl Default for RecursiveDescentPreferences {
    fn default() -> Self {
        Self {
            traverse_preferences: TraversePreferences::default(),
            recurse_array: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExpressionOpPreferences {
    pub expression: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderPreferences {
    pub format: String,
    pub indent: i32,
}

impl Default for EncoderPreferences {
    fn default() -> Self {
        Self {
            format: "yaml".to_string(),
            indent: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderPreferences {
    pub format: String,
}

impl Default for DecoderPreferences {
    fn default() -> Self {
        Self {
            format: "yaml".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssignPreferences {
    pub dont_overwrite_anchor: bool,
    pub only_write_null: bool,
    pub clobber_custom_tags: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssignVarPreferences {
    pub is_reference: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentOpPreferences {
    pub level: i32,
}

impl Default for ParentOpPreferences {
    fn default() -> Self {
        Self { level: 1 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelationalPref {
    pub or_equal: bool,
    pub greater: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeCasePrefs {
    pub to_upper_case: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationPreferences {
    Traverse(TraversePreferences),
    Flatten(FlattenPreferences),
    RecursiveDescent(RecursiveDescentPreferences),
    Expression(ExpressionOpPreferences),
    Encoder(EncoderPreferences),
    Decoder(DecoderPreferences),
    Assign(AssignPreferences),
    AssignVar(AssignVarPreferences),
    Parent(ParentOpPreferences),
    Relational(RelationalPref),
    ChangeCase(ChangeCasePrefs),
}

macro_rules! get_preference {
    ($fn_name:ident, $variant:ident, $ty:ty) => {
        pub fn $fn_name(pref: Option<&OperationPreferences>) -> $ty {
            match pref {
                Some(OperationPreferences::$variant(value)) => value.clone(),
                _ => <$ty>::default(),
            }
        }
    };
}

get_preference!(get_traverse_preference, Traverse, TraversePreferences);
get_preference!(get_flatten_preference, Flatten, FlattenPreferences);
get_preference!(
    get_recursive_descent_preference,
    RecursiveDescent,
    RecursiveDescentPreferences
);
get_preference!(
    get_expression_preference,
    Expression,
    ExpressionOpPreferences
);
get_preference!(get_encoder_preference, Encoder, EncoderPreferences);
get_preference!(get_decoder_preference, Decoder, DecoderPreferences);
get_preference!(get_assign_preference, Assign, AssignPreferences);
get_preference!(get_assign_var_preference, AssignVar, AssignVarPreferences);
get_preference!(get_parent_preference, Parent, ParentOpPreferences);
get_preference!(get_relational_preference, Relational, RelationalPref);
get_preference!(get_change_case_preference, ChangeCase, ChangeCasePrefs);
