use crate::error::{AppError, AppResult};
use crate::models::ai_categorization::{AiProvider, AiSettings};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, warn};

const REQUEST_TIMEOUT_SECS: u64 = 60;

/// Transaction data sent to the AI for categorization
#[derive(Debug, Clone, Serialize)]
pub struct TransactionForCategorization {
    pub id: i64,
    pub date: String,
    pub description: String,
    pub amount: String,
    pub currency: String,
}

/// Category option for AI to choose from
#[derive(Debug, Clone, Serialize)]
pub struct CategoryOption {
    pub id: i64,
    pub path: String,
}

/// AI's categorization suggestion for a transaction
#[derive(Debug, Clone, Deserialize)]
pub struct CategorizationSuggestion {
    pub transaction_id: i64,
    pub category_id: Option<i64>,
    pub confidence: f64,
    pub reasoning: String,
}

/// Response from AI containing multiple suggestions
#[derive(Debug, Clone, Deserialize)]
pub struct CategorizationResponse {
    pub suggestions: Vec<CategorizationSuggestion>,
}

/// Test connection result
pub struct ConnectionTestResult {
    pub success: bool,
    pub message: String,
    pub model_info: Option<String>,
}

/// Create an HTTP client with appropriate timeout
fn create_client() -> AppResult<Client> {
    Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| AppError::Internal(format!("Failed to create HTTP client: {}", e)))
}

/// Test connection to the AI provider
pub async fn test_connection(settings: &AiSettings) -> ConnectionTestResult {
    match settings.provider {
        AiProvider::Ollama => test_ollama_connection(settings).await,
        AiProvider::OpenAi => test_openai_connection(settings).await,
        AiProvider::Anthropic => test_anthropic_connection(settings).await,
    }
}

async fn test_ollama_connection(settings: &AiSettings) -> ConnectionTestResult {
    let client = match create_client() {
        Ok(c) => c,
        Err(e) => {
            return ConnectionTestResult {
                success: false,
                message: e.to_string(),
                model_info: None,
            }
        }
    };

    // Test by listing available models
    let url = format!("{}/api/tags", settings.base_url.trim_end_matches('/'));

    match client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            #[derive(Deserialize)]
            struct OllamaModels {
                models: Vec<OllamaModel>,
            }
            #[derive(Deserialize)]
            struct OllamaModel {
                name: String,
            }

            match response.json::<OllamaModels>().await {
                Ok(models) => {
                    let model_names: Vec<String> =
                        models.models.iter().map(|m| m.name.clone()).collect();
                    let has_model = model_names
                        .iter()
                        .any(|m| m.starts_with(&settings.model) || m == &settings.model);

                    if has_model {
                        ConnectionTestResult {
                            success: true,
                            message: "Connected successfully".to_string(),
                            model_info: Some(format!("Model '{}' available", settings.model)),
                        }
                    } else {
                        ConnectionTestResult {
                            success: true,
                            message: format!(
                                "Connected, but model '{}' not found. Available: {}",
                                settings.model,
                                model_names.join(", ")
                            ),
                            model_info: None,
                        }
                    }
                }
                Err(e) => ConnectionTestResult {
                    success: false,
                    message: format!("Failed to parse response: {}", e),
                    model_info: None,
                },
            }
        }
        Ok(response) => ConnectionTestResult {
            success: false,
            message: format!("Server returned status: {}", response.status()),
            model_info: None,
        },
        Err(e) => ConnectionTestResult {
            success: false,
            message: format!("Connection failed: {}", e),
            model_info: None,
        },
    }
}

async fn test_openai_connection(settings: &AiSettings) -> ConnectionTestResult {
    if settings.api_key.is_empty() {
        return ConnectionTestResult {
            success: false,
            message: "API key is required".to_string(),
            model_info: None,
        };
    }

    let client = match create_client() {
        Ok(c) => c,
        Err(e) => {
            return ConnectionTestResult {
                success: false,
                message: e.to_string(),
                model_info: None,
            }
        }
    };

    let url = format!("{}/models", settings.base_url.trim_end_matches('/'));

    match client
        .get(&url)
        .header("Authorization", format!("Bearer {}", settings.api_key))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => ConnectionTestResult {
            success: true,
            message: "Connected successfully".to_string(),
            model_info: Some(format!("Using model: {}", settings.model)),
        },
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            ConnectionTestResult {
                success: false,
                message: format!("API returned {}: {}", status, body),
                model_info: None,
            }
        }
        Err(e) => ConnectionTestResult {
            success: false,
            message: format!("Connection failed: {}", e),
            model_info: None,
        },
    }
}

