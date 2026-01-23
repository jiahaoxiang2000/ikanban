use std::{io, sync::Arc, time::Duration};

use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use super::types::{OpencodeExecutorEvent, SdkEvent};
use crate::executor::msg_store::{LogMsg, MsgStore};

/// Configuration for running an OpenCode session
#[derive(Clone)]
pub struct RunConfig {
    pub base_url: String,
    pub directory: String,
    pub prompt: String,
    pub resume_session_id: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    healthy: bool,
    #[allow(dead_code)]
    version: String,
}

#[derive(Debug, Deserialize)]
struct SessionResponse {
    id: String,
}

#[derive(Debug, Serialize)]
struct PromptRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<ModelSpec>,
    parts: Vec<TextPartInput>,
}

#[derive(Debug, Serialize, Clone)]
struct ModelSpec {
    #[serde(rename = "providerID")]
    provider_id: String,
    #[serde(rename = "modelID")]
    model_id: String,
}

#[derive(Debug, Serialize)]
struct TextPartInput {
    r#type: &'static str,
    text: String,
}

#[derive(Debug, Clone)]
enum ControlEvent {
    Idle,
    SessionError { message: String },
    Disconnected,
}

/// Run an OpenCode session with the given configuration
///
/// This spawns the session, connects to the event stream, sends the prompt,
/// and waits for completion or interruption.
pub async fn run_session(
    config: RunConfig,
    msg_store: Arc<MsgStore>,
    interrupt_rx: oneshot::Receiver<()>,
) -> Result<(), anyhow::Error> {
    let client = reqwest::Client::builder()
        .default_headers(build_default_headers(&config.directory))
        .build()?;

    let mut interrupted = false;
    let mut interrupt_rx = Some(interrupt_rx);

    // Wait for server health
    tokio::select! {
        res = wait_for_health(&client, &config.base_url) => res?,
        _ = async {
            if let Some(rx) = interrupt_rx.take() {
                let _ = rx.await;
            }
        } => {
            interrupted = true;
        }
    }

    if interrupted {
        return Ok(());
    }

    // Create or fork session
    let session_id = match config.resume_session_id.as_deref() {
        Some(existing) => {
            tokio::select! {
                res = fork_session(&client, &config.base_url, &config.directory, existing) => res?,
                _ = async {
                    if let Some(rx) = interrupt_rx.take() {
                        let _ = rx.await;
                    }
                } => {
                    interrupted = true;
                    return Ok(());
                }
            }
        }
        None => {
            tokio::select! {
                res = create_session(&client, &config.base_url, &config.directory) => res?,
                _ = async {
                    if let Some(rx) = interrupt_rx.take() {
                        let _ = rx.await;
                    }
                } => {
                    interrupted = true;
                    return Ok(());
                }
            }
        }
    };

    // Log session start
    msg_store.push(LogMsg::SessionId(session_id.clone()));
    msg_store.push(LogMsg::Event(
        serde_json::to_string(&OpencodeExecutorEvent::SessionStart {
            session_id: session_id.clone(),
        })
        .unwrap(),
    ));

    let model = config.model.as_deref().and_then(parse_model);

    let (control_tx, mut control_rx) = mpsc::unbounded_channel::<ControlEvent>();

    // Connect to event stream
    let event_resp = tokio::select! {
        res = connect_event_stream(&client, &config.base_url, &config.directory, None) => res?,
        _ = async {
            if let Some(rx) = interrupt_rx.take() {
                let _ = rx.await;
            }
        } => {
            interrupted = true;
            return Ok(());
        }
    };

    // Spawn event listener
    let event_handle = tokio::spawn(spawn_event_listener(
        EventListenerConfig {
            client: client.clone(),
            base_url: config.base_url.clone(),
            directory: config.directory.clone(),
            session_id: session_id.clone(),
            msg_store: msg_store.clone(),
            control_tx,
        },
        event_resp,
    ));

    // Send prompt and wait for completion
    let prompt_result = run_prompt_with_control(
        SessionRequestContext {
            client: &client,
            base_url: &config.base_url,
            directory: &config.directory,
            session_id: &session_id,
        },
        &config.prompt,
        model,
        &mut control_rx,
        &mut interrupt_rx,
    )
    .await;

    if interrupted || interrupt_rx.is_none() {
        send_abort(&client, &config.base_url, &config.directory, &session_id).await;
        event_handle.abort();
        return Ok(());
    }

    event_handle.abort();

    prompt_result?;
    msg_store.push(LogMsg::Event(
        serde_json::to_string(&OpencodeExecutorEvent::Done).unwrap(),
    ));
    msg_store.push(LogMsg::Finished);

    Ok(())
}

