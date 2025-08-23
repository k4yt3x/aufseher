use anyhow::Result;
use reqwest;
use serde::Deserialize;
use serde_json::json;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct OpenAICompletionsResponse {
    id: String,
    object: String,
    created: i64,
    model: String,
    usage: OpenAIUsage,
    choices: Vec<OpenAIChoice>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIChoiceMessage,
    logprobs: Option<bool>,
    finish_reason: String,
    index: i64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct OpenAIChoiceMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAICheckSpamResponse {
    spam: bool,
}

pub async fn openai_check_is_message_spam(message: &str, openai_api_key: &str) -> Result<bool> {
    let response = openai_complete_single_message(message, openai_api_key).await?;
    let parsed_response: Result<OpenAICheckSpamResponse, serde_json::Error> =
        serde_json::from_str(&response);

    match parsed_response {
        Ok(response) => {
            return Ok(response.spam);
        }
        Err(error) => {
            return Err(anyhow::anyhow!(
                "Failed to parse OpenAI response: {}",
                error
            ));
        }
    }
}

async fn openai_complete_single_message(message: &str, openai_api_key: &str) -> Result<String> {
    let request = reqwest::Client::new()
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", openai_api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": "gpt-4o",
            "temperature": 0.7,
            "messages": [
                {
                    "role": "user",
                    "content": format!("Check if the following message is spam. \
                        Spam is defined as any message that contains advertisements \
                        (e.g., crypto currency promotions, selling illegal data) \
                        or any other form of clearly unwanted content. \
                        If you are not sure if the message is spam, classify it as not spam. \
                        Only classify it as spam if you are certain. \
                        Format your reply as a JSON string. \
                        If the message is spam, reply `{{\"spam\": true}}`. \
                        If the message is not spam, reply `{{\"spam\": false}}`. \
                        Reply in plain text. Do not use code blocks. \
                        Message:\n{}", message)
                }
            ]
        }));

    // Send the request and parse the response
    let response = request.send().await?;

    // Check if the response is successful
    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "OpenAI request failed: {}",
            response.status()
        ));
    }

    // Parse the response
    let completion: OpenAICompletionsResponse = response.json().await?;

    // Check if the response has at least one choice
    if completion.choices.is_empty() {
        return Err(anyhow::anyhow!("OpenAI response has no choices"));
    }

    return Ok(completion.choices[0].message.content.clone());
}
