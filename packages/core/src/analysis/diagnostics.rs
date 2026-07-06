use std::fmt;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DiagnosticStage {
    Decode,
    #[default]
    ParseExpression,
    Eval,
    Encode,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticLocation {
    pub filename: String,
    pub byte_offset: Option<usize>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticSnippet {
    pub line_text: String,
    pub caret_start: Option<usize>,
    pub caret_end: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParseErrorInfo {
    pub op_id: Option<u16>,
    pub expected_args: Option<u32>,
    pub actual_args: Option<u32>,
    pub token_start: Option<usize>,
    pub token_end: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diagnostics {
    messages: Vec<String>,
    pub stage: Option<DiagnosticStage>,
    pub message: String,
    pub location: DiagnosticLocation,
    pub snippet: DiagnosticSnippet,
    pub parse_info: ParseErrorInfo,
}

impl Diagnostics {
    pub fn push(&mut self, message: impl Into<String>) {
        self.messages.push(message.into());
    }

    pub fn messages(&self) -> &[String] {
        &self.messages
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn clear(&mut self) {
        self.stage = None;
        self.message.clear();
        self.location = DiagnosticLocation::default();
        self.snippet = DiagnosticSnippet::default();
        self.parse_info = ParseErrorInfo::default();
    }

    pub fn set_parse_info(&mut self, info: ParseErrorInfo) {
        self.parse_info = info;
    }

    pub fn set_message(&mut self, stage: DiagnosticStage, message: impl Into<String>) {
        let parse_info = self.parse_info.clone();
        self.clear();
        self.parse_info = parse_info;
        self.stage = Some(stage);
        self.message = message.into();
    }

    pub fn set_messagef(
        &mut self,
        stage: DiagnosticStage,
        template: &str,
        args: fmt::Arguments<'_>,
    ) {
        let rendered_args = args.to_string();
        let message = if template.contains("{}") {
            template.replacen("{}", &rendered_args, 1)
        } else {
            format!("{template}{rendered_args}")
        };
        self.set_message(stage, message);
    }

    pub fn set_parse_error(&mut self, info: ParseErrorInfo, message: impl Into<String>) {
        self.set_parse_info(info);
        self.set_message(DiagnosticStage::ParseExpression, message);
    }

    pub fn set_parse_errorf(&mut self, info: ParseErrorInfo, message: impl Into<String>) {
        self.set_parse_info(info);
        self.set_message(DiagnosticStage::ParseExpression, message);
    }

    pub fn set_location_from_offset(
        &mut self,
        filename: &str,
        source: &str,
        absolute_offset: usize,
    ) {
        let (location, snippet) = compute_location_and_snippet(filename, source, absolute_offset);
        self.location = location;
        self.snippet = snippet;
    }

    pub fn set_location_from_offset_in_slice(
        &mut self,
        filename: &str,
        full_source: &str,
        slice_start: usize,
        slice_offset: usize,
    ) {
        self.set_location_from_offset(
            filename,
            full_source,
            slice_start.saturating_add(slice_offset),
        );
    }

    pub fn set_filename_if_empty(&mut self, filename: &str) {
        if self.location.filename.is_empty() {
            self.location.filename = filename.to_owned();
        }
    }
}

pub fn compute_location_and_snippet(
    filename: &str,
    source: &str,
    absolute_offset: usize,
) -> (DiagnosticLocation, DiagnosticSnippet) {
    let clamped = absolute_offset.min(source.len());
    let mut line = 1usize;
    let mut line_start = 0usize;
    for (idx, byte) in source.as_bytes().iter().enumerate().take(clamped) {
        if *byte == b'\n' {
            line += 1;
            line_start = idx + 1;
        }
    }
    let line_end = source[line_start..]
        .find('\n')
        .map(|rel| line_start + rel)
        .unwrap_or(source.len());
    let column = (clamped - line_start) + 1;

    (
        DiagnosticLocation {
            filename: filename.to_owned(),
            byte_offset: Some(absolute_offset),
            line: Some(line),
            column: Some(column),
        },
        DiagnosticSnippet {
            line_text: source[line_start..line_end].to_owned(),
            caret_start: Some(column),
            caret_end: Some(column),
        },
    )
}
