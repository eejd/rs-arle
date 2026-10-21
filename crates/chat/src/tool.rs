use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

#[derive(Clone, Copy)]
pub(crate) struct TaggedBlock {
    pub(crate) open: &'static str,
    pub(crate) close: &'static str,
}

impl TaggedBlock {
    pub(crate) fn strip_and_collect<T>(
        self,
        text: &str,
        mut parse_block: impl FnMut(&str) -> Option<T>,
    ) -> (String, Vec<T>) {
        let mut parsed = Vec::new();
        let mut remaining = text;
        let mut stripped = String::with_capacity(text.len());

        while let Some(start) = remaining.find(self.open) {
            stripped.push_str(&remaining[..start]);
            remaining = &remaining[start + self.open.len()..];

            if let Some(end) = remaining.find(self.close) {
                let block = remaining[..end].trim();
                if let Some(item) = parse_block(block) {
                    parsed.push(item);
                }
                remaining = &remaining[end + self.close.len()..];
            } else {
                stripped.push_str(remaining);
                remaining = "";
            }
        }

        stripped.push_str(remaining);
        (stripped, parsed)
    }

    pub(crate) fn strip_all(self, text: &str) -> String {
        let (stripped, _) = self.strip_and_collect::<()>(text, |_| None);
        stripped
    }
}

pub(crate) const TOOL_CALL_BLOCK: TaggedBlock = TaggedBlock {
    open: "<tool_call>",
    close: "</tool_call>",
};

pub(crate) const DSML_TOOL_CALLS_BLOCK: TaggedBlock = TaggedBlock {
    open: "<｜DSML｜tool_calls>",
    close: "</｜DSML｜tool_calls>",
};

const DSML_INVOKE_OPEN: &str = "<｜DSML｜invoke";
const DSML_INVOKE_CLOSE: &str = "</｜DSML｜invoke>";
const DSML_PARAMETER_OPEN: &str = "<｜DSML｜parameter";
const DSML_PARAMETER_CLOSE: &str = "</｜DSML｜parameter>";

pub(crate) const THINK_BLOCK: TaggedBlock = TaggedBlock {
    open: "<think>",
    close: "</think>",
};

