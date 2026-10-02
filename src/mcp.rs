//! Local stdio MCP adapter for the strict calculator.

use crate::{MathResponse, evaluate_response};
use rmcp::{
    Json, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{Implementation, ServerCapabilities, ServerConfig},
    schemars::{self, JsonSchema},
    tool, tool_handler, tool_router,
};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CalculateMathRequest {
    /// One strict scalar expression, without prose, Markdown, or JSON wrapping.
    pub expression: String,
}

#[derive(Clone, Debug, Default)]
pub struct MathMcpServer;

#[tool_router]
impl MathMcpServer {
    /// Evaluate one strict scalar mathematical expression.
    ///
    /// The versioned response includes either a result and approximation flag,
    /// or a stable error code, message, hint, and optional UTF-8 byte position.
    #[tool(
        name = "calculate",
        description = "Evaluate one strict scalar mathematical expression. Send only the expression, without prose, Markdown, LaTeX, or JSON wrapping. Returns a versioned result or stable correction error."
    )]
    fn calculate(
        &self,
        Parameters(CalculateMathRequest { expression }): Parameters<CalculateMathRequest>,
    ) -> Result<Json<MathResponse>, Json<MathResponse>> {
        let response = evaluate_response(&expression);
        if response.ok {
            Ok(Json(response))
        } else {
            Err(Json(response))
        }
    }
}

#[tool_handler]
impl ServerHandler for MathMcpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("math-4-llms", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Call calculate with one strict scalar expression and inspect ok before using the result.",
            )
    }
}