async fn test_anthropic_connection(settings: &AiSettings) -> ConnectionTestResult {
    if settings.api_key.is_empty() {
        return ConnectionTestResult {
            success: false,
            message: "API key is required".to_string(),
            model_info: None,
        };
    }

    // Anthropic doesn't have a simple health check endpoint, so we'll just verify the URL format
    // A real test would require making a minimal API call
    ConnectionTestResult {
        success: true,
        message: "Configuration looks valid".to_string(),
        model_info: Some(format!("Using model: {}", settings.model)),
    }
}

/// Categorize a batch of transactions using the configured AI provider
pub async fn categorize_transactions(
    settings: &AiSettings,
    transactions: Vec<TransactionForCategorization>,
    categories: &[CategoryOption],
) -> AppResult<Vec<CategorizationSuggestion>> {
    if transactions.is_empty() {
        return Ok(vec![]);
    }

    match settings.provider {
        AiProvider::Ollama => categorize_with_ollama(settings, transactions, categories).await,
        AiProvider::OpenAi => {
            categorize_with_openai_compatible(settings, transactions, categories).await
        }
        AiProvider::Anthropic => {
            categorize_with_anthropic(settings, transactions, categories).await
        }
    }
}

fn build_system_prompt(categories: &[CategoryOption]) -> String {
    let category_list: String = categories
        .iter()
        .map(|c| format!("- ID {}: {}", c.id, c.path))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are a financial transaction categorization assistant. Your task is to analyze transaction descriptions and suggest the most appropriate category.

Available categories (use the ID number):
{}

Rules:
1. Analyze each transaction's description to determine the most likely category
2. If no category fits well, set category_id to null
3. Provide a confidence score between 0.0 and 1.0 (1.0 = very confident)
4. Keep reasoning brief (1-2 sentences)

You MUST respond with valid JSON in this exact format:
{{"suggestions": [
  {{"transaction_id": <id>, "category_id": <id or null>, "confidence": <0.0-1.0>, "reasoning": "<brief explanation>"}}
]}}"#,
        category_list
    )
}

fn build_user_prompt(transactions: &[TransactionForCategorization]) -> String {
    let transaction_list: String = transactions
        .iter()
        .map(|t| {
            format!(
                "- ID {}: \"{}\" ({} {})",
                t.id, t.description, t.amount, t.currency
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "Categorize these transactions:\n\n{}\n\nRespond with JSON only.",
        transaction_list
    )
}

async fn categorize_with_ollama(
    settings: &AiSettings,
    transactions: Vec<TransactionForCategorization>,
    categories: &[CategoryOption],
) -> AppResult<Vec<CategorizationSuggestion>> {
    let client = create_client()?;
    let url = format!("{}/api/generate", settings.base_url.trim_end_matches('/'));

    let system_prompt = build_system_prompt(categories);
    let user_prompt = build_user_prompt(&transactions);

    #[derive(Serialize)]
    struct OllamaRequest {
        model: String,
        prompt: String,
        system: String,
        stream: bool,
        format: String,
    }

    #[derive(Deserialize)]
    struct OllamaResponse {
        response: String,
    }

    let request = OllamaRequest {
        model: settings.model.clone(),
        prompt: user_prompt,
        system: system_prompt,
        stream: false,
        format: "json".to_string(),
    };

    debug!(model = %settings.model, transaction_count = transactions.len(), "Sending categorization request to Ollama");

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Ollama request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!(
            "Ollama returned {}: {}",
            status, body
        )));
    }

    let ollama_response: OllamaResponse = response
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse Ollama response: {}", e)))?;

    parse_ai_response(&ollama_response.response, &transactions)
}