/// Open/close tags of every block the visible-text drain loops scan for. Shared
/// by `VisibleTextStream::drain` and `StreamingToolCalls::drain` so the set lives
/// in one place.
pub(crate) const VISIBLE_TAGS: [&str; 6] = [
    TOOL_CALL_BLOCK.open,
    TOOL_CALL_BLOCK.close,
    DSML_TOOL_CALLS_BLOCK.open,
    DSML_TOOL_CALLS_BLOCK.close,
    THINK_BLOCK.open,
    THINK_BLOCK.close,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HiddenBlock {
    ToolCall,
    DsmlToolCalls,
    Think,
}

/// Map a matched *open* tag to the [`HiddenBlock`] it starts. Close tags (and any
/// other slice) map to `None`, matching the drain loops' fallthrough. Shared by
/// both drain implementations.
pub(crate) fn hidden_block_for_open_tag(tag: &str) -> Option<HiddenBlock> {
    match tag {
        tag if tag == TOOL_CALL_BLOCK.open => Some(HiddenBlock::ToolCall),
        tag if tag == DSML_TOOL_CALLS_BLOCK.open => Some(HiddenBlock::DsmlToolCalls),
        tag if tag == THINK_BLOCK.open => Some(HiddenBlock::Think),
        _ => None,
    }
}

pub(crate) fn find_first_tag<'a>(text: &str, tags: &'a [&'a str]) -> Option<(usize, &'a str)> {
    tags.iter()
        .filter_map(|tag| text.find(tag).map(|idx| (idx, *tag)))
        .min_by_key(|(idx, _)| *idx)
}

pub(crate) fn longest_tag_prefix_suffix(text: &str, tags: &[&str]) -> usize {
    let text = text.as_bytes();
    let max_len = tags
        .iter()
        .map(|tag| tag.len())
        .max()
        .unwrap_or(0)
        .min(text.len());

    (1..=max_len)
        .rev()
        .find(|&len| {
            let suffix = &text[text.len() - len..];
            tags.iter().any(|tag| tag.as_bytes().starts_with(suffix))
        })
        .unwrap_or(0)
}

pub(crate) fn json_object_len(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'{') {
        return None;
    }
    let mut depth = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    for (i, &c) in bytes.iter().enumerate() {
        if in_str {
            match c {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse a single `<tool_call>` inner JSON payload into a [`ToolCall`].
///
/// Shares the exact decoding contract used by [`parse_tool_calls`].
pub(crate) fn parse_tool_call_block(json_str: &str) -> Option<ToolCall> {
    let value = serde_json::from_str::<Value>(json_str).ok()?;
    // A nameless payload is unroutable — prose *about* tool calls (e.g. an
    // example JSON in an explanation), not a call.
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())?
        .to_string();
    let arguments = value
        .get("arguments")
        .cloned()
        .unwrap_or(Value::Object(Map::default()));
    Some(ToolCall::new(name, arguments))
}

pub(crate) fn parse_streaming_tool_call_block(
    inner: &str,
    schemas: &ToolSchemas,
) -> Option<ToolCall> {
    let inner = inner.trim();
    if inner.is_empty() {
        return None;
    }

    if let Some(function_pos) = inner.find("<function=")
        && inner[..function_pos].trim().is_empty()
    {
        return parse_native_function_block(&inner[function_pos..], schemas);
    }

    let json_start = inner.find('{')?;
    if !inner[..json_start].trim().is_empty() {
        return None;
    }
    let json_len = json_object_len(&inner[json_start..])?;
    parse_tool_call_block(&inner[json_start..json_start + json_len])
}

pub(crate) fn parse_dsml_tool_calls_block(inner: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    let mut rest = inner;

    while let Some(invoke_start) = rest.find(DSML_INVOKE_OPEN) {
        let after_open = &rest[invoke_start + DSML_INVOKE_OPEN.len()..];
        let Some(tag_end) = after_open.find('>') else {
            break;
        };
        let attrs = &after_open[..tag_end];
        let body_start = tag_end + 1;
        let body_and_tail = &after_open[body_start..];
        let Some(invoke_end) = body_and_tail.find(DSML_INVOKE_CLOSE) else {
            break;
        };

        if let Some(call) = parse_dsml_invoke_block(attrs, &body_and_tail[..invoke_end]) {
            calls.push(call);
        }
        rest = &body_and_tail[invoke_end + DSML_INVOKE_CLOSE.len()..];
    }

    calls
}

fn parse_dsml_invoke_block(attrs: &str, body: &str) -> Option<ToolCall> {
    let name = quoted_attr_value(attrs, "name")?.trim().to_string();
    if name.is_empty() {
        return None;
    }

    let mut args = Map::new();
    let mut rest = body;
    while let Some(param_start) = rest.find(DSML_PARAMETER_OPEN) {
        let after_open = &rest[param_start + DSML_PARAMETER_OPEN.len()..];
        let Some(tag_end) = after_open.find('>') else {
            break;
        };
        let attrs = &after_open[..tag_end];
        let value_start = tag_end + 1;
        let value_and_tail = &after_open[value_start..];
        let Some(param_end) = value_and_tail.find(DSML_PARAMETER_CLOSE) else {
            break;
        };

        if let Some(key) = quoted_attr_value(attrs, "name").filter(|key| !key.trim().is_empty()) {
            let key = key.trim().to_string();
            let raw = value_and_tail[..param_end].trim();
            let is_string = quoted_attr_value(attrs, "string").as_deref() != Some("false");
            let value = if is_string {
                Value::String(raw.to_string())
            } else {
                serde_json::from_str::<Value>(raw)
                    .unwrap_or_else(|_| Value::String(raw.to_string()))
            };
            args.insert(key, value);
        }

        rest = &value_and_tail[param_end + DSML_PARAMETER_CLOSE.len()..];
    }

    Some(ToolCall::new(name, Value::Object(args)))
}

fn quoted_attr_value(attrs: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = attrs.find(&needle)? + needle.len();
    let tail = &attrs[start..];
    let end = tail.find('"')?;
    Some(tail[..end].to_string())
}

pub(crate) fn parse_native_function_block(inner: &str, schemas: &ToolSchemas) -> Option<ToolCall> {
    let fstart = inner.find("<function=")?;
    let after_fn = &inner[fstart + "<function=".len()..];
    let name_end = after_fn.find('>')?;
    let name = after_fn[..name_end].trim().to_string();
    if name.is_empty() {
        return None;
    }
    let mut args = Map::new();
    let mut rest = &after_fn[name_end + 1..];
    while let Some(ps) = rest.find("<parameter=") {
        let after_p = &rest[ps + "<parameter=".len()..];
        let Some(key_end) = after_p.find('>') else {
            break;
        };
        let key = after_p[..key_end].trim().to_string();
        let val_area = &after_p[key_end + 1..];
        let Some(pe) = val_area.find("</parameter>") else {
            break;
        };
        let value_text = strip_one_surrounding_newline(&val_area[..pe]);
        let value = typed_parameter_value(value_text, schemas.parameter_type(&name, &key));
        args.insert(key, value);
        rest = &val_area[pe + "</parameter>".len()..];
    }
    Some(ToolCall::new(name, Value::Object(args)))
}

/// How a request's tool schema says a parameter's text is to be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParameterType {
    /// `string`: the text is the value, whatever it looks like.
    Text,
    /// `boolean`: `true` / `false` in any case, which includes Python's
    /// `True` / `False`.
    Boolean,
}

/// The tool definitions of a request, reduced to what the tool-call parse
/// needs: which parameters are declared `string` or `boolean`.
///
/// Qwen's native tool-call XML carries every scalar as bare text, and its chat
/// template spells it with Jinja's `string` filter, so a boolean is `True`, an
/// integer `17`, and a string `12345` is also just `12345`. The text alone
/// cannot say which one it is; the request's tool schema can. A parameter
/// declared `string` is returned as its exact text; one declared `boolean` is
/// read as a boolean (`True` / `False` included). Every other parameter, and
/// every parameter of a tool the schemas do not describe, is typed from its text
/// alone (JSON scalar, array or object, else string), as before.
///
/// Schemas are read through `type`, `anyOf` / `oneOf` and local `$ref`
/// ([`DeclaredTypes`]); `null` is ignored, so pydantic's `Optional[bool]` is a
/// boolean.
#[derive(Debug, Clone, Default)]
pub struct ToolSchemas {
    by_tool: std::collections::HashMap<String, std::collections::HashMap<String, ParameterType>>,
}

impl ToolSchemas {
    /// The schemas of tool definitions in the internal canonical format.
    pub fn from_definitions(tools: &[ToolDefinition]) -> Self {
        Self::from_parameters(
            tools
                .iter()
                .map(|tool| (tool.name.as_str(), &tool.parameters)),
        )
    }

    /// `(tool name, JSON-schema `parameters`)` pairs.
    pub(crate) fn from_parameters<'a>(tools: impl Iterator<Item = (&'a str, &'a Value)>) -> Self {
        let mut by_tool = std::collections::HashMap::<_, std::collections::HashMap<_, _>>::new();
        for (name, parameters) in tools {
            let Some(properties) = parameter_properties(parameters) else {
                continue;
            };
            let declared = properties.iter().filter_map(|(key, schema)| {
                Some((key.clone(), declared_type(schema, parameters)?))
            });
            by_tool
                .entry(name.to_string())
                .or_default()
                .extend(declared);
        }
        by_tool.retain(|_, parameters| !parameters.is_empty());
        Self { by_tool }
    }

    fn parameter_type(&self, tool: &str, parameter: &str) -> Option<ParameterType> {
        self.by_tool.get(tool)?.get(parameter).copied()
    }
}

/// What a parameter schema declares, read through `type`, `anyOf` / `oneOf`
/// and local `$ref`s ([`DeclaredTypes`]): `string` when it accepts a string
/// and nothing but `null` besides; `boolean` when it accepts a boolean and no
/// string. A schema with an unreadable branch, or one that accepts a string
/// beside another type (`["string", "integer"]`, `["string", "boolean"]`),
/// declares nothing: the text could be either, so it is typed from its text.
fn declared_type(schema: &Value, root: &Value) -> Option<ParameterType> {
    let types = DeclaredTypes::of(schema, root);
    match types {
        DeclaredTypes {
            unreadable: true, ..
        } => None,
        DeclaredTypes {
            string: true,
            boolean: false,
            other: false,
            ..
        } => Some(ParameterType::Text),
        DeclaredTypes {
            string: false,
            boolean: true,
            ..
        } => Some(ParameterType::Boolean),
        _ => None,
    }
}

fn spelled_boolean(text: &str) -> Option<bool> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("true") {
        Some(true)
    } else if text.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}

