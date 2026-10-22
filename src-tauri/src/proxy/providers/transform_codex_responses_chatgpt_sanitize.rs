//! ChatGPT Codex backend sanitization for native Responses passthrough.
//!
//! Official Codex (`chatgpt.com/backend-api/codex`) is not the public
//! `api.openai.com` Responses API. Its request struct rejects OpenAI-platform
//! fields with `Unsupported parameter: <name>` (HTTP 400). Factory Droid and
//! other BYOK clients still emit those fields when they match a built-in GPT
//! profile (notably `safety_identifier`).
//!
//! The Claude → Responses Codex OAuth transform already drops a subset of
//! these (`max_output_tokens`, `temperature`, `top_p`). Native official
//! passthrough forwards the body verbatim, so the same scrub has to live here.
//! Deterministic field removals only — no semantic rewriting.

use serde_json::Value;

/// Top-level fields the ChatGPT Codex backend rejects. Probed 2026-08-18
/// against `chatgpt.com/backend-api/codex` via the official provider.
const CHATGPT_CODEX_UNSUPPORTED_FIELDS: &[&str] = &[
    "safety_identifier",
    "max_output_tokens",
    "temperature",
    "top_p",
    "prompt_cache_retention",
    "user",
    "metadata",
];

/// Strip ChatGPT-Codex-unsupported top-level fields from a native Responses
/// request. Returns whether anything changed. Idempotent.
pub(crate) fn sanitize_chatgpt_codex_responses_request(body: &mut Value) -> bool {
    let Some(obj) = body.as_object_mut() else {
        return false;
    };

    let mut changed = false;
    for field in CHATGPT_CODEX_UNSUPPORTED_FIELDS {
        if obj.remove(*field).is_some() {
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strips_droid_safety_identifier_and_keeps_codex_fields() {
        let mut body = json!({
            "model": "gpt-5.6-sol",
            "input": [{"role": "user", "content": "hi"}],
            "stream": true,
            "store": false,
            "safety_identifier": "user-123",
            "max_output_tokens": 128000,
            "prompt_cache_retention": "24h",
            "prompt_cache_key": "sess-1",
            "reasoning": {"effort": "medium"},
            "parallel_tool_calls": true
        });

        assert!(sanitize_chatgpt_codex_responses_request(&mut body));
        assert!(body.get("safety_identifier").is_none());
        assert!(body.get("max_output_tokens").is_none());
        assert!(body.get("prompt_cache_retention").is_none());
        assert_eq!(body["model"], "gpt-5.6-sol");
        assert_eq!(body["stream"], true);
        assert_eq!(body["store"], false);
        assert_eq!(body["prompt_cache_key"], "sess-1");
        assert_eq!(body["reasoning"]["effort"], "medium");
        assert_eq!(body["parallel_tool_calls"], true);
    }

    #[test]
    fn strips_sampling_and_openai_platform_fields() {
        let mut body = json!({
            "model": "gpt-5.6-luna",
            "temperature": 1,
            "top_p": 1,
            "user": "abc",
            "metadata": {"x": "y"},
            "service_tier": "priority",
            "text": {"verbosity": "low"}
        });

        assert!(sanitize_chatgpt_codex_responses_request(&mut body));
        assert!(body.get("temperature").is_none());
        assert!(body.get("top_p").is_none());
        assert!(body.get("user").is_none());
        assert!(body.get("metadata").is_none());
        assert_eq!(body["service_tier"], "priority");
        assert_eq!(body["text"]["verbosity"], "low");
    }

    #[test]
    fn is_idempotent_and_ignores_non_objects() {
        let mut body = json!({"model": "gpt-5.6-sol"});
        assert!(!sanitize_chatgpt_codex_responses_request(&mut body));

        let mut again = json!({"safety_identifier": "x"});
        assert!(sanitize_chatgpt_codex_responses_request(&mut again));
        assert!(!sanitize_chatgpt_codex_responses_request(&mut again));

        let mut not_obj = json!(["safety_identifier"]);
        assert!(!sanitize_chatgpt_codex_responses_request(&mut not_obj));
    }
}
