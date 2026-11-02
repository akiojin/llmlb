//! Bounded token metadata observation without retaining provider payload strings.

use crate::common::usage::UsageTokens;

const MAX_DEPTH: usize = 64;
const MAX_KEY_BYTES: usize = 128;
const MAX_NUMBER_BYTES: usize = 32;

/// Only an entire usage object is a checkpoint. A truncated enclosing response
/// does not erase an earlier checkpoint, while an unfinished usage never wins.
#[derive(Clone, Copy, Default)]
struct Fields {
    input: Option<Option<u64>>,
    output: Option<Option<u64>>,
    total: Option<Option<u64>>,
}

#[derive(Clone, Copy, PartialEq)]
enum Key {
    Usage,
    Response,
    Message,
    Input,
    Output,
    Total,
    Error,
    Type,
    Other,
}

impl Key {
    fn parse(raw: &[u8]) -> Self {
        match serde_json::from_slice::<String>(raw).as_deref() {
            Ok("usage") => Self::Usage,
            Ok("response") => Self::Response,
            Ok("message") => Self::Message,
            Ok("input_tokens" | "prompt_tokens") => Self::Input,
            Ok("output_tokens" | "completion_tokens") => Self::Output,
            Ok("total_tokens") => Self::Total,
            Ok("error") => Self::Error,
            Ok("type") => Self::Type,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Root,
    Response,
    Message,
    Usage,
    Error,
    Other,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    KeyOrEnd,
    Key,
    Colon,
    ValueOrEnd,
    Value,
    CommaOrEnd,
}

struct Frame {
    object: bool,
    role: Role,
    phase: Phase,
    key: Key,
    fields: Fields,
    semantic_error: bool,
}

enum Lexeme {
    Idle,
    String {
        key: bool,
        semantic_type: bool,
        raw: Vec<u8>,
        escaped: bool,
        unicode_left: u8,
        too_long: bool,
    },
    Scalar(Vec<u8>, bool, Option<NumberState>),
}

/// Numeric grammar remains bounded even when a provider emits millions of digits.
#[derive(Clone, Copy)]
enum NumberState {
    Minus,
    Zero,
    Integer,
    Dot,
    Fraction,
    ExponentStart,
    ExponentSign,
    Exponent,
    Invalid,
}

impl NumberState {
    fn first(byte: u8) -> Option<Self> {
        match byte {
            b'-' => Some(Self::Minus),
            b'0' => Some(Self::Zero),
            b'1'..=b'9' => Some(Self::Integer),
            _ => None,
        }
    }

    fn push(self, byte: u8) -> Self {
        match (self, byte) {
            (Self::Minus, b'0') => Self::Zero,
            (Self::Minus, b'1'..=b'9') | (Self::Integer, b'0'..=b'9') => Self::Integer,
            (Self::Zero | Self::Integer, b'.') => Self::Dot,
            (Self::Dot | Self::Fraction, b'0'..=b'9') => Self::Fraction,
            (Self::Zero | Self::Integer | Self::Fraction, b'e' | b'E') => Self::ExponentStart,
            (Self::ExponentStart, b'+' | b'-') => Self::ExponentSign,
            (Self::ExponentStart | Self::ExponentSign | Self::Exponent, b'0'..=b'9') => {
                Self::Exponent
            }
            _ => Self::Invalid,
        }
    }

    fn complete(self) -> bool {
        matches!(
            self,
            Self::Zero | Self::Integer | Self::Fraction | Self::Exponent
        )
    }
}

struct JsonMetadata {
    stack: Vec<Frame>,
    lexeme: Lexeme,
    started: bool,
    complete: bool,
    invalid: bool,
    observation: Option<Fields>,
    semantic_failed: bool,
}

impl Default for JsonMetadata {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            lexeme: Lexeme::Idle,
            started: false,
            complete: false,
            invalid: false,
            observation: None,
            semantic_failed: false,
        }
    }
}

impl JsonMetadata {
    fn reset_event(&mut self) {
        self.stack.clear();
        self.lexeme = Lexeme::Idle;
        self.started = false;
        self.complete = false;
        self.invalid = false;
    }

    fn wants_key(&self) -> bool {
        self.stack.last().is_some_and(|frame| {
            frame.object && matches!(frame.phase, Phase::Key | Phase::KeyOrEnd)
        })
    }

    fn wants_value(&self) -> bool {
        self.stack.last().map_or(!self.started, |frame| {
            matches!(frame.phase, Phase::Value | Phase::ValueOrEnd)
        })
    }

    fn scalar(&mut self, value: Option<u64>) {
        if !self.wants_value() {
            self.invalid = true;
            return;
        }
        self.started = true;
        if let Some(frame) = self.stack.last_mut() {
            if frame.role == Role::Usage {
                match frame.key {
                    Key::Input => frame.fields.input = Some(value),
                    Key::Output => frame.fields.output = Some(value),
                    Key::Total => frame.fields.total = Some(value),
                    _ => {}
                }
            }
            frame.phase = Phase::CommaOrEnd;
            frame.key = Key::Other;
        } else {
            self.complete = true;
        }
    }

