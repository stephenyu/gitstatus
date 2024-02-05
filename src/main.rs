use git2::{BranchType, Error, Repository};
use std::process;
use std::process::Command;

fn main() {
    let repo = match Repository::discover(".") {
        Ok(repo) => repo,
        Err(_) => {
            println!("No Git repository found.");
            process::exit(1); // Exit with a non-zero status to indicate failure
        }
    };

    let mut components = Vec::new();

    // Get current branch name and add it to components if present
    if let Ok(branch_name) = get_current_branch_name(&repo) {
        components.push(branch_name);
    }

    //// Get upstream branch name and add it to components if present
    if let Ok(upstream_name) = get_upstream_branch_name(&repo) {
        components.push(upstream_name);
    }

    // Get git status message and add it to components if present
    let status_message = get_git_status();
    if !status_message.is_empty() {
        components.push(status_message);
    }

    // Join all components with a space, ensuring no extra spaces if a section is empty
    let output = components.join(" ");
    println!("{}", output);
}

fn get_current_branch_name(repo: &Repository) -> Result<String, Error> {
    let head = repo.head()?;
    if head.is_branch() {
        let shorthand = head.shorthand().unwrap_or("unknown branch");
        Ok(shorthand.to_string())
    } else {
        Ok("HEAD is detached".to_string())
    }
}

fn get_upstream_branch_name(repo: &Repository) -> Result<String, Error> {
    let head = repo.head()?;
    let branch = repo.find_branch(head.shorthand().unwrap_or_default(), BranchType::Local)?;
    let upstream = branch.upstream()?;
    let upstream_name = upstream.name()?.unwrap_or_default();
    Ok(upstream_name.to_string())
}

fn get_git_status() -> String {
    let output = Command::new("git")
        .args(["status", "--porcelain", "-uno"])
        .output();

    match output {
        Ok(output) => {
            if output.stdout.is_empty() {
                "✓".to_string()
            } else {
                parse_git_status_output(String::from_utf8_lossy(&output.stdout))
            }
        }
        Err(_) => "Failed to get status".to_string(),
    }
}

fn parse_git_status_output(output: std::borrow::Cow<str>) -> String {
    let mut updated = 0;
    let mut deleted = 0;
    let mut untracked = 0;

    for line in output.lines() {
        match &line[0..2] {
            " M" => updated += 1,
            " D" => deleted += 1,
            "??" => untracked += 1,
            _ => {}
        }
    }

    let mut message = String::new();
    if updated > 0 {
        message += &format!("+{}", updated);
    }
    if deleted > 0 {
        message += &format!("x{}", deleted);
    }
    if untracked > 0 {
        message += &format!("n{}", untracked);
    }
    if message.is_empty() {
        message = "✓".to_string();
    }

    message
}