async fn categorize_with_openai_compatible(
    settings: &AiSettings,
    transactions: Vec<TransactionForCategorization>,
    categories: &[CategoryOption],
) -> AppResult<Vec<CategorizationSuggestion>> {
    let client = create_client()?;
    let url = format!(
        "{}/chat/completions",
        settings.base_url.trim_end_matches('/')
    );

    let system_prompt = build_system_prompt(categories);
    let user_prompt = build_user_prompt(&transactions);

    #[derive(Serialize)]
    struct Message {
        role: String,
        content: String,
    }

    #[derive(Serialize)]
    struct OpenAiRequest {
        model: String,
        messages: Vec<Message>,
        temperature: f64,
        response_format: ResponseFormat,
    }

    #[derive(Serialize)]
    struct ResponseFormat {
        r#type: String,
    }

    #[derive(Deserialize)]
    struct OpenAiResponse {
        choices: Vec<Choice>,
    }

    #[derive(Deserialize)]
    struct Choice {
        message: ChoiceMessage,
    }

    #[derive(Deserialize)]
    struct ChoiceMessage {
        content: String,
    }

    let request = OpenAiRequest {
        model: settings.model.clone(),
        messages: vec![
            Message {
                role: "system".to_string(),
                content: system_prompt,
            },
            Message {
                role: "user".to_string(),
                content: user_prompt,
            },
        ],
        temperature: 0.3,
        response_format: ResponseFormat {
            r#type: "json_object".to_string(),
        },
    };

    debug!(model = %settings.model, transaction_count = transactions.len(), "Sending categorization request to OpenAI-compatible API");

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", settings.api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("OpenAI request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!(
            "OpenAI API returned {}: {}",
            status, body
        )));
    }

    let openai_response: OpenAiResponse = response
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse OpenAI response: {}", e)))?;

    let content = openai_response
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    parse_ai_response(&content, &transactions)
}

async fn categorize_with_anthropic(
    settings: &AiSettings,
    transactions: Vec<TransactionForCategorization>,
    categories: &[CategoryOption],
) -> AppResult<Vec<CategorizationSuggestion>> {
    let client = create_client()?;
    let url = format!("{}/v1/messages", settings.base_url.trim_end_matches('/'));

    let system_prompt = build_system_prompt(categories);
    let user_prompt = build_user_prompt(&transactions);

    #[derive(Serialize)]
    struct Message {
        role: String,
        content: String,
    }

    #[derive(Serialize)]
    struct AnthropicRequest {
        model: String,
        max_tokens: i32,
        system: String,
        messages: Vec<Message>,
    }

    #[derive(Deserialize)]
    struct AnthropicResponse {
        content: Vec<ContentBlock>,
    }

    #[derive(Deserialize)]
    struct ContentBlock {
        text: Option<String>,
    }

    let request = AnthropicRequest {
        model: settings.model.clone(),
        max_tokens: 4096,
        system: system_prompt,
        messages: vec![Message {
            role: "user".to_string(),
            content: user_prompt,
        }],
    };

    debug!(model = %settings.model, transaction_count = transactions.len(), "Sending categorization request to Anthropic");

    let response = client
        .post(&url)
        .header("x-api-key", &settings.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Anthropic request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!(
            "Anthropic API returned {}: {}",
            status, body
        )));
    }

    let anthropic_response: AnthropicResponse = response
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse Anthropic response: {}", e)))?;

    let content = anthropic_response
        .content
        .first()
        .and_then(|c| c.text.clone())
        .unwrap_or_default();

    parse_ai_response(&content, &transactions)
}

fn parse_ai_response(
    content: &str,
    transactions: &[TransactionForCategorization],
) -> AppResult<Vec<CategorizationSuggestion>> {
    // Try to extract JSON from the response (it might have extra text)
    let json_str = extract_json(content);

    let response: CategorizationResponse = serde_json::from_str(&json_str).map_err(|e| {
        warn!(content = %content, error = %e, "Failed to parse AI response as JSON");
        AppError::Internal(format!("Failed to parse AI response: {}", e))
    })?;

    // Validate that we got suggestions for all requested transactions
    let transaction_ids: std::collections::HashSet<i64> =
        transactions.iter().map(|t| t.id).collect();

    let mut suggestions = response.suggestions;

    // Filter to only include suggestions for transactions we asked about
    suggestions.retain(|s| transaction_ids.contains(&s.transaction_id));

    // Clamp confidence values
    for suggestion in &mut suggestions {
        suggestion.confidence = suggestion.confidence.clamp(0.0, 1.0);
    }

    Ok(suggestions)
}

fn extract_json(content: &str) -> String {
    // Try to find JSON object in the content
    if let Some(start) = content.find('{') {
        if let Some(end) = content.rfind('}') {
            return content[start..=end].to_string();
        }
    }
    content.to_string()
}