/// The value of one `<parameter=...>` text, read by the declared type of its
/// parameter where there is one.
fn typed_parameter_value(text: &str, declared: Option<ParameterType>) -> Value {
    match declared {
        Some(ParameterType::Text) => Value::String(text.to_string()),
        Some(ParameterType::Boolean) => spelled_boolean(text)
            .map(Value::Bool)
            .unwrap_or_else(|| text_typed_value(text)),
        None => text_typed_value(text),
    }
}

/// Type a parameter text from its own shape: a JSON scalar, array or object,
/// else the text as a string.
fn text_typed_value(text: &str) -> Value {
    serde_json::from_str::<Value>(text.trim())
        .ok()
        .filter(|v| !v.is_string())
        .unwrap_or_else(|| Value::String(text.to_string()))
}

fn strip_one_surrounding_newline(s: &str) -> &str {
    let s = s.strip_prefix('\n').unwrap_or(s);
    let s = s.strip_suffix('\n').unwrap_or(s);
    s.strip_suffix('\r').unwrap_or(s)
}

fn call_arguments_empty(call: &ToolCall) -> bool {
    match &call.arguments {
        Value::Object(map) => map.is_empty(),
        Value::Null => true,
        _ => false,
    }
}

fn split_arguments_object(s: &str) -> Option<(Value, usize)> {
    let after_ws = s.trim_start();
    let mut prefix = s.len() - after_ws.len();
    let body = if let Some(rest) = after_ws.strip_prefix(TOOL_CALL_BLOCK.close) {
        prefix += TOOL_CALL_BLOCK.close.len();
        let rest_trimmed = rest.trim_start();
        prefix += rest.len() - rest_trimmed.len();
        rest_trimmed
    } else {
        after_ws
    };

    if !body.starts_with('{') {
        return None;
    }
    let len = json_object_len(body)?;
    let value = serde_json::from_str::<Value>(&body[..len]).ok()?;
    if !value.is_object() {
        return None;
    }

    let mut consumed = prefix + len;
    let tail = &s[consumed..];
    if let Some(close_at) = tail.find(TOOL_CALL_BLOCK.close)
        && tail[..close_at].trim().is_empty()
    {
        consumed += close_at + TOOL_CALL_BLOCK.close.len();
    }

    Some((value, consumed))
}

