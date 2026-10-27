use serde_json::Value;

use crate::application::control_plane::DccControlPlane;
use crate::application::doctor::DoctorContext;
use crate::application::report;

/// Arguments for `meshy-dcc report`.
#[derive(Debug, clap::Args)]
pub(crate) struct ReportArgs {
    /// What went wrong, in the user's own words (1-4000 characters).
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) summary: String,
    /// Where the report was written; the window passes `ui`.
    #[arg(long, default_value = "cli", value_parser = ["cli", "ui"])]
    pub(crate) source: String,
    /// Send the report to Meshy. Ask the user first: without it only the exact
    /// payload is shown and nothing leaves this computer.
    #[arg(long)]
    pub(crate) yes: bool,
}

impl ReportArgs {
    /// The command that sends exactly what the preview shows.
    fn send_command(&self) -> Vec<String> {
        let mut command: Vec<String> = ["meshy-dcc", "report", "--summary", &self.summary]
            .map(String::from)
            .into();
        if self.source != "cli" {
            command.extend(["--source".to_string(), self.source.clone()]);
        }
        command.push("--yes".to_string());
        command
    }

    pub(crate) async fn run(
        self,
        control: &DccControlPlane,
        doctor: &DoctorContext,
    ) -> anyhow::Result<Value> {
        report::validate_summary(&self.summary)?;
        let payload = report::collect(&self.summary, &self.source, control, doctor).await?;
        if !self.yes {
            return report::preview(&payload, self.send_command());
        }
        Ok(report::send(&payload, report::intake_url().as_deref()).await)
    }
}

/// `failed` and `unknown` end the command with the unavailable exit code.
pub(crate) fn did_not_send(value: &Value) -> bool {
    matches!(value["status"].as_str(), Some("failed" | "unknown"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_send_command_repeats_the_summary_and_source() {
        let mut args = ReportArgs {
            summary: "--yes is broken".into(),
            source: "cli".into(),
            yes: false,
        };
        assert_eq!(
            args.send_command(),
            [
                "meshy-dcc",
                "report",
                "--summary",
                "--yes is broken",
                "--yes"
            ]
        );

        args.source = "ui".into();
        assert_eq!(
            args.send_command(),
            [
                "meshy-dcc",
                "report",
                "--summary",
                "--yes is broken",
                "--source",
                "ui",
                "--yes"
            ]
        );
    }
}
