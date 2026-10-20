//! OpenAI-compatible chat request/response types layered on top of the shared
//! chat/tool-call protocol helpers in [`crate::protocol`].
//!
//! Naming convention:
//! - Types and functions prefixed `OpenAi` / `openai_` are the OpenAI wire
//!   format (HTTP request/response bodies).
//! - Unprefixed types and functions (`ChatMessage`, `ToolCall`, `ToolDefinition`,
//!   `messages_to_prompt`, `parse_tool_calls`) are the internal canonical
//!   protocol format, re-exported from [`crate::protocol`].

pub mod protocol;

pub use protocol::{
    ChatMessage, ChatMlMessage, ChatMlSpan, ChatRole, ParsedAssistantResponse, RenderedChatMl,
    StreamingToolCalls, ToolCall, ToolChoiceMode, ToolDefinition, VisibleTextStream,
    build_tool_block, build_tool_block_with_choice, messages_to_prompt,
    messages_to_prompt_with_tool_choice, parse_tool_calls, render_chatml, render_chatml_with_spans,
    render_structured_chatml_with_spans,
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const DSV4_BOS: &str = "<｜begin▁of▁sentence｜>";
const DSV4_EOS: &str = "<｜end▁of▁sentence｜>";
const DSV4_USER: &str = "<｜User｜>";
const DSV4_ASSISTANT: &str = "<｜Assistant｜>";
const DSV4_THINK_START: &str = "<think>";
const DSV4_THINK_END: &str = "</think>";
const DSV4_DSML: &str = "｜DSML｜";

const DSV4_REASONING_EFFORT_MAX: &str = concat!(
    "Reasoning Effort: Absolute maximum with no shortcuts permitted.\n",
    "You MUST be very thorough in your thinking and comprehensively decompose the problem to resolve the root cause, rigorously stress-testing your logic against all potential paths, edge cases, and adversarial scenarios.\n",
    "Explicitly write out your entire deliberation process, documenting every intermediate step, considered alternative, and rejected hypothesis to ensure absolutely no assumption is left unchecked.\n\n"
);

/// A single message in a chat conversation, in OpenAI wire format.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAiChatMessage {
    pub role: String,
    /// Text content. `None` when the assistant message contains only tool calls.
    #[serde(default)]
    pub content: Option<OpenAiChatContent>,
    /// Tool calls emitted by the assistant.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<OpenAiToolCall>,
    /// Present on `tool` role messages — the call id being responded to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Present on `tool` role messages — the tool name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Message content as sent by OAI-compatible clients.
///
/// Modern tools (OpenAI SDK, LiteLLM, LangChain's
/// openai adapter) always send `content` as a **part array**
/// (`[{"type":"text","text":"..."}, ...]`) to leave room for multimodal
/// inputs, while older tools still send a plain string. Our server is
/// text-only so we accept both and flatten via [`Self::to_text`].
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OpenAiChatContent {
    /// Legacy plain-text form: `"content": "hello"`.
    Text(String),
    /// Modern part-array form: `"content": [{"type":"text","text":"hello"}, ...]`.
    /// Kept as untyped `Value` so unsupported part types (image_url, audio,
    /// etc.) do not fail deserialization — they are simply ignored by
    /// [`Self::to_text`].
    Parts(Vec<Value>),
}

impl OpenAiChatContent {
    /// Flatten to a plain text string. For the Parts form, concatenates
    /// every `{"type":"text","text":"..."}` part in order; parts whose type
    /// is not `text` are silently skipped (we are a text-only server).
    pub fn to_text(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Parts(parts) => parts
                .iter()
                .filter(|p| p.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|p| p.get("text").and_then(Value::as_str))
                .collect(),
        }
    }
}

impl From<String> for OpenAiChatContent {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<&str> for OpenAiChatContent {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

/// OpenAI-format tool call object embedded in an assistant message.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAiToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: OpenAiFunctionCall,
}

/// Function name + JSON-encoded arguments.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAiFunctionCall {
    pub name: String,
    /// JSON string (not parsed object) — matches OpenAI wire format.
    pub arguments: String,
}

/// Tool definition passed in a chat completion request.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAiToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAiFunctionDefinition,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAiFunctionDefinition {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<Value>,
}

