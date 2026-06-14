use crate::operators::{
    registry::get_registered_format,
    registry_tables_formats::{Decoder as RegisteredDecoder, Encoder as RegisteredEncoder},
    *,
};

/// Check if a byte slice is a single line ending with a newline.
fn is_single_line_with_newline(s: &[u8]) -> bool {
    if s.is_empty() || s[s.len() - 1] != b'\n' {
        return false;
    }
    // Check for any newline before the last character
    !s[..s.len() - 1].contains(&b'\n')
}

/// Remove trailing newline characters from a byte slice.
fn chomp_trailing_newlines(s: &mut Vec<u8>) {
    while s.last() == Some(&b'\n') {
        s.pop();
    }
}

fn get_encoder(ctx: &Context, format: &str, indent: i32) -> Option<Box<dyn RegisteredEncoder>> {
    let entry = get_registered_format(format)?;
    let factory = entry.encoder_factory?;
    factory(ctx, indent).ok()
}

fn get_decoder(ctx: &Context, format: &str) -> Option<Box<dyn RegisteredDecoder>> {
    let entry = get_registered_format(format)?;
    let factory = entry.decoder_factory?;
    factory(ctx).ok()
}

pub(crate) fn encode_node_to_string(
    ctx: &Context,
    candidate: &TreeNode,
    format: &str,
    indent: i32,
) -> Result<String, CoreError> {
    let mut encoder =
        get_encoder(ctx, format, indent).ok_or(CoreError::Format(FormatError::UnknownFormat))?;
    let output = encoder.encode_to_string(candidate)?;
    encoder.deinit();
    Ok(output)
}

// ── Preference extraction ────────────────────────────────────────

fn get_encoder_prefs(pref: Option<&OperationPreference>) -> EncoderPreferences {
    match pref {
        Some(OperationPreference::Encoder(p)) => p.clone(),
        _ => EncoderPreferences::default(),
    }
}

fn get_decoder_prefs(pref: Option<&OperationPreference>) -> DecoderPreferences {
    match pref {
        Some(OperationPreference::Decoder(p)) => p.clone(),
        _ => DecoderPreferences::default(),
    }
}

// ── encodeWithOriginalAdjust ─────────────────────────────────────

fn encode_with_original_adjust(
    results: &mut Vec<TreeNode>,
    candidate: &TreeNode,
    prefs: &EncoderPreferences,
    out0: &str,
    original: Option<&str>,
) -> Result<(), CoreError> {
    let mut out = out0.as_bytes().to_vec();

    if let Some(orig) = original {
        if is_single_line_with_newline(&out) && !is_single_line_with_newline(orig.as_bytes()) {
            chomp_trailing_newlines(&mut out);
        }
    }

    if (prefs.format == "json" && prefs.indent == 0) || prefs.format == "csv" {
        chomp_trailing_newlines(&mut out);
    }

    let out_str =
        String::from_utf8(out).map_err(|_| CoreError::Format(FormatError::UnknownFormat))?;
    let result =
        *candidate.create_replacement(NodeKind::Scalar, SemType::Str.to_string(), &out_str)?;
    results.push(result);
    Ok(())
}

/// Encode tree nodes to string format (encode operator).
pub fn op_encode(
    ctx: Context,
    _d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let prefs = get_encoder_prefs(expression_node.operation.preferences.as_deref());

    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let out = match encode_node_to_string(&ctx, candidate, &prefs.format, prefs.indent) {
            Ok(s) => s,
            Err(e) => {
                if matches!(e, CoreError::Format(FormatError::UnknownFormat)) {
                    if let Some(ref d) = ctx.diagnostics {
                        d.set_message("eval", "no support for output format")?;
                    }
                }
                return Err(e);
            }
        };

        // Look up original bytes from codec_state (set by op_decode).
        let original = ctx
            .codec_state
            .as_ref()
            .and_then(|cs| cs.original_for(candidate));
        encode_with_original_adjust(&mut results, candidate, &prefs, &out, original)?;
    }
    ctx.child_context(results)
}

/// Decode string values back to tree nodes (decode operator).
pub fn op_decode(
    context0: Context,
    _d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut ctx = context0;
    let prefs = get_decoder_prefs(expression_node.operation.preferences.as_deref());

    let Some(mut decoder) = get_decoder(&ctx, &prefs.format) else {
        if let Some(ref d) = ctx.diagnostics {
            d.set_message("eval", "no support for input format")?;
        }
        return Err(CoreError::Format(FormatError::UnknownFormat));
    };

    // Ensure codec state exists so we can remember original bytes.
    ctx.ensure_codec_state();

    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let mut node = *decoder.decode_string(&candidate.value)?;
        node.key = candidate.key.clone();
        node.parent = candidate.parent.clone();
        node.sequence_index = candidate.sequence_index;

        // Remember the original text so op_encode can decide
        if let Some(ref mut state) = ctx.codec_state {
            state.remember_original(&node, &candidate.value);
        }

        results.push(node);
    }
    decoder.deinit();
    ctx.child_context(results)
}