fn build_default_headers(directory: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Ok(value) = reqwest::header::HeaderValue::from_str(directory) {
        headers.insert("x-opencode-directory", value);
    }
    headers
}

struct SessionRequestContext<'a> {
    client: &'a reqwest::Client,
    base_url: &'a str,
    directory: &'a str,
    session_id: &'a str,
}

async fn run_prompt_with_control(
    ctx: SessionRequestContext<'_>,
    prompt_text: &str,
    model: Option<ModelSpec>,
    control_rx: &mut mpsc::UnboundedReceiver<ControlEvent>,
    interrupt_rx: &mut Option<oneshot::Receiver<()>>,
) -> Result<(), anyhow::Error> {
    let mut idle_seen = false;
    let mut session_error: Option<String> = None;

    let mut prompt_fut = Box::pin(prompt(
        ctx.client,
        ctx.base_url,
        ctx.directory,
        ctx.session_id,
        prompt_text,
        model,
    ));

    let prompt_result = loop {
        tokio::select! {
            _ = async {
                if let Some(rx) = interrupt_rx.take() {
                    let _ = rx.await;
                }
            } => return Ok(()),
            res = &mut prompt_fut => break res,
            event = control_rx.recv() => match event {
                Some(ControlEvent::SessionError { message }) => {
                    if let Some(existing) = &mut session_error {
                        existing.push('\n');
                        existing.push_str(&message);
                    } else {
                        session_error = Some(message);
                    }
                }
                Some(ControlEvent::Disconnected) if interrupt_rx.is_some() => {
                    return Err(anyhow::anyhow!("OpenCode event stream disconnected while prompt was running"));
                }
                Some(ControlEvent::Disconnected) => return Ok(()),
                Some(ControlEvent::Idle) => idle_seen = true,
                None => {}
            }
        }
    };

    if interrupt_rx.is_none() {
        return Ok(());
    }

    prompt_result?;

    if !idle_seen {
        // Wait for session.idle to capture tail updates
        loop {
            tokio::select! {
                _ = async {
                    if let Some(rx) = interrupt_rx.take() {
                        let _ = rx.await;
                    }
                } => return Ok(()),
                event = control_rx.recv() => match event {
                    Some(ControlEvent::Idle) | None => break,
                    Some(ControlEvent::SessionError { message }) => {
                        if let Some(existing) = &mut session_error {
                            existing.push('\n');
                            existing.push_str(&message);
                        } else {
                            session_error = Some(message);
                        }
                    }
                    Some(ControlEvent::Disconnected) if interrupt_rx.is_some() => {
                        return Err(anyhow::anyhow!(
                            "OpenCode event stream disconnected while waiting for session to go idle"
                        ));
                    }
                    Some(ControlEvent::Disconnected) => return Ok(()),
                }
            }
        }
    }

    if let Some(message) = session_error {
        if interrupt_rx.is_none() {
            return Ok(());
        }
        return Err(anyhow::anyhow!(message));
    }

    Ok(())
}

async fn wait_for_health(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<(), anyhow::Error> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut last_err: Option<String> = None;

    loop {
        if tokio::time::Instant::now() > deadline {
            return Err(anyhow::anyhow!(
                "Timed out waiting for OpenCode server health: {}",
                last_err.unwrap_or_else(|| "unknown error".to_string())
            ));
        }

        let resp = client.get(format!("{base_url}/global/health")).send().await;
        match resp {
            Ok(resp) => {
                if !resp.status().is_success() {
                    last_err = Some(format!("HTTP {}", resp.status()));
                } else if let Ok(body) = resp.json::<HealthResponse>().await {
                    if body.healthy {
                        return Ok(());
                    }
                    last_err = Some(format!("unhealthy server (version {})", body.version));
                } else {
                    last_err = Some("failed to parse health response".to_string());
                }
            }
            Err(err) => {
                last_err = Some(err.to_string());
            }
        }

        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

async fn create_session(
    client: &reqwest::Client,
    base_url: &str,
    directory: &str,
) -> Result<String, anyhow::Error> {
    let resp = client
        .post(format!("{base_url}/session"))
        .query(&[("directory", directory)])
        .json(&serde_json::json!({}))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "OpenCode session.create failed: HTTP {}",
            resp.status()
        ));
    }

    let session = resp.json::<SessionResponse>().await?;
    Ok(session.id)
}