impl From<&OpenAiToolCall> for ToolCall {
    fn from(tool_call: &OpenAiToolCall) -> Self {
        let arguments = serde_json::from_str::<Value>(&tool_call.function.arguments)
            .unwrap_or_else(|_| Value::String(tool_call.function.arguments.clone()));
        Self::new(tool_call.function.name.clone(), arguments)
    }
}

impl From<&OpenAiChatMessage> for ChatMessage {
    fn from(message: &OpenAiChatMessage) -> Self {
        let tool_calls = message.tool_calls.iter().map(ToolCall::from).collect();

        Self {
            role: ChatRole::from(message.role.as_str()),
            content: message
                .content
                .as_ref()
                .map(OpenAiChatContent::to_text)
                .unwrap_or_default(),
            tool_calls,
        }
    }
}

impl From<&OpenAiToolDefinition> for ToolDefinition {
    fn from(tool: &OpenAiToolDefinition) -> Self {
        Self::new(
            tool.function.name.clone(),
            tool.function.description.clone().unwrap_or_default(),
            tool.function
                .parameters
                .clone()
                .unwrap_or_else(|| json!({})),
        )
    }
}

/// Convert an OpenAI messages array + optional tool definitions into a
/// ChatML prompt string ready for inference.
pub fn openai_messages_to_prompt(
    messages: &[OpenAiChatMessage],
    tools: &[OpenAiToolDefinition],
) -> String {
    openai_messages_to_prompt_with_tool_choice(messages, tools, &ToolChoiceMode::Auto)
}

