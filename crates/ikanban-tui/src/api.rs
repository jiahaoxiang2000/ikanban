use reqwest::Client;
use uuid::Uuid;

use crate::models::{
    ApiResponse, CreateProject, CreateTask, ExecutionProcess, ExecutionProcessLog, Project, Task,
    UpdateProject, UpdateTask,
};

/// HTTP API client for iKanban server
#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Get the base URL for WebSocket connections
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // Project endpoints

    pub async fn list_projects(&self) -> anyhow::Result<Vec<Project>> {
        let url = format!("{}/api/projects", self.base_url);
        let response: ApiResponse<Vec<Project>> = self.client.get(&url).send().await?.json().await?;

        if response.success {
            Ok(response.data.unwrap_or_default())
        } else {
            anyhow::bail!(response.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn create_project(&self, payload: &CreateProject) -> anyhow::Result<Project> {
        let url = format!("{}/api/projects", self.base_url);
        let response: ApiResponse<Project> = self
            .client
            .post(&url)
            .json(payload)
            .send()
            .await?
            .json()
            .await?;

        if response.success {
            response.data.ok_or_else(|| anyhow::anyhow!("No data returned"))
        } else {
            anyhow::bail!(response.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn update_project(&self, id: Uuid, payload: &UpdateProject) -> anyhow::Result<Project> {
        let url = format!("{}/api/projects/{}", self.base_url, id);
        let response: ApiResponse<Project> = self
            .client
            .put(&url)
            .json(payload)
            .send()
            .await?
            .json()
            .await?;

        if response.success {
            response.data.ok_or_else(|| anyhow::anyhow!("No data returned"))
        } else {
            anyhow::bail!(response.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn delete_project(&self, id: Uuid) -> anyhow::Result<()> {
        let url = format!("{}/api/projects/{}", self.base_url, id);
        let response: ApiResponse<()> = self.client.delete(&url).send().await?.json().await?;

        if response.success {
            Ok(())
        } else {
            anyhow::bail!(response.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    // Task endpoints

    pub async fn list_tasks(&self, project_id: Uuid) -> anyhow::Result<Vec<Task>> {
        let url = format!("{}/api/tasks?project_id={}", self.base_url, project_id);
        let response: ApiResponse<Vec<Task>> = self.client.get(&url).send().await?.json().await?;

        if response.success {
            Ok(response.data.unwrap_or_default())
        } else {
            anyhow::bail!(response.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn create_task(&self, payload: &CreateTask) -> anyhow::Result<Task> {
        let url = format!("{}/api/tasks", self.base_url);
        let response: ApiResponse<Task> = self
            .client
            .post(&url)
            .json(payload)
            .send()
            .await?
            .json()
            .await?;

        if response.success {
            response.data.ok_or_else(|| anyhow::anyhow!("No data returned"))
        } else {
            anyhow::bail!(response.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn update_task(&self, id: Uuid, payload: &UpdateTask) -> anyhow::Result<Task> {
        let url = format!("{}/api/tasks/{}", self.base_url, id);
        let response: ApiResponse<Task> = self
            .client
            .put(&url)
            .json(payload)
            .send()
            .await?
            .json()
            .await?;

        if response.success {
            response.data.ok_or_else(|| anyhow::anyhow!("No data returned"))
        } else {
            anyhow::bail!(response.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn delete_task(&self, id: Uuid) -> anyhow::Result<()> {
        let url = format!("{}/api/tasks/{}", self.base_url, id);
        let response: ApiResponse<()> = self.client.delete(&url).send().await?.json().await?;

        if response.success {
            Ok(())
        } else {
            anyhow::bail!(response.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    /// Get WebSocket URL for projects stream
    pub fn projects_ws_url(&self) -> String {
        let ws_base = self.base_url.replace("http://", "ws://").replace("https://", "wss://");
        format!("{}/api/projects/stream/ws", ws_base)
    }

    /// Get WebSocket URL for tasks stream
    pub fn tasks_ws_url(&self, project_id: Uuid) -> String {
        let ws_base = self.base_url.replace("http://", "ws://").replace("https://", "wss://");
        format!("{}/api/tasks/stream/ws?project_id={}", ws_base, project_id)
    }

    /// Get WebSocket URL for execution logs stream
    pub fn execution_logs_ws_url(&self, execution_id: Uuid) -> String {
        let ws_base = self.base_url.replace("http://", "ws://").replace("https://", "wss://");
        format!("{}/api/executions/{}/logs/stream", ws_base, execution_id)
    }

    // Execution endpoints

    pub async fn list_executions(&self, session_id: Uuid) -> anyhow::Result<Vec<ExecutionProcess>> {
        let url = format!("{}/api/sessions/{}/executions", self.base_url, session_id);
        let response: ApiResponse<Vec<ExecutionProcess>> = self.client.get(&url).send().await?.json().await?;

        if response.success {
            Ok(response.data.unwrap_or_default())
        } else {
            anyhow::bail!(response.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn get_execution(&self, id: Uuid) -> anyhow::Result<ExecutionProcess> {
        let url = format!("{}/api/executions/{}", self.base_url, id);
        let response: ApiResponse<ExecutionProcess> = self.client.get(&url).send().await?.json().await?;

        if response.success {
            response.data.ok_or_else(|| anyhow::anyhow!("No data returned"))
        } else {
            anyhow::bail!(response.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    pub async fn stop_execution(&self, id: Uuid) -> anyhow::Result<ExecutionProcess> {
        let url = format!("{}/api/executions/{}/stop", self.base_url, id);
        let response: ApiResponse<ExecutionProcess> = self.client.post(&url).send().await?.json().await?;

        if response.success {
            response.data.ok_or_else(|| anyhow::anyhow!("No data returned"))
        } else {
            anyhow::bail!(response.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }

    // Execution log endpoints

    pub async fn get_execution_logs(&self, execution_id: Uuid, limit: Option<u64>) -> anyhow::Result<Vec<ExecutionProcessLog>> {
        let mut url = format!("{}/api/executions/{}/logs", self.base_url, execution_id);
        if let Some(limit) = limit {
            url.push_str(&format!("?limit={}", limit));
        }
        let response: ApiResponse<Vec<ExecutionProcessLog>> = self.client.get(&url).send().await?.json().await?;

        if response.success {
            Ok(response.data.unwrap_or_default())
        } else {
            anyhow::bail!(response.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
    }
}
