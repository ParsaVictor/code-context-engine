use crate::traits::{
    BoxFuture, ChunkStream, CompletionChunk, ModelInfo, Provider, ProviderCapabilities,
    ProviderRequest, ProviderResponse, Usage,
};
use neuromesh_core::{Result, TokenCounter};

pub struct MockProvider {
    response_template: String,
}

impl MockProvider {
    pub fn new(response_template: impl Into<String>) -> Self {
        Self {
            response_template: response_template.into(),
        }
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self {
            response_template: "Successfully executed task with optimized neural context.".into(),
        }
    }
}

impl Provider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn send<'a>(&'a self, request: &'a ProviderRequest) -> BoxFuture<'a, Result<ProviderResponse>> {
        Box::pin(async move {
            let prompt_text: String = request
                .messages
                .iter()
                .map(|m| m.content.as_str())
                .collect();
            let prompt_tokens = TokenCounter::count_tokens(&prompt_text);
            // The QA-judge path (tasks without a `verify` command) sends two
            // requests: one asking for an answer, one asking a grader for a
            // PASS/FAIL verdict. A static template can't play both roles, so
            // branch on the judge prompt's fixed instruction line — anything
            // else gets the configured template, exactly as before.
            let reply = if prompt_text.contains("Reply with exactly one line: PASS or FAIL") {
                "PASS — mock judge always agrees".to_string()
            } else {
                self.response_template.clone()
            };
            let completion_tokens = TokenCounter::count_tokens(&reply);

            Ok(ProviderResponse {
                id: "mock-cmpl-1".to_string(),
                model: request.model.clone(),
                content: reply,
                usage: Usage {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens: prompt_tokens + completion_tokens,
                },
                finish_reason: Some("stop".to_string()),
            })
        })
    }

    fn stream<'a>(&'a self, _request: &'a ProviderRequest) -> BoxFuture<'a, Result<ChunkStream>> {
        Box::pin(async move {
            let chunk = CompletionChunk {
                id: "mock-chunk-1".to_string(),
                delta: self.response_template.clone(),
                finish_reason: Some("stop".to_string()),
            };
            Ok(Box::pin(futures_util::stream::once(async move { Ok(chunk) })) as ChunkStream)
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_streaming: true,
            supports_vision: true,
            supports_function_calling: true,
            context_window_max: 128000,
        }
    }

    fn model_info(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "mock-model".into(),
            display_name: "Mock Provider".into(),
            context_window: 128000,
            supports_streaming: true,
            supports_tools: true,
        }]
    }
}