/// Convert an OpenAI messages array + optional tool definitions into a ChatML
/// prompt, honoring an OpenAI `tool_choice` hint.
pub fn openai_messages_to_prompt_with_tool_choice(
    messages: &[OpenAiChatMessage],
    tools: &[OpenAiToolDefinition],
    choice: &ToolChoiceMode,
) -> String {
    let protocol_messages = messages.iter().map(ChatMessage::from).collect::<Vec<_>>();
    let protocol_tools = tools.iter().map(ToolDefinition::from).collect::<Vec<_>>();
    messages_to_prompt_with_tool_choice(&protocol_messages, &protocol_tools, choice)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeepSeekV4ChatTemplateOptions {
    /// `false` renders the non-thinking chat prefix:
    /// `<｜Assistant｜></think>`. `true` renders `<｜Assistant｜><think>`.
    pub thinking: bool,
    /// Currently only `"max"` emits the official long reasoning-effort prefix.
    pub reasoning_effort: Option<String>,
}

/// Convert OpenAI messages to the DeepSeek-V4 encoding format.
///
/// DeepSeek-V4-Flash does not ship a Jinja `chat_template`; the model repo
/// provides a dedicated `encoding/encoding_dsv4.py` reference. This renderer
/// implements the serving subset used by OpenAI-compatible chat requests:
/// system/user/assistant/tool messages, OpenAI function-tool definitions, and
/// non-thinking or thinking assistant generation prefixes.
pub fn openai_messages_to_deepseek_v4_prompt(
    messages: &[OpenAiChatMessage],
    tools: &[OpenAiToolDefinition],
    options: &DeepSeekV4ChatTemplateOptions,
) -> String {
    let mut out = String::from(DSV4_BOS);
    if options.thinking && options.reasoning_effort.as_deref() == Some("max") {
        out.push_str(DSV4_REASONING_EFFORT_MAX);
    }

    let assistant_prefix = if options.thinking {
        DSV4_THINK_START
    } else {
        DSV4_THINK_END
    };
    let mut rendered_tools = false;

    if !tools.is_empty() && !messages.iter().any(|m| m.role == "system") {
        out.push_str(&render_deepseek_v4_tools(tools));
        rendered_tools = true;
    }

    for (idx, message) in messages.iter().enumerate() {
        match message.role.as_str() {
            "system" => {
                if let Some(content) = message.content.as_ref() {
                    out.push_str(&content.to_text());
                }
                if !rendered_tools && !tools.is_empty() {
                    out.push_str("\n\n");
                    out.push_str(&render_deepseek_v4_tools(tools));
                    rendered_tools = true;
                }
            }
            "user" => {
                out.push_str(DSV4_USER);
                if let Some(content) = message.content.as_ref() {
                    out.push_str(&content.to_text());
                }
            }
            "assistant" => {
                out.push_str(DSV4_ASSISTANT);
                if let Some(content) = message.content.as_ref() {
                    out.push_str(&content.to_text());
                }
                if !message.tool_calls.is_empty() {
                    out.push_str(&render_deepseek_v4_tool_calls(&message.tool_calls));
                }
                out.push_str(DSV4_EOS);
            }
            "tool" => {
                out.push_str(DSV4_USER);
                out.push_str("<tool_result>");
                if let Some(content) = message.content.as_ref() {
                    out.push_str(&content.to_text());
                }
                out.push_str("</tool_result>");
            }
            "developer" => {
                out.push_str(DSV4_USER);
                if let Some(content) = message.content.as_ref() {
                    out.push_str(&content.to_text());
                }
            }
            _ => {
                out.push_str(DSV4_USER);
                if let Some(content) = message.content.as_ref() {
                    out.push_str(&content.to_text());
                }
            }
        }

        let is_last = idx + 1 == messages.len();
        if is_last
            && matches!(
                message.role.as_str(),
                "system" | "user" | "tool" | "developer"
            )
        {
            out.push_str(DSV4_ASSISTANT);
            out.push_str(assistant_prefix);
        }
    }

    out
}

fn render_deepseek_v4_tools(tools: &[OpenAiToolDefinition]) -> String {
    let schemas = tools
        .iter()
        .map(|tool| {
            serde_json::to_string(&json!({
                "name": tool.function.name,
                "description": tool.function.description.clone().unwrap_or_default(),
                "parameters": tool.function.parameters.clone().unwrap_or_else(|| json!({})),
            }))
            .expect("tool schema serialization")
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "## Tools\n\n\
You have access to a set of tools to help answer the user's question. You can invoke tools by writing a \"<{DSV4_DSML}tool_calls>\" block like the following:\n\n\
<{DSV4_DSML}tool_calls>\n\
<{DSV4_DSML}invoke name=\"$TOOL_NAME\">\n\
<{DSV4_DSML}parameter name=\"$PARAMETER_NAME\" string=\"true|false\">$PARAMETER_VALUE</{DSV4_DSML}parameter>\n\
...\n\
</{DSV4_DSML}invoke>\n\
</{DSV4_DSML}tool_calls>\n\n\
String parameters should be specified as is and set `string=\"true\"`. For all other types (numbers, booleans, arrays, objects), pass the value in JSON format and set `string=\"false\"`.\n\n\
If thinking_mode is enabled (triggered by {DSV4_THINK_START}), you MUST output your complete reasoning inside {DSV4_THINK_START}...{DSV4_THINK_END} BEFORE any tool calls or final response.\n\n\
Otherwise, output directly after {DSV4_THINK_END} with tool calls or final response.\n\n\
### Available Tool Schemas\n\n\
{schemas}\n\n\
You MUST strictly follow the above defined tool name and parameter schemas to invoke tool calls."
    )
}

fn render_deepseek_v4_tool_calls(tool_calls: &[OpenAiToolCall]) -> String {
    if tool_calls.is_empty() {
        return String::new();
    }

    let calls = tool_calls
        .iter()
        .map(|call| {
            format!(
                "<{DSV4_DSML}invoke name=\"{}\">\n{}\n</{DSV4_DSML}invoke>",
                call.function.name,
                render_deepseek_v4_tool_arguments(&call.function.arguments)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("\n\n<{DSV4_DSML}tool_calls>\n{calls}\n</{DSV4_DSML}tool_calls>")
}

fn render_deepseek_v4_tool_arguments(arguments: &str) -> String {
    let parsed = serde_json::from_str::<Value>(arguments)
        .unwrap_or_else(|_| json!({ "arguments": arguments }));
    let Some(obj) = parsed.as_object() else {
        return format!(
            "<{DSV4_DSML}parameter name=\"arguments\" string=\"false\">{parsed}</{DSV4_DSML}parameter>"
        );
    };

    obj.iter()
        .map(|(key, value)| {
            let is_string = value.is_string();
            let rendered = value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string());
            format!(
                "<{DSV4_DSML}parameter name=\"{key}\" string=\"{}\">{rendered}</{DSV4_DSML}parameter>",
                if is_string { "true" } else { "false" }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse `<tool_call>...</tool_call>` blocks from model output.
/// Returns `(visible_content, tool_calls)` where `visible_content` has the
/// tool call blocks and `<think>` blocks stripped.
pub fn openai_parse_tool_calls(text: &str) -> (String, Vec<ToolCall>) {
    let ParsedAssistantResponse {
        content,
        tool_calls,
    } = parse_tool_calls(text);
    (content, tool_calls)
}

/// The parameters a request's tool schemas declare `boolean`, by tool name.
///
/// Qwen's native tool-call XML spells every scalar the way Jinja's `string`
/// filter does (its chat template renders arguments with `| string`), so a
/// boolean is written `True` / `False`. That is not JSON:
/// [`parse_tool_calls`] types the text from its own shape alone (it has no
/// tool schema) and leaves `True` as the string `"True"`, while `17` and
/// `true` become a number and a boolean. This type carries the schema's
/// answer to the one place that can give it: a string argument of a
/// parameter declared `boolean` whose text is `true` / `false` in any case
/// becomes a JSON boolean. Nothing else is retyped, so a parameter declared
/// `string` that holds the text `True` stays a string.
#[derive(Debug, Clone, Default)]
pub struct BooleanArguments {
    by_tool: std::collections::HashMap<String, Vec<String>>,
}

impl BooleanArguments {
    /// Collect the `boolean` parameters of `tools`: those whose schema, read
    /// through `type`, `anyOf` / `oneOf` and local `$ref`s, accepts `boolean`
    /// and not `string` (`null` aside), and has no branch that cannot be read.
    /// `["boolean", "integer"]` counts; `["boolean", "string"]` is ambiguous
    /// (the text `True` may be the answer) and does not.
    pub fn from_tools(tools: &[OpenAiToolDefinition]) -> Self {
        let mut by_tool = std::collections::HashMap::<String, Vec<String>>::new();
        for tool in tools {
            let Some(parameters) = tool.function.parameters.as_ref() else {
                continue;
            };
            let Some(properties) = protocol::parameter_properties(parameters) else {
                continue;
            };
            let names = properties
                .iter()
                .filter(|(_, schema)| declares_boolean(schema, parameters))
                .map(|(name, _)| name.clone());
            let mut names = names.peekable();
            if names.peek().is_some() {
                by_tool
                    .entry(tool.function.name.clone())
                    .or_default()
                    .extend(names);
            }
        }
        Self { by_tool }
    }

    /// Type the boolean parameters of one parsed call.
    pub fn coerce(&self, call: &mut ToolCall) {
        let Some(names) = self.by_tool.get(&call.name) else {
            return;
        };
        let Some(arguments) = call.arguments.as_object_mut() else {
            return;
        };
        for name in names {
            if let Some(value) = arguments.get_mut(name)
                && let Value::String(text) = value
                && let Some(boolean) = spelled_boolean(text)
            {
                *value = Value::Bool(boolean);
            }
        }
    }

    /// Type the boolean parameters of every parsed call.
    pub fn coerce_all(&self, calls: &mut [ToolCall]) {
        for call in calls {
            self.coerce(call);
        }
    }
}

fn declares_boolean(schema: &Value, root: &Value) -> bool {
    let types = protocol::DeclaredTypes::of(schema, root);
    types.boolean && !types.string && !types.unreadable
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, parameters: Value) -> OpenAiToolDefinition {
        OpenAiToolDefinition {
            tool_type: "function".into(),
            function: OpenAiFunctionDefinition {
                name: name.into(),
                description: None,
                parameters: Some(parameters),
            },
        }
    }

    fn echo_args() -> OpenAiToolDefinition {
        tool(
            "echo_args",
            json!({"type": "object", "properties": {
                "text": {"type": "string"},
                "count": {"type": "integer"},
                "flag": {"type": "boolean"},
                "maybe": {"type": ["boolean", "null"]},
                "label": {"type": ["boolean", "string"]},
            }}),
        )
    }

    // The tool block of the Qwen3.5 template, as the model writes it
    // (quantbench tool_arg `scalars` case; the reference text of the engine run).
    const QWEN_SCALARS: &str = "<tool_call>\n<function=echo_args>\n<parameter=text>\nhand summer\n</parameter>\n<parameter=count>\n17\n</parameter>\n<parameter=flag>\nTrue\n</parameter>\n</function>\n</tool_call>";

    #[test]
    fn declared_boolean_parameters_become_booleans() {
        let booleans = BooleanArguments::from_tools(&[echo_args()]);
        let (_, mut calls) = openai_parse_tool_calls(QWEN_SCALARS);
        booleans.coerce_all(&mut calls);
        assert_eq!(
            calls[0].arguments,
            json!({"text": "hand summer", "count": 17, "flag": true})
        );

        let mut call = ToolCall::new(
            "echo_args",
            json!({"flag": "False", "maybe": " TRUE ", "count": 3}),
        );
        booleans.coerce(&mut call);
        assert_eq!(
            call.arguments,
            json!({"flag": false, "maybe": true, "count": 3})
        );
    }

    #[test]
    fn only_declared_boolean_parameters_of_the_called_tool_are_retyped() {
        let booleans = BooleanArguments::from_tools(&[echo_args()]);

        // `text` is a string: the text `True` is the answer, not a boolean.
        // `label` lists `string` beside `boolean`: ambiguous, stays text.
        // Undeclared parameters keep what the parser made of them.
        let mut call = ToolCall::new(
            "echo_args",
            json!({"text": "True", "label": "True", "extra": "True", "flag": "yes"}),
        );
        booleans.coerce(&mut call);
        assert_eq!(
            call.arguments,
            json!({"text": "True", "label": "True", "extra": "True", "flag": "yes"})
        );

        // The same parameter name on another tool is not touched.
        let mut other = ToolCall::new("other_tool", json!({"flag": "True"}));
        booleans.coerce(&mut other);
        assert_eq!(other.arguments, json!({"flag": "True"}));
    }

    #[test]
    fn calls_without_an_object_of_arguments_or_a_schema_are_left_alone() {
        let booleans = BooleanArguments::from_tools(&[
            echo_args(),
            tool("no_properties", json!({"type": "object"})),
        ]);
        let mut raw = ToolCall::new("echo_args", Value::String("flag=True".into()));
        booleans.coerce(&mut raw);
        assert_eq!(raw.arguments, Value::String("flag=True".into()));

        let mut call = ToolCall::new("no_properties", json!({"flag": "True"}));
        booleans.coerce(&mut call);
        assert_eq!(call.arguments, json!({"flag": "True"}));

        BooleanArguments::default().coerce(&mut call);
        assert_eq!(call.arguments, json!({"flag": "True"}));
    }

    fn coerced(parameters: Value, arguments: Value) -> Value {
        let booleans = BooleanArguments::from_tools(&[tool("t", parameters)]);
        let mut call = ToolCall::new("t", arguments);
        booleans.coerce(&mut call);
        call.arguments
    }

    #[test]
    fn nullable_and_referenced_boolean_schemas_are_read() {
        let args = json!({
            "opt": "True", "one": "false", "refd": "TRUE", "nested": "False",
            "mixed": "True", "other": "True",
        });
        let parameters = json!({"type": "object", "properties": {
            // pydantic `Optional[bool]`
            "opt": {"anyOf": [{"type": "boolean"}, {"type": "null"}], "default": null},
            "one": {"oneOf": [{"type": "boolean"}, {"type": "integer"}]},
            "refd": {"$ref": "#/$defs/Flag"},
            "nested": {"anyOf": [{"$ref": "#/$defs/Flag"}, {"type": "null"}]},
            // ambiguous: the text `True` may be the answer
            "mixed": {"anyOf": [{"type": "string"}, {"type": "boolean"}]},
            "other": {"type": "integer"},
        }, "$defs": {"Flag": {"type": "boolean", "title": "Flag"}}});
        assert_eq!(
            coerced(parameters, args),
            json!({
                "opt": true, "one": false, "refd": true, "nested": false,
                "mixed": "True", "other": "True",
            })
        );
    }

    #[test]
    fn a_parameters_object_behind_a_top_level_ref_is_read() {
        let parameters = json!({
            "$ref": "#/$defs/Args",
            "$defs": {"Args": {"type": "object", "properties": {"flag": {"type": "boolean"}}}},
        });
        assert_eq!(
            coerced(parameters, json!({"flag": "True"})),
            json!({"flag": true})
        );
    }

    #[test]
    fn unreadable_schemas_are_left_untyped() {
        let parameters = json!({"type": "object", "properties": {
            "dangling": {"anyOf": [{"type": "boolean"}, {"$ref": "#/$defs/Missing"}]},
            "remote": {"$ref": "https://example.com/flag.json"},
            "open": {"anyOf": [{"type": "boolean"}, {}]},
            "cycle": {"$ref": "#/properties/cycle"},
            "bare": {"description": "no type at all"},
        }});
        let args = json!({
            "dangling": "True", "remote": "True", "open": "True", "cycle": "True", "bare": "True"
        });
        assert_eq!(coerced(parameters, args.clone()), args);
    }
}
