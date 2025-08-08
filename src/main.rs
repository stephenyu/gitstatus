use anyhow::{Context, Result};
use clap::Parser;
use git2::{BranchType, Repository, Status};
use std::path::Path;

#[derive(Parser)]
#[command(
    name = "gitstatus",
    about = "Get concise git repository status information",
    version
)]
struct Args {
    /// Path to the git repository (defaults to current directory)
    #[arg(short, long, default_value = ".")]
    path: String,

    /// Show detailed output
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    match run(&args) {
        Ok(output) => println!("{}", output),
        Err(e) => {
            if args.verbose {
                eprintln!("Error: {:?}", e);
            }
            std::process::exit(1);
        }
    }
}

fn run(args: &Args) -> Result<String> {
    let repo = discover_repository(&args.path).context("Failed to find git repository")?;

    let status = GitStatus::from_repository(&repo)?;
    Ok(status.format())
}

fn discover_repository(path: &str) -> Result<Repository> {
    Repository::discover(Path::new(path)).context("Not a git repository or unable to access")
}

#[derive(Debug)]
struct GitStatus {
    current_branch: String,
    upstream_branch: Option<String>,
    changes: ChangesSummary,
}

#[derive(Debug, Default)]
struct ChangesSummary {
    modified: usize,
    deleted: usize,
    added: usize,
    renamed: usize,
    typechange: usize,
}

impl GitStatus {
    fn from_repository(repo: &Repository) -> Result<Self> {
        let head = repo.head().ok();
        let current_branch = get_current_branch_name(repo, head.as_ref())?;
        let upstream_branch = get_upstream_branch_name(repo, head.as_ref()).ok();
        let changes = get_changes_summary(repo)?;

        Ok(GitStatus {
            current_branch,
            upstream_branch,
            changes,
        })
    }

    fn format(&self) -> String {
        let mut components = Vec::new();

        // Add current branch
        components.push(self.current_branch.clone());

        // Add upstream branch if it exists and is different
        if let Some(ref upstream) = self.upstream_branch {
            if upstream != &self.current_branch {
                components.push(upstream.clone());
            }
        }

        // Add changes summary
        components.push(self.changes.format());

        components.join(" ")
    }
}

impl ChangesSummary {
    fn is_clean(&self) -> bool {
        self.modified == 0
            && self.deleted == 0
            && self.added == 0
            && self.renamed == 0
            && self.typechange == 0
    }

    fn format(&self) -> String {
        if self.is_clean() {
            return "✓".to_string();
        }

        let mut parts = Vec::new();

        if self.added > 0 {
            parts.push(format!("+{}", self.added));
        }
        if self.modified > 0 {
            parts.push(format!("~{}", self.modified));
        }
        if self.deleted > 0 {
            parts.push(format!("-{}", self.deleted));
        }
        if self.renamed > 0 {
            parts.push(format!("r{}", self.renamed));
        }
        if self.typechange > 0 {
            parts.push(format!("t{}", self.typechange));
        }

        parts.join("")
    }
}

fn get_current_branch_name(_repo: &Repository, head: Option<&git2::Reference>) -> Result<String> {
    match head {
        Some(head_ref) => {
            if head_ref.is_branch() {
                let shorthand = head_ref.shorthand().context("Failed to get branch shorthand")?;
                Ok(shorthand.to_string())
            } else {
                Ok("HEAD".to_string()) // Detached HEAD state
            }
        }
        None => {
            // Repository has no HEAD (empty repository)
            Ok("(no branch)".to_string())
        }
    }
}

fn get_upstream_branch_name(repo: &Repository, head: Option<&git2::Reference>) -> Result<String> {
    match head {
        Some(head_ref) => {
            let branch_name = head_ref.shorthand().context("Failed to get branch name")?;

            let branch = repo
                .find_branch(branch_name, BranchType::Local)
                .context("Failed to find local branch")?;

            let upstream = branch.upstream().context("No upstream branch configured")?;

            let upstream_name = upstream
                .name()
                .context("Failed to get upstream branch name")?
                .context("Upstream branch name contains invalid UTF-8")?;

            Ok(upstream_name.to_string())
        }
        None => {
            Err(anyhow::anyhow!("No HEAD reference available"))
        }
    }
}

fn get_changes_summary(repo: &Repository) -> Result<ChangesSummary> {
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)        // Show untracked files
        .recurse_untracked_dirs(false)  // But don't scan inside untracked dirs
        .include_ignored(false);

    let statuses = repo
        .statuses(Some(&mut opts))
        .context("Failed to get repository status")?;

    let mut summary = ChangesSummary::default();

    for entry in statuses.iter() {
        let status = entry.status();

        if status.contains(Status::INDEX_NEW) || status.contains(Status::WT_NEW) {
            summary.added += 1;
        }
        if status.contains(Status::INDEX_MODIFIED) || status.contains(Status::WT_MODIFIED) {
            summary.modified += 1;
        }
        if status.contains(Status::INDEX_DELETED) || status.contains(Status::WT_DELETED) {
            summary.deleted += 1;
        }
        if status.contains(Status::INDEX_RENAMED) {
            summary.renamed += 1;
        }
        if status.contains(Status::INDEX_TYPECHANGE) || status.contains(Status::WT_TYPECHANGE) {
            summary.typechange += 1;
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_changes_summary_clean() {
        let summary = ChangesSummary::default();
        assert!(summary.is_clean());
        assert_eq!(summary.format(), "✓");
    }

    #[test]
    fn test_changes_summary_with_changes() {
        let summary = ChangesSummary {
            added: 2,
            modified: 1,
            deleted: 3,
            renamed: 0,
            typechange: 0,
        };
        assert!(!summary.is_clean());
        assert_eq!(summary.format(), "+2~1-3");
    }

    #[test]
    fn test_changes_summary_all_types() {
        let summary = ChangesSummary {
            added: 1,
            modified: 2,
            deleted: 3,
            renamed: 4,
            typechange: 5,
        };
        assert_eq!(summary.format(), "+1~2-3r4t5");
    }

    #[test]
    fn test_git_status_format_no_upstream() {
        let status = GitStatus {
            current_branch: "main".to_string(),
            upstream_branch: None,
            changes: ChangesSummary::default(),
        };
        assert_eq!(status.format(), "main ✓");
    }

    #[test]
    fn test_git_status_format_with_upstream() {
        let status = GitStatus {
            current_branch: "main".to_string(),
            upstream_branch: Some("origin/main".to_string()),
            changes: ChangesSummary::default(),
        };
        assert_eq!(status.format(), "main origin/main ✓");
    }

    #[test]
    fn test_git_status_format_same_upstream() {
        let status = GitStatus {
            current_branch: "main".to_string(),
            upstream_branch: Some("main".to_string()),
            changes: ChangesSummary::default(),
        };
        // Should not duplicate branch name when upstream is the same
        assert_eq!(status.format(), "main ✓");
    }

    #[test]
    fn test_git_status_format_with_changes() {
        let status = GitStatus {
            current_branch: "feature".to_string(),
            upstream_branch: Some("origin/feature".to_string()),
            changes: ChangesSummary {
                added: 1,
                modified: 2,
                deleted: 0,
                renamed: 0,
                typechange: 0,
            },
        };
        assert_eq!(status.format(), "feature origin/feature +1~2");
    }
}
