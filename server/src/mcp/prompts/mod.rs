//! MCP *prompts* — server-side, version-controlled workflow templates.
//!
//! Where a tool is something the LLM calls autonomously mid-conversation, a
//! prompt is a user-initiated template: MCP clients (Claude Desktop) surface
//! these as selectable slash-command entries whose arguments render as a form.
//! Hosting the canonical household workflows here means every family member
//! gets the same version as it improves, instead of each person pasting their
//! own drifting copy.
//!
//! Layout mirrors the tool surface: one file per prompt for the renderable
//! body + its snapshot tests; this module holds the thin `#[prompt]` wiring.
//! The generated `prompt_router()` is `pub(crate)` so the `#[prompt_handler]`
//! on `handler.rs`'s `ServerHandler` impl can reach it across modules.

pub mod weekly_dinner_plan;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{GetPromptResult, PromptMessage, PromptMessageRole};
use rmcp::{prompt, prompt_router, ErrorData as McpError};

use super::handler::FewdMcp;
use super::schemas::errors::InputError;
use super::schemas::WeeklyDinnerPlanArgs;

#[prompt_router(vis = "pub(crate)")]
impl FewdMcp {
    /// Plan a week of family dinners end-to-end. Snaps `week_start_date` to its
    /// Monday and renders the canonical workflow; see
    /// [`weekly_dinner_plan::render`] for the body.
    #[prompt(
        name = "weekly_dinner_plan",
        description = "Plan this coming week's family dinners end-to-end. Fill in \
            the week plus your context — schedule, ingredients to use up, season, \
            recipe preference, effort limits — and the assistant proposes a plan, \
            asks questions, then (once you confirm) schedules the meals, builds the \
            grocery list, and makes the fridge printable."
    )]
    async fn weekly_dinner_plan(
        &self,
        Parameters(args): Parameters<WeeklyDinnerPlanArgs>,
    ) -> Result<GetPromptResult, McpError> {
        // `family_schedule` is required, but serde only enforces presence —
        // an empty/whitespace-only value would render a useless blank schedule
        // line. Reject it with the same actionable error other write paths use.
        if args.family_schedule.trim().is_empty() {
            return Err(McpError::invalid_params(
                InputError::EmptyName("family_schedule").to_string(),
                None,
            ));
        }
        let date =
            chrono::NaiveDate::parse_from_str(&args.week_start_date, "%Y-%m-%d").map_err(|_| {
                McpError::invalid_params(
                    InputError::InvalidDate {
                        field: "week_start_date",
                        value: args.week_start_date.clone(),
                    }
                    .to_string(),
                    None,
                )
            })?;
        // `week_bounds` returns None only for dates so near chrono's range
        // limits that the week math would overflow — reject those rather than
        // letting the arithmetic panic.
        let (monday, sunday) = weekly_dinner_plan::week_bounds(date).ok_or_else(|| {
            McpError::invalid_params(
                format!(
                    "week_start_date '{}' is outside the supported calendar range.",
                    args.week_start_date
                ),
                None,
            )
        })?;
        let body = weekly_dinner_plan::render(monday, sunday, &args);

        Ok(
            GetPromptResult::new(vec![PromptMessage::new_text(PromptMessageRole::User, body)])
                .with_description("Weekly family dinner-planning workflow"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The prompt is registered with the expected name and argument
    /// required/optional flags. Mirrors the `tool_router` registration test in
    /// `handler.rs`: catches a prompt silently dropping out of `prompts/list`
    /// or an argument's required-ness flipping.
    #[test]
    fn weekly_dinner_plan_is_registered_with_correct_arg_flags() {
        let prompts = FewdMcp::prompt_router().list_all();
        let prompt = prompts
            .iter()
            .find(|p| p.name == "weekly_dinner_plan")
            .expect("weekly_dinner_plan must be registered in the prompt router");

        let args = prompt
            .arguments
            .as_ref()
            .expect("weekly_dinner_plan must expose arguments");

        let required: Vec<&str> = args
            .iter()
            .filter(|a| a.required == Some(true))
            .map(|a| a.name.as_str())
            .collect();
        let optional: Vec<&str> = args
            .iter()
            .filter(|a| a.required != Some(true))
            .map(|a| a.name.as_str())
            .collect();

        assert!(required.contains(&"week_start_date"));
        assert!(required.contains(&"family_schedule"));
        for opt in [
            "ingredients_to_use_up",
            "style_or_season",
            "recipe_preference",
            "effort_constraints",
        ] {
            assert!(optional.contains(&opt), "{opt} should be optional");
        }
    }
}
