use std::process::Command;

fn main() {
    // Generate version information from git or fallback
    let version = get_version_from_git();
    println!("cargo:rustc-env=COPYPASTE_VERSION={}", version);

    // Re-run build script if git state changes (only if .git exists)
    if std::path::Path::new(".git").exists() {
        println!("cargo:rerun-if-changed=.git/HEAD");
        println!("cargo:rerun-if-changed=.git/refs");
    }
}

fn get_version_from_git() -> String {
    // Try to get git describe output first
    if let Ok(output) = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
    {
        if output.status.success() {
            let describe = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // If we have a tag, use it; otherwise use dev build with commit info
            if describe.starts_with('v') || !describe.contains("-g") {
                return describe;
            } else {
                // For untagged commits, create a dev version
                return format!("0.1.0-dev+{}", describe);
            }
        }
    }

    // Fallback: use commit count + short hash
    if let (Ok(count), Ok(hash)) = (
        Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .output(),
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output(),
    ) {
        if count.status.success() && hash.status.success() {
            let count_str = String::from_utf8_lossy(&count.stdout).trim().to_string();
            let hash_str = String::from_utf8_lossy(&hash.stdout).trim().to_string();
            return format!("0.1.0-dev.{}.{}", count_str, hash_str);
        }
    }

    // Check for environment variables set by Docker
    if let Ok(commit) = std::env::var("GIT_COMMIT") {
        if let Ok(message) = std::env::var("GIT_COMMIT_MESSAGE") {
            if !commit.is_empty() {
                return format!(
                    "0.1.0-dev+{}-{}",
                    commit,
                    message.chars().take(20).collect::<String>()
                );
            }
        }
        if !commit.is_empty() {
            return format!("0.1.0-dev+{}", commit);
        }
    }

    // Final fallback: use static version
    "0.1.0-docker".to_string()
}