fn extract_tool_calls(text: &str, schemas: &ToolSchemas) -> (String, Vec<ToolCall>) {
    if !text.contains(TOOL_CALL_BLOCK.open) && !text.contains(DSML_TOOL_CALLS_BLOCK.open) {
        return (text.to_string(), Vec::new());
    }
    let mut out = String::with_capacity(text.len());
    let mut calls = Vec::new();
    let mut rest = text;
    while let Some((start, tag)) =
        find_first_tag(rest, &[TOOL_CALL_BLOCK.open, DSML_TOOL_CALLS_BLOCK.open])
    {
        out.push_str(&rest[..start]);
        let after = &rest[start + tag.len()..];

        if tag == DSML_TOOL_CALLS_BLOCK.open {
            if let Some(end) = after.find(DSML_TOOL_CALLS_BLOCK.close) {
                calls.extend(parse_dsml_tool_calls_block(after[..end].trim()));
                rest = &after[end + DSML_TOOL_CALLS_BLOCK.close.len()..];
            } else {
                calls.extend(parse_dsml_tool_calls_block(after.trim()));
                rest = "";
            }
            continue;
        }

        if let Some(fpos) = after.find("<function=")
            && after[..fpos].trim().is_empty()
        {
            let consumed = if let Some(fend) = after.find("</function>") {
                let end = fend + "</function>".len();
                if let Some(call) = parse_native_function_block(&after[..end], schemas) {
                    calls.push(call);
                }
                match after[end..].find(TOOL_CALL_BLOCK.close) {
                    Some(c) if after[end..end + c].trim().is_empty() => {
                        end + c + TOOL_CALL_BLOCK.close.len()
                    }
                    _ => end,
                }
            } else {
                after.len()
            };
            rest = &after[consumed..];
            continue;
        }

        match after
            .find('{')
            .and_then(|b| json_object_len(&after[b..]).map(|len| (b, len)))
        {
            Some((b, len)) => {
                let mut consumed = b + len;
                let tail = &after[consumed..];
                if let Some(close_at) = tail.find(TOOL_CALL_BLOCK.close)
                    && tail[..close_at].trim().is_empty()
                {
                    consumed += close_at + TOOL_CALL_BLOCK.close.len();
                }

                if let Some(call) = parse_tool_call_block(after[b..b + len].trim()) {
                    if call_arguments_empty(&call)
                        && let Some((args, follow_consumed)) =
                            split_arguments_object(&after[consumed..])
                    {
                        calls.push(ToolCall::new(call.name, args));
                        consumed += follow_consumed;
                    } else {
                        calls.push(call);
                    }
                } else {
                    // Unparseable / nameless payload — a mention, not a call:
                    // keep the literal text instead of swallowing it.
                    out.push_str(tag);
                    out.push_str(&after[..consumed]);
                }
                rest = &after[consumed..];
            }
            None => {
                // No parseable JSON after the opener. A `{` adjacent to the
                // opener is a truncated real call — drop it, don't leak
                // half-JSON. Anything else is a literal tag mention: keep it.
                match after.find('{') {
                    Some(b) if after[..b].trim().is_empty() => rest = "",
                    _ => {
                        out.push_str(tag);
                        rest = after;
                    }
                }
            }
        }
    }
    out.push_str(rest);
    (out, calls)
}

