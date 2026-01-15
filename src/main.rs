use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, ErrorData as McpError, Implementation, ProtocolVersion,
    ServerCapabilities, ServerInfo,
};
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    id: usize,
    title: String,
    description: String,
    completed: bool,
}

#[derive(Debug, Clone)]
struct TaskManager {
    tasks: Arc<Mutex<Vec<Task>>>,
    next_id: Arc<Mutex<usize>>, // Counter for generating unique task IDs
    tool_router: ToolRouter<TaskManager>,
}

#[tool_router]
impl TaskManager {
    fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Add a new task to the task manager")]
    async fn add_task(
        &self,
        Parameters(AddTaskRequest { title, description }): Parameters<AddTaskRequest>,
    ) -> Result<CallToolResult, McpError> {
        let mut tasks = self.tasks.lock().await;
        let mut next_id = self.next_id.lock().await;

        let task = Task {
            id: *next_id,
            title: title.clone(),
            description,
            completed: false,
        };

        *next_id += 1;
        tasks.push(task.clone());

        let response = serde_json::json!({
            "success": true,
            "task": task,
            "message": format!("Task '{}' added successfully with ID {}", title, task.id)
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap(),
        )]))
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct AddTaskRequest {
    #[schemars(description = "The title of the task")]
    title: String,
    #[schemars(description = "A detailed description of the task")]
    description: String,
}

#[tool_handler]
impl ServerHandler for TaskManager {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_03_26,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "task-manager".to_string(),
                version: "0.1.0".to_string(),
                title: None,
                website_url: None,
                icons: None,
            },
            instructions: Some(
                "A task manager MCP server that allows you to add, complete, list, and retrieve tasks with real-time updates."
                    .to_string(),
            ),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let service = StreamableHttpService::new(
        || Ok(TaskManager::new()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let tcp_listener = tokio::net::TcpListener::bind("127.0.0.1:8001").await?;

    tracing::info!("Server ready at http://127.0.0.1:8001/mcp");

    axum::serve(tcp_listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.unwrap();
        })
        .await?;

    Ok(())
}
