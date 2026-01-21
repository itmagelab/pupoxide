use crate::domain::report::{ResourceReport, ResourceStatus};
use colored::*;

pub struct PrettyFormatter;

impl PrettyFormatter {
    pub fn display(reports: &[ResourceReport], show_unchanged: bool) {
        if reports.is_empty() {
            println!("\n{}", "No resources to apply.".yellow());
            return;
        }

        let filtered_reports: Vec<_> = reports
            .iter()
            .filter(|r| show_unchanged || r.status != ResourceStatus::Unchanged)
            .collect();

        if filtered_reports.is_empty() && !reports.is_empty() {
            println!(
                "\n{}",
                "No changes detected. All resources are already in the desired state."
                    .green()
                    .bold()
            );
            println!("(Use --show-unchanged to see all resources)");
            return;
        }

        println!("\n{}", "Catalog Application Summary:".bold().underline());
        println!("{:-<60}", "");

        let mut applied = 0;
        let mut unchanged = 0;
        let mut failed = 0;
        let mut would_apply = 0;

        for report in reports {
            if !show_unchanged && report.status == ResourceStatus::Unchanged {
                unchanged += 1;
                continue;
            }

            let status_str = match report.status {
                ResourceStatus::Applied => {
                    applied += 1;
                    "SUCCESS".green().bold()
                }
                ResourceStatus::Unchanged => {
                    unchanged += 1;
                    "UNCHANGED".cyan()
                }
                ResourceStatus::Failed => {
                    failed += 1;
                    "FAILED".red().bold()
                }
                ResourceStatus::Skipped => "SKIPPED".yellow(),
                ResourceStatus::WouldApply => {
                    would_apply += 1;
                    "WOULD APPLY".blue().bold()
                }
            };

            let padding = ".".repeat(60_usize.saturating_sub(report.resource_id.len() + 12));
            print!("{} {} ", report.resource_id, padding);
            println!("[{}]", status_str);

            if let Some(msg) = &report.message {
                println!("   {}", msg.red());
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
}