    fn begin(&mut self, object: bool) {
        if !self.wants_value() || self.stack.len() == MAX_DEPTH {
            self.invalid = true;
            return;
        }
        let role = if !object {
            Role::Other
        } else if let Some(frame) = self.stack.last() {
            match (frame.role, frame.key) {
                (Role::Root | Role::Response | Role::Message, Key::Usage) => Role::Usage,
                (Role::Root, Key::Response) => Role::Response,
                (Role::Root, Key::Message) => Role::Message,
                (Role::Root | Role::Response, Key::Error) => Role::Error,
                _ => Role::Other,
            }
        } else {
            Role::Root
        };
        // A container cannot itself be a numeric token count. Mark the parent
        // value complete now, but never checkpoint an unfinished child object.
        self.scalar(None);
        self.complete = false;
        self.stack.push(Frame {
            object,
            role,
            phase: if object {
                Phase::KeyOrEnd
            } else {
                Phase::ValueOrEnd
            },
            key: Key::Other,
            fields: Fields::default(),
            semantic_error: false,
        });
    }

    fn end(&mut self, object: bool) {
        let Some(frame) = self.stack.pop() else {
            self.invalid = true;
            return;
        };
        let valid_phase = frame.phase == Phase::CommaOrEnd
            || (frame.object && frame.phase == Phase::KeyOrEnd)
            || (!frame.object && frame.phase == Phase::ValueOrEnd);
        if frame.object != object || !valid_phase {
            self.invalid = true;
            return;
        }
        if frame.role == Role::Usage {
            let observation = self.observation.get_or_insert_with(Fields::default);
            if frame.fields.input.is_some() {
                observation.input = frame.fields.input;
            }
            if frame.fields.output.is_some() {
                observation.output = frame.fields.output;
            }
            if frame.fields.total.is_some() {
                observation.total = frame.fields.total;
            }
        }
        if frame.role == Role::Error || frame.semantic_error {
            // A completed structured error stays a failure through later SSE
            // message_stop/completed events. Nested business payloads are Other.
            self.semantic_failed = true;
        }
        self.complete = self.stack.is_empty();
    }

    fn string(&mut self, key: bool, semantic_type: bool, raw: &[u8], too_long: bool) {
        if key {
            if !self.wants_key() {
                self.invalid = true;
                return;
            }
            let frame = self.stack.last_mut().expect("key requires an object");
            frame.key = if too_long {
                Key::Other
            } else {
                Key::parse(raw)
            };
            frame.phase = Phase::Colon;
        } else {
            let semantic_error = semantic_type
                && !too_long
                && matches!(
                    serde_json::from_slice::<String>(raw).as_deref(),
                    Ok("error" | "response.failed")
                );
            if let Some(frame) = self.stack.last_mut().filter(|_| semantic_error) {
                frame.semantic_error = true;
            }
            self.scalar(None);
        }
    }

    fn finish_scalar(&mut self, bytes: &[u8], too_long: bool, syntax: Option<NumberState>) {
        if too_long {
            // Unbounded numeric literals can never be a supported integer.
            if syntax.is_some_and(NumberState::complete) {
                self.scalar(None);
            } else {
                self.invalid = true;
            }
            return;
        }
        match serde_json::from_slice::<serde_json::Value>(bytes) {
            Ok(value) if !value.is_string() && !value.is_array() && !value.is_object() => {
                let tokens = if bytes.iter().all(u8::is_ascii_digit) {
                    value.as_u64().filter(|value| *value <= i64::MAX as u64)
                } else {
                    None
                };
                self.scalar(tokens);
            }
            _ => self.invalid = true,
        }
    }