async fn fork_session(
    client: &reqwest::Client,
    base_url: &str,
    directory: &str,
    session_id: &str,
) -> Result<String, anyhow::Error> {
    let resp = client
        .post(format!("{base_url}/session/{session_id}/fork"))
        .query(&[("directory", directory)])
        .json(&serde_json::json!({}))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "OpenCode session.fork failed: HTTP {}",
            resp.status()
        ));
    }

    let session = resp.json::<SessionResponse>().await?;
    Ok(session.id)
}

async fn prompt(
    client: &reqwest::Client,
    base_url: &str,
    directory: &str,
    session_id: &str,
    prompt: &str,
    model: Option<ModelSpec>,
) -> Result<(), anyhow::Error> {
    let req = PromptRequest {
        model,
        parts: vec![TextPartInput {
            r#type: "text",
            text: prompt.to_string(),
        }],
    };

    let resp = client
        .post(format!("{base_url}/session/{session_id}/message"))
        .query(&[("directory", directory)])
        .json(&req)
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "OpenCode session.prompt failed: HTTP {status} {body}"
        ));
    }

    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!(
            "OpenCode session.prompt returned empty response body"
        ));
    }

    let parsed: Value = serde_json::from_str(trimmed)?;

    // Success response: { info, parts }
    if parsed.get("info").is_some() && parsed.get("parts").is_some() {
        return Ok(());
    }

    // Error response: { name, data }
    if let Some(name) = parsed.get("name").and_then(Value::as_str) {
        let message = parsed
            .pointer("/data/message")
            .and_then(Value::as_str)
            .unwrap_or(trimmed);
        return Err(anyhow::anyhow!(
            "OpenCode session.prompt failed: {name}: {message}"
        ));
    }

    Err(anyhow::anyhow!(
        "OpenCode session.prompt returned unexpected response: {trimmed}"
    ))
}

async fn send_abort(client: &reqwest::Client, base_url: &str, directory: &str, session_id: &str) {
    let request = client
        .post(format!("{base_url}/session/{session_id}/abort"))
        .query(&[("directory", directory)]);

    let _ = tokio::time::timeout(Duration::from_millis(800), async move {
        let resp = request.send().await;
        if let Ok(resp) = resp {
            // Drain body
            let _ = resp.bytes().await;
        }
    })
    .await;
}

fn parse_model(model: &str) -> Option<ModelSpec> {
    let (provider_id, model_id) = match model.split_once('/') {
        Some((provider, rest)) => (provider.to_string(), rest.to_string()),
        None => (model.to_string(), String::new()),
    };

    Some(ModelSpec {
        provider_id,
        model_id,
    })
}

async fn connect_event_stream(
    client: &reqwest::Client,
    base_url: &str,
    directory: &str,
    last_event_id: Option<&str>,
) -> Result<reqwest::Response, anyhow::Error> {
    let mut req = client
        .get(format!("{base_url}/event"))
        .header(reqwest::header::ACCEPT, "text/event-stream")
        .query(&[("directory", directory)]);

    if let Some(last_event_id) = last_event_id {
        req = req.header("Last-Event-ID", last_event_id);
    }

    let resp = req.send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read response body>".to_string());
        return Err(anyhow::anyhow!(
            "OpenCode event stream failed: HTTP {status} {body}"
        ));
    }

    Ok(resp)
}

struct EventListenerConfig {
    client: reqwest::Client,
    base_url: String,
    directory: String,
    session_id: String,
    msg_store: Arc<MsgStore>,
    control_tx: mpsc::UnboundedSender<ControlEvent>,
}

async fn spawn_event_listener(config: EventListenerConfig, initial_resp: reqwest::Response) {
    let EventListenerConfig {
        client,
        base_url,
        directory,
        session_id,
        msg_store,
        control_tx,
    } = config;

    let mut last_event_id: Option<String> = None;
    let mut base_retry_delay = Duration::from_millis(3000);
    let mut attempt: u32 = 0;
    let max_attempts: u32 = 20;
    let mut resp: Option<reqwest::Response> = Some(initial_resp);

    loop {
        let current_resp = match resp.take() {
            Some(r) => {
                attempt = 0;
                r
            }
            None => {
                match connect_event_stream(&client, &base_url, &directory, last_event_id.as_deref())
                    .await
                {
                    Ok(r) => {
                        attempt = 0;
                        r
                    }
                    Err(err) => {
                        msg_store.push(LogMsg::Event(
                            serde_json::to_string(&OpencodeExecutorEvent::Error {
                                message: format!("OpenCode event stream reconnect failed: {err}"),
                            })
                            .unwrap(),
                        ));
                        attempt += 1;
                        if attempt >= max_attempts {
                            let _ = control_tx.send(ControlEvent::Disconnected);
                            return;
                        }

                        tokio::time::sleep(exponential_backoff(base_retry_delay, attempt)).await;
                        continue;
                    }
                }
            }
        };

        let outcome = process_event_stream(
            EventStreamContext {
                client: &client,
                base_url: &base_url,
                directory: &directory,
                session_id: &session_id,
                msg_store: &msg_store,
                control_tx: &control_tx,
                base_retry_delay: &mut base_retry_delay,
                last_event_id: &mut last_event_id,
            },
            current_resp,
        )
        .await;

        match outcome {
            Ok(EventStreamOutcome::Idle) | Ok(EventStreamOutcome::Terminal) => return,
            Ok(EventStreamOutcome::Disconnected) | Err(_) => {
                attempt += 1;
                if attempt >= max_attempts {
                    let _ = control_tx.send(ControlEvent::Disconnected);
                    return;
                }
            }
        }

        tokio::time::sleep(exponential_backoff(base_retry_delay, attempt)).await;
        resp = None;
    }
}

