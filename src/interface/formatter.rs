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

        let mut breadcrumbs_str = String::new();
        if let Some(ctx) = &report.source_context {
            let mut parts = Vec::new();
            if let Some(role) = &ctx.role {
                parts.push(role.clone());
            }
            if let Some(profile) = &ctx.profile {
                parts.push(profile.clone());
            }
            if let Some(module) = &ctx.module {
                parts.push(module.clone());
            }

            if !parts.is_empty() {
                breadcrumbs_str = format!("[{}] ", parts.join("::")).dimmed().to_string();
            }
        }

        // Adjust padding considering breadcrumbs (using visible length for better estimation)
        // Since we use dimmed(), the underlying string has ANSI codes.
        // We'll estimate based on names length.
        let bc_len = report
            .source_context
            .as_ref()
            .map(|ctx| {
                let mut l = 2; // []
                if let Some(r) = &ctx.role {
                    l += r.len();
                }
                if let Some(p) = &ctx.profile {
                    l += if l > 2 { 2 } else { 0 } + p.len();
                }
                if let Some(m) = &ctx.module {
                    l += if l > 2 { 2 } else { 0 } + m.len();
                }
                l + 1 // space
            })
            .unwrap_or(0);

        let padding = ".".repeat(75_usize.saturating_sub(report.resource_id.len() + 12 + bc_len));
        let duration_ms = report.duration.as_millis();
        let duration_str = if duration_ms < 1000 {
            format!("({}ms)", duration_ms)
        } else {
            format!("({:.2}s)", report.duration.as_secs_f64())
        };

        let mut line = format!(
            "{}{} {} [{}] {}",
            breadcrumbs_str,
            report.resource_id,
            padding,
            status_str,
            duration_str.dimmed()
        );

        if let (&ResourceStatus::Failed, Some(msg)) = (&report.status, &report.message) {
            if let Some((first_line, rest)) = msg.split_once('\n') {
                let system_msg = first_line.strip_suffix(':').unwrap_or(first_line);
                line.push_str(&format!("\n   {}\n{}", system_msg.red().bold(), rest.red()));
            } else {
                line.push_str(&format!("\n   {}", msg.red().bold()));
            }
        }
        line
    }

    pub fn print_summary(reports: &[ResourceReport], total_duration: std::time::Duration) {
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
            "Summary: {} applied, {} unchanged, {} failed{} (Total: {:.2}s)",
            applied,
            unchanged,
            failed,
            if would_apply > 0 {
                format!(", {} would apply", would_apply)
            } else {
                "".to_string()
            },
            total_duration.as_secs_f64()
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
