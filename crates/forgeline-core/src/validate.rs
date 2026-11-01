use crate::CoreError;

pub fn validate_name(name: &str) -> Result<String, CoreError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 80 || name == "." || name == ".." {
        return Err(CoreError::InvalidName);
    }
    if name
        .chars()
        .any(|ch| ch.is_control() || ch == '/' || ch == '\\')
    {
        return Err(CoreError::InvalidName);
    }
    Ok(name.to_string())
}

/// Accept only clone URLs we are willing to pass as a single git argument.
/// Embedded https credentials are rejected so tokens are not stored in SQLite.
pub fn validate_git_url(url: &str) -> Result<(), CoreError> {
    let url = url.trim();
    if url.is_empty() || url.len() > 2000 {
        return Err(CoreError::InvalidGitUrl);
    }
    if url.chars().any(|ch| ch.is_control() || ch.is_whitespace()) {
        return Err(CoreError::InvalidGitUrl);
    }
    if url.contains(['\'', '"', ';', '|', '&', '$', '`', '\\', '<', '>', '(', ')']) {
        return Err(CoreError::InvalidGitUrl);
    }
    if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("ssh://"))
        .or_else(|| url.strip_prefix("git://"))
    {
        let authority = rest.split(['/', '?']).next().unwrap_or("");
        if authority.is_empty() {
            return Err(CoreError::InvalidGitUrl);
        }
        if url.starts_with("https://") && authority.contains('@') {
            return Err(CoreError::InvalidGitUrl);
        }
        return Ok(());
    }
    if let Some((user, rest)) = url.split_once('@') {
        if user == "git" && rest.contains(':') && !rest.starts_with(':') {
            return Ok(());
        }
    }
    Err(CoreError::InvalidGitUrl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_git_urls() {
        assert_eq!(validate_name("  Shop  ").unwrap(), "Shop");
        assert!(validate_name("").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("..").is_err());

        assert!(validate_git_url("https://github.com/org/repo.git").is_ok());
        assert!(validate_git_url("ssh://git@github.com/org/repo.git").is_ok());
        assert!(validate_git_url("git@github.com:org/repo.git").is_ok());
        assert!(validate_git_url("http://github.com/org/repo.git").is_err());
        assert!(validate_git_url("file:///tmp/repo").is_err());
        assert!(validate_git_url("https://user:token@github.com/org/repo.git").is_err());
        assert!(validate_git_url("https://github.com/org/repo.git;rm").is_err());
        assert!(validate_git_url("https://github.com/org/repo.git && true").is_err());
    }
}
