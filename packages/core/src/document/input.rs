#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedText(pub String);

impl OwnedText {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseTextWithEdits {
    pub base: OwnedText,
    pub edits: Vec<OwnedText>,
}

impl BaseTextWithEdits {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: OwnedText::new(base),
            edits: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteStream {
    text: OwnedText,
}

impl ByteStream {
    pub fn from_text(text: impl Into<String>) -> Self {
        Self {
            text: OwnedText::new(text),
        }
    }

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }
}
