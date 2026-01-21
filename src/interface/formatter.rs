use crate::domain::report::{ResourceReport, ResourceStatus};
use colored::*;

pub struct PrettyFormatter;

impl PrettyFormatter {
    pub fn print_header() {
        println!("\n{}", "Catalog Application Summary:".bold().underline());
        println!("{:-<60}", "");
    }

    pub fn format_line(report: &ResourceReport) -> String {
        let status_str = match report.status {
            ResourceStatus::Applied => "SUCCESS".green().bold(),
            ResourceStatus::Unchanged => "UNCHANGED".cyan(),
            ResourceStatus::Failed => "FAILED".red().bold(),
            ResourceStatus::Skipped => "SKIPPED".yellow(),
            ResourceStatus::WouldApply => "WOULD APPLY".blue().bold(),
        };

        let padding = ".".repeat(60_usize.saturating_sub(report.resource_id.len() + 12));
        let mut line = format!("{} {} [{}]", report.resource_id, padding, status_str);

        if let Some(msg) = &report.message {
            line.push_str(&format!("\n   {}", msg.red()));
        }
        line
    }

    pub fn print_summary(reports: &[ResourceReport]) {
        let mut applied = 0;
        let mut unchanged = 0;
        let mut failed = 0;
        let mut would_apply = 0;

        for r in reports {
            match r.status {
                ResourceStatus::Applied => applied += 1,
                ResourceStatus::Unchanged => unchanged += 1,
                ResourceStatus::Failed => failed += 1,
                ResourceStatus::WouldApply => would_apply += 1,
                _ => {}
            }
        }

        println!("{:-<60}", "");
        let summary = format!(
            "Summary: {} applied, {} unchanged, {} failed{}",
            applied,
            unchanged,
            failed,
            if would_apply > 0 {
                format!(", {} would apply", would_apply)
            } else {
                "".to_string()
            }
        );

        if failed > 0 {
            println!("{}", summary.red().bold());
        } else {
            println!("{}", summary.green().bold());
        }
        println!();
    }

    /// Legend for when no changes are detected
    pub fn print_no_changes() {
        println!(
            "\n{}",
            "No changes detected. All resources are already in the desired state."
                .green()
                .bold()
        );
        println!("(Use --show-unchanged to see all resources)");
    }
}
