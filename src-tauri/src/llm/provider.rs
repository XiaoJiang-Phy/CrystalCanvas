//! [Overview: Provider-Agnostic LLM HTTP Client]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use async_trait::async_trait;
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum ProviderConfig {
    OpenAi { api_key: String, model: String },
    DeepSeek { api_key: String, model: String },
    Claude { api_key: String, model: String },
    Gemini { api_key: String, model: String },
    Ollama { model: String },
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<String, String>;
}

/// Helper function to extract text from a JSON structure based on a path of string keys
fn extract_json_text(val: &serde_json::Value, paths: &[&str]) -> Option<String> {
    let mut current = val;
    for &p in paths {
        if let Some(next) = current.get(p) {
            current = next;
        } else if let Some(arr) = current.as_array() {
            if let Ok(idx) = p.parse::<usize>() {
                if let Some(next) = arr.get(idx) {
                    current = next;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else {
            return None;
        }
    }
    current.as_str().map(|s| s.to_string())
}

// =========================================================================
// Provider Implementations
// =========================================================================

pub struct OpenAiCompatibleProvider {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub client: Client,
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<String, String> {
        let payload = serde_json::json!({
            "model": self.model,
            "messages": messages,
            // Use standard parameters for reasoning models if needed, though most
            // reasoning models ignore temp/top_p. We omit them for safety.
        });

        let res = self
            .client
            .post(&self.base_url)
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = res.status();
        let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;

        if !status.is_success() {
            return Err(format!("API Error ({}): {:?}", status, body));
        }

        extract_json_text(&body, &["choices", "0", "message", "content"])
            .ok_or_else(|| "Failed to parse response".to_string())
    }
}

pub struct AnthropicProvider {
    pub api_key: String,
    pub model: String,
    pub client: Client,
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<String, String> {
        // Claude /v1/messages API expects:
        // System message at the top level, messages without system role in array.
        let mut system_text = String::new();
        let mut claude_msgs = Vec::new();

        for m in messages {
            if m.role == "system" {
                system_text.push_str(&m.content);
                system_text.push('\n');
            } else {
                claude_msgs.push(serde_json::json!({
                    "role": m.role,
                    "content": m.content
                }));
            }
        }

        let payload = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "system": system_text,
            "messages": claude_msgs
        });

        let mut headers = header::HeaderMap::new();
        headers.insert(
            "x-api-key",
            header::HeaderValue::from_str(&self.api_key)
                .map_err(|e| format!("Invalid API key header value: {}", e))?,
        );
        headers.insert(
            "anthropic-version",
            header::HeaderValue::from_static("2023-06-01"),
        );
        headers.insert(
            "content-type",
            header::HeaderValue::from_static("application/json"),
        );

        let res = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = res.status();
        let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;

        if !status.is_success() {
            return Err(format!("Anthropic API Error ({}): {:?}", status, body));
        }

        extract_json_text(&body, &["content", "0", "text"])
            .ok_or_else(|| "Failed to parse response".to_string())
    }
}

pub struct GeminiProvider {
    pub api_key: String,
    pub model: String,
    pub client: Client,
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<String, String> {
        // Gemini URL pattern: https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={API_KEY}
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        // Convert ChatMessage to Gemini format
        // System instructions map to "system_instruction". User/Assistant map to "user"/"model".
        let mut system_instruction = String::new();
        let mut gemini_contents = Vec::new();

        for m in messages {
            if m.role == "system" {
                system_instruction.push_str(&m.content);
                system_instruction.push('\n');
            } else {
                let role = if m.role == "assistant" {
                    "model"
                } else {
                    "user"
                };
                gemini_contents.push(serde_json::json!({
                    "role": role,
                    "parts": [{ "text": m.content }]
                }));
            }
        }

        let payload = serde_json::json!({
            "system_instruction": {
                "parts": [{ "text": system_instruction }]
            },
            "contents": gemini_contents
        });

        let res = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = res.status();
        let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;

        if !status.is_success() {
            return Err(format!("Gemini API Error ({}): {:?}", status, body));
        }

        // Response structure: candidates[0].content.parts[0].text
        extract_json_text(&body, &["candidates", "0", "content", "parts", "0", "text"])
            .ok_or_else(|| "Failed to parse response".to_string())
    }
}

pub struct OllamaProvider {
    pub model: String,
    pub client: Client,
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<String, String> {
        let payload = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": false
        });

        let res = self
            .client
            .post("http://localhost:11434/api/chat")
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = res.status();
        let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;

        if !status.is_success() {
            return Err(format!("Ollama API Error ({}): {:?}", status, body));
        }

        extract_json_text(&body, &["message", "content"])
            .ok_or_else(|| "Failed to parse response".to_string())
    }
}

/// Factory function to create a provider instance from configuration
pub fn create_provider(config: &ProviderConfig) -> Box<dyn LlmProvider> {
    let client = Client::new();
    match config {
        ProviderConfig::OpenAi { api_key, model } => Box::new(OpenAiCompatibleProvider {
            base_url: "https://api.openai.com/v1/chat/completions".to_string(),
            api_key: api_key.clone(),
            model: model.clone(),
            client,
        }),
        ProviderConfig::DeepSeek { api_key, model } => Box::new(OpenAiCompatibleProvider {
            base_url: "https://api.deepseek.com/chat/completions".to_string(),
            api_key: api_key.clone(),
            model: model.clone(),
            client,
        }),
        ProviderConfig::Claude { api_key, model } => Box::new(AnthropicProvider {
            api_key: api_key.clone(),
            model: model.clone(),
            client,
        }),
        ProviderConfig::Gemini { api_key, model } => Box::new(GeminiProvider {
            api_key: api_key.clone(),
            model: model.clone(),
            client,
        }),
        ProviderConfig::Ollama { model } => Box::new(OllamaProvider {
            model: model.clone(),
            client,
        }),
    }
}