/// The scalar types a JSON-schema fragment can accept, read from `type`,
/// `anyOf` / `oneOf` / `allOf` and local `$ref`s (`#/...`, resolved against the
/// tool's `parameters` object).
///
/// `null` is ignored: `Optional[bool]` (pydantic: `anyOf: [boolean, null]`) is a
/// boolean for this purpose. `allOf` is read as the union of its entries'
/// types (conservative: a type any entry names counts, so a string entry
/// makes a parameter ambiguous), and an entry that names no type is a mere
/// constraint (`maxLength`, `description`, pydantic v1's
/// `{"allOf": [{"$ref": ...}], "default": ...}`). `enum` / `const` are not read.
///
/// The walk is bounded whatever the schema looks like (the schema comes from
/// the request): at most [`SCHEMA_NODE_LIMIT`] schema nodes are visited, at
/// most [`SCHEMA_DEPTH_LIMIT`] deep, and it stops at the first unreadable
/// node, so a self-referential or exponentially branching `$ref` / `anyOf`
/// ends as `unreadable` instead of being expanded.
#[derive(Debug, Default)]
pub(crate) struct DeclaredTypes {
    pub(crate) string: bool,
    pub(crate) boolean: bool,
    /// Any other type (`integer`, `array`, ...).
    pub(crate) other: bool,
    /// A node whose type cannot be read: no `type` / `anyOf` / `oneOf` /
    /// `allOf` / `$ref` (outside an `allOf` entry), a `$ref` that is not a
    /// resolvable local pointer, a non-string `type` entry, or a walk past the
    /// depth or node limit. Callers must not type a parameter on the strength
    /// of a schema with an unreadable node.
    pub(crate) unreadable: bool,
    /// Schema nodes visited (at most the limit plus the node that crossed it).
    pub(crate) nodes: u32,
}

const SCHEMA_DEPTH_LIMIT: u8 = 8;
pub(crate) const SCHEMA_NODE_LIMIT: u32 = 128;

impl DeclaredTypes {
    pub(crate) fn of(schema: &Value, root: &Value) -> Self {
        let mut types = Self::default();
        if !types.collect(schema, root, 0) {
            types.unreadable = true;
        }
        types
    }

