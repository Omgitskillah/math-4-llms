use math_4_llms::mcp::MathMcpServer;
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = MathMcpServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
