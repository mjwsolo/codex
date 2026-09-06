use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::plan_spec::create_update_plan_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::config_types::ModeKind;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::plan_tool::UpdatePlanArgs;
use codex_protocol::protocol::EventMsg;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde_json::Value as JsonValue;

pub struct PlanHandler;

pub struct PlanToolOutput {
    /// localcode: appended when the model closes a 3+ item plan with no verification step.
    note: String,
}

const PLAN_UPDATED_MESSAGE: &str = "Plan updated";
const LOCALCODE_VERIFY_NOTE: &str = "\n\nNOTE: you just marked a 3+ item plan fully done and none of the items was a verification step. Before you write your final summary, run the project's build/typecheck/tests (or a focused smoke check) to confirm the work actually runs, and re-read the user's original request: every requirement must be implemented for real — no stubs, placeholders or 'demo only' pieces. Reopen anything partial as a plan item. Do not claim it works without verifying.";

fn localcode_verify_note(plan: &[codex_protocol::plan_tool::PlanItemArg]) -> String {
    use codex_protocol::plan_tool::StepStatus;
    if plan.len() < 3 || !plan.iter().all(|i| matches!(i.status, StepStatus::Completed)) {
        return String::new();
    }
    let text = plan.iter().map(|i| i.step.to_lowercase()).collect::<Vec<_>>().join(" ");
    let mentions_verification = ["verif", "test", "build", "typecheck", "smoke", "run the app", "check"]
        .iter()
        .any(|k| text.contains(k));
    if mentions_verification { String::new() } else { LOCALCODE_VERIFY_NOTE.to_string() }
}

impl ToolOutput for PlanToolOutput {
    fn log_output(&self) -> String {
        format!("{PLAN_UPDATED_MESSAGE}{}", self.note)
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        let mut output = FunctionCallOutputPayload::from_text(format!("{PLAN_UPDATED_MESSAGE}{}", self.note));
        output.success = Some(true);

        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output,
        }
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        JsonValue::Object(serde_json::Map::new())
    }
}

impl ToolExecutor<ToolInvocation> for PlanHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("update_plan")
    }

    fn spec(&self) -> ToolSpec {
        create_update_plan_tool()
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(self.handle_call(invocation))
    }
}

impl PlanHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id: _,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "update_plan handler received unsupported payload".to_string(),
                ));
            }
        };

        if turn.mode() == ModeKind::Plan {
            return Err(FunctionCallError::RespondToModel(
                "update_plan is a TODO/checklist tool and is not allowed in Plan mode".to_string(),
            ));
        }

        let args = parse_update_plan_arguments(&arguments)?;
        session.set_latest_plan(args.plan.clone()).await;
        let note = localcode_verify_note(&args.plan);
        session
            .send_event(turn.as_ref(), EventMsg::PlanUpdate(args))
            .await;

        Ok(boxed_tool_output(PlanToolOutput { note }))
    }
}

impl CoreToolRuntime for PlanHandler {
    fn is_builtin_control_tool(&self) -> bool {
        true
    }
}

fn parse_update_plan_arguments(arguments: &str) -> Result<UpdatePlanArgs, FunctionCallError> {
    serde_json::from_str::<UpdatePlanArgs>(arguments).map_err(|e| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e}"))
    })
}