    /// Whether `schema` named any type (directly or through `anyOf` / `oneOf` /
    /// `allOf` / `$ref`); the caller decides what an unnamed one means.
    fn collect(&mut self, schema: &Value, root: &Value, depth: u8) -> bool {
        if self.unreadable {
            return true;
        }
        self.nodes += 1;
        if depth >= SCHEMA_DEPTH_LIMIT || self.nodes > SCHEMA_NODE_LIMIT {
            self.unreadable = true;
            return true;
        }
        let mut readable = false;
        match schema.get("type") {
            Some(Value::String(ty)) => {
                readable = true;
                self.note(ty);
            }
            Some(Value::Array(types)) => {
                readable = true;
                for ty in types {
                    match ty.as_str() {
                        Some(ty) => self.note(ty),
                        None => self.unreadable = true,
                    }
                }
            }
            _ => {}
        }
        for key in ["anyOf", "oneOf"] {
            if let Some(Value::Array(branches)) = schema.get(key) {
                readable = true;
                for branch in branches {
                    if !self.collect(branch, root, depth + 1) {
                        self.unreadable = true;
                    }
                    if self.unreadable {
                        return true;
                    }
                }
            }
        }
        if let Some(Value::Array(entries)) = schema.get("allOf") {
            for entry in entries {
                readable |= self.collect(entry, root, depth + 1);
                if self.unreadable {
                    return true;
                }
            }
        }
        if let Some(reference) = schema.get("$ref") {
            readable = true;
            match reference
                .as_str()
                .and_then(|r| r.strip_prefix('#'))
                .and_then(|pointer| root.pointer(pointer))
            {
                Some(target) if self.collect(target, root, depth + 1) => {}
                _ => self.unreadable = true,
            }
        }
        readable
    }

    fn note(&mut self, ty: &str) {
        match ty {
            "string" => self.string = true,
            "boolean" => self.boolean = true,
            "null" => {}
            _ => self.other = true,
        }
    }
}

/// The `properties` of a tool's `parameters` object, following one top-level
/// local `$ref` (pydantic emits `{"$ref": "#/$defs/Args", "$defs": {...}}`).
pub(crate) fn parameter_properties(parameters: &Value) -> Option<&Map<String, Value>> {
    let object = match parameters.get("$ref").and_then(Value::as_str) {
        Some(reference) => parameters.pointer(reference.strip_prefix('#')?)?,
        None => parameters,
    };
    object.get("properties")?.as_object()
}

/// Structured tool definition used for prompt injection.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl ToolDefinition {
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }

    pub(crate) fn prompt_schema(&self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "arguments": compact_parameters(&self.parameters),
        })
    }
}

fn compact_parameters(parameters: &Value) -> Value {
    let Some(object) = parameters.as_object() else {
        return parameters.clone();
    };

    let Some(properties) = object.get("properties").and_then(Value::as_object) else {
        return parameters.clone();
    };

    let mut compact = Map::new();
    for (name, schema) in properties {
        let ty = schema
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("value")
            .to_string();
        compact.insert(name.clone(), Value::String(ty));
    }

    if let Some(required) = object.get("required").and_then(Value::as_array)
        && !required.is_empty()
    {
        compact.insert("required".to_string(), Value::Array(required.clone()));
    }

    Value::Object(compact)
}

/// Structured tool call emitted by the model or embedded in an assistant turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

impl ToolCall {
    pub fn new(name: impl Into<String>, arguments: Value) -> Self {
        Self {
            name: name.into(),
            arguments,
        }
    }

    /// Render this call as the body that goes INSIDE a `<tool_call>...</tool_call>`
    /// wrapper, in the Qwen3.6-Coder native XML format the chat template trains on.
    pub(crate) fn prompt_payload(&self) -> String {
        let mut out = String::new();
        out.push_str("<function=");
        out.push_str(&self.name);
        out.push_str(">\n");
        match self.arguments.as_object() {
            Some(args) => {
                for (key, value) in args {
                    push_parameter(&mut out, key, value);
                }
            }
            None if !self.arguments.is_null() => {
                push_parameter(&mut out, "arguments", &self.arguments);
            }
            None => {}
        }
        out.push_str("</function>");
        out
    }
}

fn push_parameter(out: &mut String, key: &str, value: &Value) {
    out.push_str("<parameter=");
    out.push_str(key);
    out.push_str(">\n");
    match value {
        Value::String(s) => out.push_str(s),
        other => out.push_str(&serde_json::to_string(other).expect("tool arg serialization")),
    }
    out.push_str("\n</parameter>\n");
}

