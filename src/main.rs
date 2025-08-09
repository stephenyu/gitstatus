use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use gix::Repository;
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

    /// Control untracked files handling (e.g., -uno == --untracked no)
    /// Values: no | normal | all
    #[arg(short = 'u', long = "untracked", value_enum)]
    untracked: Option<UntrackedArg>,

    /// Show all files including untracked files (overrides default behavior)
    #[arg(long)]
    all: bool,
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

    let status = GitStatus::from_repository(&repo, args.untracked, args.all)?;
    Ok(status.format())
}

fn discover_repository(path: &str) -> Result<Repository> {
    gix::discover(Path::new(path)).context("Not a git repository or unable to access")
}

#[derive(Debug)]
struct GitStatus {
    current_branch: String,
    upstream_branch: Option<String>,
    changes: ChangesSummary,
}

#[derive(Debug, Default)]
struct ChangesSummary {
    staged: usize,
    modified: usize,
    deleted: usize,
    renamed: usize,
    untracked: usize,
}

impl GitStatus {
    fn from_repository(repo: &Repository, untracked: Option<UntrackedArg>, all: bool) -> Result<Self> {
        let current_branch = get_current_branch_name(repo)?;
        let upstream_branch = get_upstream_branch_name(repo).ok();
        let changes = get_changes_summary(repo, untracked, all)?;

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
        self.staged == 0
            && self.modified == 0
            && self.deleted == 0
            && self.renamed == 0
            && self.untracked == 0
    }

    fn format(&self) -> String {
        if self.is_clean() {
            return "✓".to_string();
        }

        let mut parts = Vec::new();

        if self.staged > 0 {
            parts.push(format!("^{}", self.staged));
        }
        if self.renamed > 0 {
            parts.push(format!("~{}", self.renamed));
        }
        if self.modified > 0 {
            parts.push(format!("~{}", self.modified));
        }
        if self.deleted > 0 {
            parts.push(format!("-{}", self.deleted));
        }
        if self.untracked > 0 {
            parts.push(format!("+{}", self.untracked));
        }

        parts.join("")
    }
}

fn get_current_branch_name(repo: &Repository) -> Result<String> {
    match repo.head() {
        Ok(head_ref) => {
            match head_ref.referent_name() {
                Some(refname) => {
                    // Extract branch name from refs/heads/branch_name
                    let branch_name = refname.shorten();
                    Ok(branch_name.to_string())
                }
                None => {
                    // Detached HEAD state
                    Ok("HEAD".to_string())
                }
            }
        }
        Err(_) => {
            // Repository has no HEAD (empty repository)
            Ok("(no branch)".to_string())
        }
    }
}

fn get_upstream_branch_name(repo: &Repository) -> Result<String> {
    let head = repo.head()?;
    let tracking_branch = head.into_remote(gix::remote::Direction::Fetch).transpose()?;

    match tracking_branch {
        Some(branch) => match branch.name() {
            Some(name) => Ok(name.as_bstr().to_string()),
            None => Err(anyhow::anyhow!("Upstream branch name is not valid UTF-8")),
        },
        None => Err(anyhow::anyhow!(
            "No upstream branch configured for current branch"
        )),
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum UntrackedArg {
    No,
    Normal,
    All,
}

fn get_changes_summary(repo: &Repository, untracked: Option<UntrackedArg>, all: bool) -> Result<ChangesSummary> {
    let mut summary = ChangesSummary::default();
    
    // Use git command to get proper staged vs unstaged distinction
    let repo_path = repo.workdir().unwrap_or(repo.path());
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(&["status", "--porcelain"])
        .output()
        .context("Failed to execute git status")?;
    
    if !output.status.success() {
        return Ok(summary);
    }
    
    let status_output = String::from_utf8_lossy(&output.stdout);
    
    for line in status_output.lines() {
        if line.len() < 2 {
            continue;
        }
        
        let chars: Vec<char> = line.chars().collect();
        let index_status = chars[0];
        let worktree_status = chars[1];
        
        // Count staged changes (index status is not space or ?)
        match index_status {
            'A' | 'M' | 'D' | 'R' | 'C' => summary.staged += 1,
            _ => {}
        }
        
        // Count working directory changes
        match worktree_status {
            'M' => summary.modified += 1,
            'D' => summary.deleted += 1,
            _ => {}
        }
        
        // Count untracked files (both index and worktree status are ?)
        if index_status == '?' && worktree_status == '?' {
            let should_include_untracked = match untracked {
                Some(UntrackedArg::No) => false,
                Some(UntrackedArg::Normal) | Some(UntrackedArg::All) => true,
                None => all,
            };
            
            if should_include_untracked {
                summary.untracked += 1;
            }
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
            staged: 0,
            modified: 1,
            deleted: 3,
            renamed: 0,
            untracked: 0,
        };
        assert!(!summary.is_clean());
        assert_eq!(summary.format(), "~1-3");
    }

    #[test]
    fn test_changes_summary_all_types() {
        let summary = ChangesSummary {
            staged: 1,
            modified: 2,
            deleted: 3,
            renamed: 4,
            untracked: 5,
        };
        assert_eq!(summary.format(), "^1~4~2-3+5");
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
                staged: 0,
                modified: 2,
                deleted: 0,
                renamed: 0,
                untracked: 0,
            },
        };
        assert_eq!(status.format(), "feature origin/feature ~2");
    }

    #[test]
    fn test_changes_summary_one_modified_one_deleted() {
        let summary = ChangesSummary {
            staged: 0,
            modified: 1,
            deleted: 1,
            renamed: 0,
            untracked: 0,
        };
        assert_eq!(summary.format(), "~1-1");
    }
}