    fn push(&mut self, byte: u8) {
        if self.invalid {
            return;
        }
        match &mut self.lexeme {
            Lexeme::String {
                key,
                semantic_type,
                raw,
                escaped,
                unicode_left,
                too_long,
            } => {
                if *key || *semantic_type {
                    if raw.len() < MAX_KEY_BYTES {
                        raw.push(byte);
                    } else {
                        *too_long = true;
                    }
                }
                if *unicode_left > 0 {
                    if !byte.is_ascii_hexdigit() {
                        self.invalid = true;
                    }
                    *unicode_left -= 1;
                    return;
                }
                if *escaped {
                    *escaped = false;
                    if byte == b'u' {
                        *unicode_left = 4;
                    } else if !matches!(
                        byte,
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't'
                    ) {
                        self.invalid = true;
                    }
                    return;
                }
                if byte == b'\\' {
                    *escaped = true;
                } else if byte == b'"' {
                    let key = *key;
                    let semantic_type = *semantic_type;
                    let too_long = *too_long;
                    let raw = std::mem::take(raw);
                    self.lexeme = Lexeme::Idle;
                    self.string(key, semantic_type, &raw, too_long);
                } else if byte < 0x20 {
                    self.invalid = true;
                }
                return;
            }
            Lexeme::Scalar(bytes, too_long, syntax) => {
                if matches!(byte, b',' | b'}' | b']' | b' ' | b'\r' | b'\n' | b'\t') {
                    let bytes = std::mem::take(bytes);
                    let too_long = *too_long;
                    let syntax = *syntax;
                    self.lexeme = Lexeme::Idle;
                    self.finish_scalar(&bytes, too_long, syntax);
                    if self.invalid {
                        return;
                    }
                } else {
                    if let Some(number) = syntax {
                        *number = number.push(byte);
                    }
                    if bytes.len() < MAX_NUMBER_BYTES {
                        bytes.push(byte);
                    } else {
                        *too_long = true;
                    }
                    return;
                }
            }
            Lexeme::Idle => {}
        }
        if byte.is_ascii_whitespace() {
            return;
        }
        if self.complete {
            self.invalid = true;
            return;
        }
        match byte {
            b'{' => self.begin(true),
            b'[' => self.begin(false),
            b'}' => self.end(true),
            b']' => self.end(false),
            b'"' if self.wants_key() || self.wants_value() => {
                let key = self.wants_key();
                let semantic_type = !key
                    && self
                        .stack
                        .last()
                        .is_some_and(|frame| frame.role == Role::Root && frame.key == Key::Type);
                self.lexeme = Lexeme::String {
                    key,
                    semantic_type,
                    raw: if key || semantic_type {
                        vec![b'"']
                    } else {
                        Vec::new()
                    },
                    escaped: false,
                    unicode_left: 0,
                    too_long: false,
                };
            }
            b':' => {
                if let Some(frame) = self
                    .stack
                    .last_mut()
                    .filter(|frame| frame.phase == Phase::Colon)
                {
                    frame.phase = Phase::Value;
                } else {
                    self.invalid = true;
                }
            }
            b',' => {
                if let Some(frame) = self
                    .stack
                    .last_mut()
                    .filter(|frame| frame.phase == Phase::CommaOrEnd)
                {
                    frame.phase = if frame.object {
                        Phase::Key
                    } else {
                        Phase::Value
                    };
                } else {
                    self.invalid = true;
                }
            }
            b'-' | b'0'..=b'9' | b't' | b'f' | b'n' if self.wants_value() => {
                self.lexeme = Lexeme::Scalar(vec![byte], false, NumberState::first(byte));
            }
            _ => self.invalid = true,
        }
    }
}

enum Framing {
    Json,
    Sse {
        prefix: Vec<u8>,
        data: bool,
        ignore: bool,
        first: bool,
    },
    Binary,
}

/// Streaming projection of a bounded set of provider metadata keys. Payload
/// strings are visited byte by byte without storing base64, audio, or text.
pub(super) struct MetadataScanner {
    framing: Framing,
    json: JsonMetadata,
}

impl MetadataScanner {
    pub(super) fn new(content_type: &str) -> Self {
        let content_type = content_type.to_ascii_lowercase();
        let framing =
            if content_type.starts_with("text/event-stream") {
                Framing::Sse {
                    prefix: Vec::new(),
                    data: false,
                    ignore: false,
                    first: false,
                }
            } else if content_type.split(';').next().is_some_and(|mime| {
                mime.trim() == "application/json" || mime.trim().ends_with("+json")
            }) {
                Framing::Json
            } else {
                Framing::Binary
            };
        Self {
            framing,
            json: JsonMetadata::default(),
        }
    }

    pub(super) fn textual(&self) -> bool {
        !matches!(self.framing, Framing::Binary)
    }

    pub(super) fn push(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            match &mut self.framing {
                Framing::Json => self.json.push(byte),
                Framing::Binary => return,
                Framing::Sse {
                    prefix,
                    data,
                    ignore,
                    first,
                } => {
                    if byte == b'\n' {
                        if *data {
                            self.json.push(b'\n');
                        } else if prefix.is_empty() && !*ignore {
                            self.json.reset_event();
                        }
                        prefix.clear();
                        *data = false;
                        *ignore = false;
                        *first = false;
                    } else if *data {
                        if !*first || byte != b' ' {
                            self.json.push(byte);
                        }
                        *first = false;
                    } else if byte != b'\r' && !*ignore {
                        prefix.push(byte);
                        if !b"data:".starts_with(prefix) {
                            *ignore = true;
                            prefix.clear();
                        } else if prefix.len() == 5 {
                            *data = true;
                            *first = true;
                            prefix.clear();
                        }
                    }
                }
            }
        }
    }

    pub(super) fn observation(&self) -> Option<UsageTokens> {
        self.json.observation.map(|fields| {
            UsageTokens::provider(
                fields.input.flatten(),
                fields.output.flatten(),
                fields.total.flatten(),
            )
        })
    }

    /// Only trusted structural locations and complete values can establish an
    /// HTTP200 stream failure. Quoted/generated text is never inspected for it.
    pub(super) fn failed(&self) -> bool {
        self.json.semantic_failed
    }

    #[cfg(test)]
    pub(super) fn retained_bytes(&self) -> usize {
        let token = match &self.json.lexeme {
            Lexeme::Idle => 0,
            Lexeme::String { raw, .. } | Lexeme::Scalar(raw, _, _) => raw.capacity(),
        };
        token + self.json.stack.capacity() * std::mem::size_of::<Frame>()
    }
}