/// Parse `<tool_call>...</tool_call>` blocks from raw assistant output.
///
/// `schemas` types native `<parameter=...>` values by the declared parameter
/// types (see [`ToolSchemas`]); `&ToolSchemas::default()` types them from
/// their text alone.
pub fn parse_tool_calls(text: &str, schemas: &ToolSchemas) -> super::ParsedAssistantResponse {
    let (stripped, tool_calls) = extract_tool_calls(text, schemas);

    super::ParsedAssistantResponse {
        content: THINK_BLOCK.strip_all(&stripped).trim().to_string(),
        tool_calls,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> ToolSchemas {
        ToolSchemas::from_parameters(
            [
                (
                    "echo_args",
                    &json!({"type": "object", "properties": {
                        "text": {"type": "string"},
                        "zip": {"type": "string"},
                        "note": {"type": ["string", "null"]},
                        "word": {"type": "string"},
                        "count": {"type": "integer"},
                        "flag": {"type": "boolean"},
                        "maybe": {"type": ["boolean", "null"]},
                        "either": {"type": ["string", "integer"]},
                        "any": {"anyOf": [{"type": "string"}, {"type": "integer"}]},
                        "optflag": {"anyOf": [{"type": "boolean"}, {"type": "null"}]},
                        "optzip": {"anyOf": [{"type": "string"}, {"type": "null"}]},
                        "refzip": {"$ref": "#/$defs/Zip"},
                        "refflag": {"$ref": "#/$defs/Flag"},
                        "nestedzip": {"anyOf": [{"$ref": "#/$defs/Zip"}, {"type": "null"}]},
                        "choice": {"oneOf": [{"type": "boolean"}, {"type": "integer"}]},
                        "v1zip": {"allOf": [{"$ref": "#/$defs/Zip"}], "default": "0"},
                        "both": {"anyOf": [{"type": "string"}, {"type": "boolean"}]},
                        "dangling": {"$ref": "#/$defs/Missing"},
                    }, "$defs": {"Zip": {"type": "string", "title": "Zip"},
                                       "Flag": {"type": "boolean"}}}),
                ),
                ("no_properties", &json!({"type": "object"})),
            ]
            .into_iter(),
        )
    }

    fn call(tool: &str, parameters: &[(&str, &str)]) -> String {
        let mut out = format!("<tool_call>\n<function={tool}>\n");
        for (key, value) in parameters {
            out.push_str(&format!("<parameter={key}>\n{value}\n</parameter>\n"));
        }
        out.push_str("</function>\n</tool_call>");
        out
    }

    fn parsed(text: &str, schemas: &ToolSchemas) -> Value {
        let calls = parse_tool_calls(text, schemas).tool_calls;
        assert_eq!(calls.len(), 1);
        calls[0].arguments.clone()
    }

    fn streamed(text: &str, schemas: &ToolSchemas, chunk: usize) -> Value {
        let mut stream = crate::StreamingToolCalls::new(schemas.clone());
        let mut calls = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        for piece in chars.chunks(chunk) {
            calls.extend(stream.push(&piece.iter().collect::<String>()).1);
        }
        calls.extend(stream.finish().1);
        assert_eq!(calls.len(), 1);
        calls.remove(0).arguments
    }

    #[test]
    fn declared_parameters_are_typed_by_the_schema() {
        // The Qwen3.5 text of the quantbench `scalars` case (`True`), and
        // string parameters whose text looks like a number, `null`, a boolean.
        let text = call(
            "echo_args",
            &[
                ("text", "hand summer"),
                ("count", "17"),
                ("flag", "True"),
                ("zip", "12345"),
                ("note", "null"),
                ("word", "true"),
                ("maybe", " FALSE "),
                ("optflag", "True"),
                ("optzip", "12345"),
                ("refzip", "12345"),
                ("refflag", "True"),
                ("nestedzip", "20240131"),
                ("choice", "FALSE"),
                ("v1zip", "8675309"),
                ("refzip2", "5"),
            ],
        );
        let want = json!({
            "text": "hand summer", "count": 17, "flag": true, "zip": "12345",
            "note": "null", "word": "true", "maybe": false,
            "optflag": true, "optzip": "12345", "refzip": "12345", "refzip2": 5,
            "refflag": true, "nestedzip": "20240131", "choice": false, "v1zip": "8675309"
        });
        assert_eq!(parsed(&text, &tools()), want);
        for chunk in [1, 7, text.len()] {
            assert_eq!(streamed(&text, &tools(), chunk), want, "chunk {chunk}");
        }
    }

    #[test]
    fn without_a_declaration_the_text_types_itself() {
        // Undeclared parameter, ambiguous `type` array, `anyOf`, a boolean
        // parameter whose text is no boolean, a tool without `properties` and
        // a tool the schemas do not know.
        let text = call(
            "echo_args",
            &[
                ("extra", "5"),
                ("either", "12345"),
                ("any", "12345"),
                ("both", "true"),
                ("dangling", "12345"),
                ("flag", "yes"),
                ("items", "[1, 2]"),
            ],
        );
        assert_eq!(
            parsed(&text, &tools()),
            json!({
                "extra": 5, "either": 12345, "any": 12345, "both": true, "dangling": 12345,
                "flag": "yes", "items": [1, 2]
            })
        );
        for tool in ["no_properties", "unknown_tool"] {
            let text = call(tool, &[("zip", "12345"), ("flag", "True"), ("on", "true")]);
            assert_eq!(
                parsed(&text, &tools()),
                json!({"zip": 12345, "flag": "True", "on": true}),
                "{tool}"
            );
        }
        assert_eq!(
            parsed(
                &call("echo_args", &[("zip", "12345")]),
                &ToolSchemas::default()
            ),
            json!({"zip": 12345})
        );
    }

    #[test]
    fn json_tool_call_arguments_are_not_retyped() {
        let text = r#"<tool_call>{"name": "echo_args", "arguments": {"zip": 12345, "text": "12345", "flag": "True"}}</tool_call>"#;
        assert_eq!(
            parsed(text, &tools()),
            json!({"zip": 12345, "text": "12345", "flag": "True"})
        );
    }

    #[test]
    fn a_parameters_object_behind_a_top_level_ref_is_read() {
        let schemas = ToolSchemas::from_parameters(
            [(
                "echo_args",
                &json!({
                    "$ref": "#/$defs/Args",
                    "$defs": {"Args": {"type": "object", "properties": {
                        "zip": {"type": "string"}, "flag": {"type": "boolean"},
                    }}},
                }),
            )]
            .into_iter(),
        );
        assert_eq!(
            parsed(
                &call("echo_args", &[("zip", "12345"), ("flag", "True")]),
                &schemas
            ),
            json!({"zip": "12345", "flag": true})
        );
    }

    #[test]
    fn adversarial_schemas_leave_the_parameter_text_typed_and_return() {
        // 400 branches that are all `$ref`s back to the definition, and a
        // chain that doubles at each of 20 levels: both unreadable, so
        // `zip` (a `string` on paper) is typed from its text; no blow-up.
        let selfref: Vec<Value> = (0..400).map(|_| json!({"$ref": "#/$defs/A"})).collect();
        let mut defs = serde_json::Map::new();
        for i in 0..20 {
            let next = json!({"$ref": format!("#/$defs/D{}", i + 1)});
            defs.insert(format!("D{i}"), json!({"anyOf": [next.clone(), next]}));
        }
        defs.insert("D20".into(), json!({"type": "string"}));
        defs.insert("A".into(), json!({"anyOf": selfref}));
        // wider than the node limit, though every branch is a plain string
        let wide: Vec<Value> = (0..400).map(|_| json!({"type": "string"})).collect();
        let schemas = ToolSchemas::from_parameters(
            [(
                "echo_args",
                &json!({"type": "object", "properties": {
                    "zip": {"$ref": "#/$defs/A"},
                    "deep": {"$ref": "#/$defs/D0"},
                    "text": {"type": "string"},
                    "wide": {"anyOf": wide},
                }, "$defs": Value::Object(defs)}),
            )]
            .into_iter(),
        );
        assert_eq!(
            parsed(
                &call(
                    "echo_args",
                    &[
                        ("zip", "12345"),
                        ("deep", "67890"),
                        ("text", "11"),
                        ("wide", "99")
                    ]
                ),
                &schemas
            ),
            json!({"zip": 12345, "deep": 67890, "text": "11", "wide": 99})
        );
    }
}
