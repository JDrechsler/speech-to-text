use std::time::Duration;

use reqwest::{Method, RequestBuilder};
use serde_json::{Map, Value, json};

use super::{ModelApi, RewriteError, RewriteModel, instructions};
use crate::profiles::ActiveProfile;
use crate::secrets;

pub(super) const ANTHROPIC_VERSION: &str = "2023-06-01";
const REPLY_TIMEOUT: Duration = Duration::from_secs(60);
const LISTING_TIMEOUT: Duration = Duration::from_secs(15);

pub async fn rewrite(transcript: &str, profile: &ActiveProfile) -> Result<String, RewriteError> {
    let Some(model) = &profile.rewriter else {
        return Err(RewriteError::NoModel);
    };
    rewrite_with(model, stored_key(model).as_deref(), transcript, profile).await
}

pub async fn available_models(model: &RewriteModel) -> Result<Vec<String>, RewriteError> {
    models_listed_by(model, stored_key(model).as_deref()).await
}

fn stored_key(model: &RewriteModel) -> Option<String> {
    secrets::read(&model.key_id())
}

pub(super) async fn rewrite_with(
    model: &RewriteModel,
    key: Option<&str>,
    transcript: &str,
    profile: &ActiveProfile,
) -> Result<String, RewriteError> {
    let input = format!("<dictation>\n{transcript}\n</dictation>");
    let output = respond(model, key, &instructions(profile), &input).await?;
    let rewritten = output.trim();
    if rewritten.is_empty() {
        return Err(RewriteError::Empty(model.name.clone()));
    }
    Ok(rewritten.to_string())
}

pub(super) async fn respond(
    model: &RewriteModel,
    key: Option<&str>,
    instructions: &str,
    input: &str,
) -> Result<String, RewriteError> {
    if !model.is_configured() {
        return Err(RewriteError::NotConfigured(model.name.clone()));
    }
    let (path, body) = match model.api {
        ModelApi::Anthropic => (
            "/messages",
            json!({
                "model": model.model,
                "max_tokens": 4096,
                "system": instructions,
                "messages": [{"role": "user", "content": input}],
            }),
        ),
        ModelApi::OpenAi => (
            "/chat/completions",
            json!({
                "model": model.model,
                "messages": [
                    {"role": "system", "content": instructions},
                    {"role": "user", "content": input},
                ],
            }),
        ),
    };
    let request = authorized_request(model, key, Method::POST, path, REPLY_TIMEOUT)?.json(&body);
    let reply = send(request, model).await?;
    let text = match model.api {
        ModelApi::Anthropic => anthropic_text(&reply),
        ModelApi::OpenAi => openai_text(&reply),
    };
    text.map(|text| without_reasoning(&text).to_string())
        .ok_or_else(|| RewriteError::BadResponse(model.name.clone()))
}

pub(super) async fn models_listed_by(model: &RewriteModel, key: Option<&str>) -> Result<Vec<String>, RewriteError> {
    let path = match model.api {
        ModelApi::Anthropic => "/models?limit=1000",
        ModelApi::OpenAi => "/models",
    };
    let request = authorized_request(model, key, Method::GET, path, LISTING_TIMEOUT)?;
    let reply = send(request, model).await?;
    let entries = reply
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| RewriteError::BadResponse(model.name.clone()))?;
    let mut ids: Vec<String> = entries
        .iter()
        .filter_map(|entry| entry.get("id")?.as_str().map(String::from))
        .collect();
    ids.sort();
    Ok(ids)
}

fn authorized_request(
    model: &RewriteModel,
    key: Option<&str>,
    method: Method,
    path: &str,
    timeout: Duration,
) -> Result<RequestBuilder, RewriteError> {
    let url = model
        .endpoint(path)
        .ok_or_else(|| RewriteError::NotConfigured(model.name.clone()))?;
    if url.scheme() == "http" && !model.is_local() {
        return Err(RewriteError::InsecureAddress(model.name.clone()));
    }
    let client = http_client(model).map_err(|error| network_error(error, model))?;
    let request = client.request(method, url).timeout(timeout);
    Ok(match (model.api, key) {
        (ModelApi::Anthropic, None) => request.header("anthropic-version", ANTHROPIC_VERSION),
        (ModelApi::Anthropic, Some(key)) => request
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("x-api-key", key),
        (ModelApi::OpenAi, None) => request,
        (ModelApi::OpenAi, Some(key)) => request.bearer_auth(key),
    })
}

async fn send(request: RequestBuilder, model: &RewriteModel) -> Result<Map<String, Value>, RewriteError> {
    let response = request
        .send()
        .await
        .map_err(|error| transport_error(error, model))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(RewriteError::Server { name: model.name.clone(), status });
    }
    let body = response
        .bytes()
        .await
        .map_err(|error| transport_error(error, model))?;
    match serde_json::from_slice(&body) {
        Ok(Value::Object(reply)) => Ok(reply),
        _ => Err(RewriteError::BadResponse(model.name.clone())),
    }
}

fn http_client(model: &RewriteModel) -> reqwest::Result<reqwest::Client> {
    let builder = reqwest::Client::builder();
    let builder = if model.is_local() { builder.no_proxy() } else { builder };
    builder.build()
}

fn transport_error(error: reqwest::Error, model: &RewriteModel) -> RewriteError {
    if error.is_timeout() {
        RewriteError::TimedOut(model.name.clone())
    } else if error.is_connect() {
        RewriteError::Unreachable { name: model.name.clone(), destination: model.destination() }
    } else {
        network_error(error, model)
    }
}

fn network_error(error: reqwest::Error, model: &RewriteModel) -> RewriteError {
    let error = error.without_url();
    let detail = match std::error::Error::source(&error) {
        Some(cause) => format!("{error}: {cause}"),
        None => error.to_string(),
    };
    RewriteError::Network { name: model.name.clone(), detail }
}

fn openai_text(reply: &Map<String, Value>) -> Option<String> {
    let message = reply.get("choices")?.get(0)?.get("message")?;
    message.get("content")?.as_str().map(String::from)
}

fn anthropic_text(reply: &Map<String, Value>) -> Option<String> {
    let blocks = reply.get("content")?.as_array()?;
    let text_blocks = blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"));
    Some(text_blocks.filter_map(|block| block.get("text")?.as_str()).collect())
}

fn without_reasoning(text: &str) -> &str {
    const END: &str = "</think>";
    match text.find(END) {
        Some(end) if text.starts_with("<think>") => &text[end + END.len()..],
        _ => text,
    }
}
