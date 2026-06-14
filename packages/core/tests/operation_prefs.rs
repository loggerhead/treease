use treease_core::core::{
    AssignPreferences, ChangeCasePrefs, DecoderPreferences, EncoderPreferences, FlattenPreferences,
    OperationPreferences, ParentOpPreferences, RecursiveDescentPreferences, TraversePreferences,
    get_change_case_preference, get_encoder_preference, get_flatten_preference,
    get_parent_preference, get_recursive_descent_preference, get_traverse_preference,
};

#[test]
fn operation_prefs_return_defaults_for_none_or_mismatched_variant() {
    let flatten = get_flatten_preference(None);
    let traverse =
        get_traverse_preference(Some(&OperationPreferences::Parent(ParentOpPreferences {
            level: 3,
        })));

    assert_eq!(flatten, FlattenPreferences { depth: -1 });
    assert_eq!(traverse, TraversePreferences::default());
}

#[test]
fn operation_prefs_return_matching_variant_value() {
    let pref = OperationPreferences::RecursiveDescent(RecursiveDescentPreferences {
        traverse_preferences: TraversePreferences {
            include_map_keys: true,
            ..TraversePreferences::default()
        },
        recurse_array: false,
    });

    let value = get_recursive_descent_preference(Some(&pref));

    assert!(!value.recurse_array);
    assert!(value.traverse_preferences.include_map_keys);
}

#[test]
fn operation_prefs_keep_encoder_and_change_case_defaults() {
    let encoder =
        get_encoder_preference(Some(&OperationPreferences::Encoder(EncoderPreferences {
            format: "json".to_string(),
            indent: 4,
        })));
    let decoder = DecoderPreferences::default();
    let change_case =
        get_change_case_preference(Some(&OperationPreferences::ChangeCase(ChangeCasePrefs {
            to_upper_case: true,
        })));

    assert_eq!(encoder.format, "json");
    assert_eq!(encoder.indent, 4);
    assert_eq!(decoder.format, "yaml");
    assert!(change_case.to_upper_case);
}

#[test]
fn operation_prefs_expose_assign_and_parent_defaults() {
    let assign = AssignPreferences::default();
    let parent = get_parent_preference(None);

    assert!(!assign.dont_overwrite_anchor);
    assert_eq!(parent.level, 1);
}