fn exponential_backoff(base: Duration, attempt: u32) -> Duration {
    let exp = attempt.saturating_sub(1).min(10);
    let mult = 1u32 << exp;
    base.checked_mul(mult)
        .unwrap_or(Duration::from_secs(30))
        .min(Duration::from_secs(30))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventStreamOutcome {
    Idle,
    #[allow(dead_code)]
    Terminal,
    Disconnected,
}

struct EventStreamContext<'a> {
    #[allow(dead_code)]
    client: &'a reqwest::Client,
    #[allow(dead_code)]
    base_url: &'a str,
    #[allow(dead_code)]
    directory: &'a str,
    session_id: &'a str,
    msg_store: &'a Arc<MsgStore>,
    control_tx: &'a mpsc::UnboundedSender<ControlEvent>,
    base_retry_delay: &'a mut Duration,
    last_event_id: &'a mut Option<String>,
}

async fn process_event_stream(
    ctx: EventStreamContext<'_>,
    resp: reqwest::Response,
) -> Result<EventStreamOutcome, anyhow::Error> {
    let mut stream = resp.bytes_stream().eventsource();

    while let Some(evt) = stream.next().await {
        let evt = evt.map_err(|err| anyhow::anyhow!(io::Error::other(err)))?;

        if !evt.id.trim().is_empty() {
            *ctx.last_event_id = Some(evt.id.trim().to_string());
        }
        if let Some(retry) = evt.retry {
            *ctx.base_retry_delay = retry;
        }

        let trimmed = evt.data.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(data) = serde_json::from_str::<Value>(trimmed) else {
            ctx.msg_store.push(LogMsg::Event(
                serde_json::to_string(&OpencodeExecutorEvent::Error {
                    message: format!(
                        "OpenCode event stream delivered non-JSON event payload: {trimmed}"
                    ),
                })
                .unwrap(),
            ));
            continue;
        };

        let Some(event_type) = data.get("type").and_then(Value::as_str) else {
            continue;
        };

        if !event_matches_session(event_type, &data, ctx.session_id) {
            continue;
        }

        ctx.msg_store.push(LogMsg::Event(
            serde_json::to_string(&OpencodeExecutorEvent::SdkEvent {
                event: data.clone(),
            })
            .unwrap(),
        ));

        match event_type {
            "session.idle" => {
                let _ = ctx.control_tx.send(ControlEvent::Idle);
                return Ok(EventStreamOutcome::Idle);
            }
            "session.error" => {
                let message = data
                    .pointer("/properties/error/data/message")
                    .or_else(|| data.pointer("/properties/error/message"))
                    .and_then(Value::as_str)
                    .unwrap_or("OpenCode session error")
                    .to_string();

                let _ = ctx.control_tx.send(ControlEvent::SessionError { message });
            }
            _ => {}
        }
    }

    Ok(EventStreamOutcome::Disconnected)
}

fn event_matches_session(event_type: &str, event: &Value, session_id: &str) -> bool {
    let extracted = match event_type {
        "message.updated" => event
            .pointer("/properties/info/sessionID")
            .and_then(Value::as_str),
        "message.part.updated" => event
            .pointer("/properties/part/sessionID")
            .and_then(Value::as_str),
        "permission.asked" | "permission.replied" | "session.idle" | "session.error" => event
            .pointer("/properties/sessionID")
            .and_then(Value::as_str),
        _ => event
            .pointer("/properties/sessionID")
            .and_then(Value::as_str)
            .or_else(|| {
                event
                    .pointer("/properties/info/sessionID")
                    .and_then(Value::as_str)
            })
            .or_else(|| {
                event
                    .pointer("/properties/part/sessionID")
                    .and_then(Value::as_str)
            }),
    };

    extracted == Some(session_id)
}
