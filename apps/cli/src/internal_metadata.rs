pub fn cli_help_json() -> serde_json::Value {
    crate::spec::root_help_json_value()
}

pub fn operators_json() -> serde_json::Value {
    serde_json::to_value(crate::catalog::operators()).expect("operators should serialize")
}

pub fn formats_json() -> serde_json::Value {
    serde_json::to_value(crate::catalog::formats()).expect("formats should serialize")
}
