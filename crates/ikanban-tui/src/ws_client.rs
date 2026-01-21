use anyhow::Result;
use futures_util::{SinkExt, stream::StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

use crate::models::{
    CreateProject, CreateTask, ExecutionProcess, ExecutionProcessLog, Project, Session,
    SubscribeTarget, Task, UpdateProject, UpdateTask, WsEvent, WsMessage, WsRequest, WsResponse,
    WsResponseData,
};

/// WebSocket client for all iKanban operations
pub struct WsClient {
    request_tx: mpsc::UnboundedSender<(String, WsRequest, oneshot::Sender<WsResponse>)>,
    event_tx: mpsc::UnboundedSender<WsEvent>,
}

impl WsClient {
    /// Create a new WebSocket client and connect
    pub fn new(server_url: &str, event_tx: mpsc::UnboundedSender<WsEvent>) -> Self {
        let ws_url = server_url
            .replace("http://", "ws://")
            .replace("https://", "wss://")
            + "/api/ws";

        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let event_tx_clone = event_tx.clone();

        tokio::spawn(async move {
            if let Err(e) = Self::run_connection(&ws_url, request_rx, event_tx_clone).await {
                tracing::error!("WebSocket connection error: {}", e);
            }
        });

        Self {
            request_tx,
            event_tx,
        }
    }

    async fn run_connection(
        url: &str,
        mut request_rx: mpsc::UnboundedReceiver<(String, WsRequest, oneshot::Sender<WsResponse>)>,
        event_tx: mpsc::UnboundedSender<WsEvent>,
    ) -> Result<()> {
        let (ws_stream, _) = connect_async(url).await?;
        tracing::info!("WebSocket connected to {}", url);

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Pending responses map
        let pending_responses: Arc<Mutex<HashMap<String, oneshot::Sender<WsResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let pending_clone = pending_responses.clone();

        // Task to handle incoming WebSocket messages
        let recv_task = tokio::spawn(async move {
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                            match ws_msg {
                                WsMessage::Response { id, response } => {
                                    let mut pending = pending_clone.lock().await;
                                    if let Some(tx) = pending.remove(&id) {
                                        let _ = tx.send(response);
                                    }
                                }
                                WsMessage::Event(event) => {
                                    let _ = event_tx.send(event);
                                }
                                WsMessage::Connected => {
                                    tracing::info!("WebSocket connected");
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        tracing::info!("WebSocket closed");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Task to handle outgoing requests
        let send_task = tokio::spawn(async move {
            while let Some((id, request, response_tx)) = request_rx.recv().await {
                {
                    let mut pending = pending_responses.lock().await;
                    pending.insert(id.clone(), response_tx);
                }

                let msg = WsMessage::Request { id, request };
                if let Ok(json) = serde_json::to_string(&msg) {
                    if ws_sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
        });

        tokio::select! {
            _ = recv_task => {},
            _ = send_task => {},
        }

        Ok(())
    }

    async fn send_request(&self, request: WsRequest) -> Result<WsResponse> {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();

        self.request_tx.send((id, request, tx))?;

        match rx.await {
            Ok(response) => Ok(response),
            Err(_) => anyhow::bail!("Request cancelled"),
        }
    }

    // Projects
    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        let response = self.send_request(WsRequest::ListProjects).await?;
        match response {
            WsResponse::Success(WsResponseData::Projects(projects)) => {
                Ok(projects.into_iter().map(|p| p.project).collect())
            }
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
            WsResponse::Error { message } => anyhow::bail!(message),
        }
    }

    pub async fn create_project(&self, payload: CreateProject) -> Result<Project> {
        let response = self.send_request(WsRequest::CreateProject(payload)).await?;
        match response {
            WsResponse::Success(WsResponseData::Project(project)) => Ok(project),
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }

    pub async fn update_project(&self, id: Uuid, data: UpdateProject) -> Result<Project> {
        let response = self
            .send_request(WsRequest::UpdateProject { id, data })
            .await?;
        match response {
            WsResponse::Success(WsResponseData::Project(project)) => Ok(project),
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }

    pub async fn delete_project(&self, id: Uuid) -> Result<()> {
        let response = self.send_request(WsRequest::DeleteProject { id }).await?;
        match response {
            WsResponse::Success(_) => Ok(()),
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }

    // Tasks
    pub async fn list_tasks(&self, project_id: Uuid) -> Result<Vec<Task>> {
        let response = self
            .send_request(WsRequest::ListTasks { project_id })
            .await?;
        match response {
            WsResponse::Success(WsResponseData::Tasks(tasks)) => {
                Ok(tasks.into_iter().map(|t| t.task).collect())
            }
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }

    pub async fn create_task(&self, payload: CreateTask) -> Result<Task> {
        let response = self.send_request(WsRequest::CreateTask(payload)).await?;
        match response {
            WsResponse::Success(WsResponseData::Task(task)) => Ok(task),
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }

    pub async fn update_task(&self, id: Uuid, data: UpdateTask) -> Result<Task> {
        let response = self
            .send_request(WsRequest::UpdateTask { id, data })
            .await?;
        match response {
            WsResponse::Success(WsResponseData::Task(task)) => Ok(task),
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }

    pub async fn delete_task(&self, id: Uuid) -> Result<()> {
        let response = self.send_request(WsRequest::DeleteTask { id }).await?;
        match response {
            WsResponse::Success(_) => Ok(()),
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }

    // Sessions
    pub async fn list_sessions(&self, task_id: Uuid) -> Result<Vec<Session>> {
        let response = self
            .send_request(WsRequest::ListSessions { task_id })
            .await?;
        match response {
            WsResponse::Success(WsResponseData::Sessions(sessions)) => Ok(sessions),
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }

    // Executions
    pub async fn list_executions(&self, session_id: Uuid) -> Result<Vec<ExecutionProcess>> {
        let response = self
            .send_request(WsRequest::ListExecutions { session_id })
            .await?;
        match response {
            WsResponse::Success(WsResponseData::Executions(executions)) => Ok(executions),
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }

    pub async fn get_execution(&self, id: Uuid) -> Result<ExecutionProcess> {
        let response = self.send_request(WsRequest::GetExecution { id }).await?;
        match response {
            WsResponse::Success(WsResponseData::Execution(execution)) => Ok(execution),
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }

    pub async fn stop_execution(&self, id: Uuid) -> Result<ExecutionProcess> {
        let response = self.send_request(WsRequest::StopExecution { id }).await?;
        match response {
            WsResponse::Success(WsResponseData::Execution(execution)) => Ok(execution),
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }

    // Execution Logs
    pub async fn get_execution_logs(
        &self,
        execution_id: Uuid,
        limit: Option<u64>,
    ) -> Result<Vec<ExecutionProcessLog>> {
        let response = self
            .send_request(WsRequest::GetExecutionLogs {
                execution_id,
                limit,
            })
            .await?;
        match response {
            WsResponse::Success(WsResponseData::ExecutionLogs(logs)) => Ok(logs),
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }

    // Subscriptions
    pub async fn subscribe(&self, target: SubscribeTarget) -> Result<()> {
        let response = self.send_request(WsRequest::Subscribe(target)).await?;
        match response {
            WsResponse::Success(_) => Ok(()),
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }

    pub async fn unsubscribe(&self, target: SubscribeTarget) -> Result<()> {
        let response = self.send_request(WsRequest::Unsubscribe(target)).await?;
        match response {
            WsResponse::Success(_) => Ok(()),
            WsResponse::Error { message } => anyhow::bail!(message),
            WsResponse::Success(_) => anyhow::bail!("Unexpected response data type"),
        }
    }
}
