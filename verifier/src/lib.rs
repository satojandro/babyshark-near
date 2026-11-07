//! BabyShark verifier — pure logic, no I/O.
//!
//! The binary (`main.rs`) runs inside OutLayer's Intel TDX enclave, fetches
//! GitHub over HTTPS and hands the JSON to these functions. Keeping the logic
//! here means it is unit-tested natively, and the attested WASM contains
//! exactly the code these tests exercise.

use serde::{Deserialize, Serialize};

/// What the escrow contract asks the verifier to check.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct VerifyRequest {
    /// GitHub repository, `owner/name`.
    pub repo: String,
    /// GitHub login whose commits count (the kid's handle, pinned on-chain).
    pub github_login: String,
    /// ISO-8601 UTC timestamp; only commits at or after this count.
    pub since: String,
}

/// The verifier's answer. Facts only — the contract decides what they are worth.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerifyReport {
    pub repo: String,
    pub github_login: String,
    pub since: String,
    /// Commits in the window whose GitHub author is `github_login`.
    pub commits_by_login: u32,
    /// Newest qualifying commit, if any.
    pub latest_sha: Option<String>,
    /// Up to 20 qualifying commit SHAs, newest first (the evidence).
    pub commit_shas: Vec<String>,
}

const MAX_EVIDENCE: usize = 20;

#[derive(Debug, PartialEq)]
pub enum InputError {
    BadJson(String),
    BadRepo,
    BadLogin,
    BadSince,
}

impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputError::BadJson(e) => write!(f, "input is not valid JSON: {e}"),
            InputError::BadRepo => write!(f, "repo must be owner/name"),
            InputError::BadLogin => write!(f, "github_login must be a GitHub username"),
            InputError::BadSince => write!(f, "since must be an ISO-8601 UTC timestamp like 2026-11-01T00:00:00Z"),
        }
    }
}

/// Parse and validate the request. Everything here ends up in a URL, so it is
/// checked strictly: no query-string or path injection into the GitHub call.
pub fn parse_request(raw: &str) -> Result<VerifyRequest, InputError> {
    let req: VerifyRequest =
        serde_json::from_str(raw).map_err(|e| InputError::BadJson(e.to_string()))?;
    if !valid_repo(&req.repo) {
        return Err(InputError::BadRepo);
    }
    if !valid_login(&req.github_login) {
        return Err(InputError::BadLogin);
    }
    if !valid_since(&req.since) {
        return Err(InputError::BadSince);
    }
    Ok(req)
}

fn valid_repo(repo: &str) -> bool {
    let mut parts = repo.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let ok = |s: &str| {
        !s.is_empty()
            && s.len() <= 100
            && s != "."
            && s != ".."
            && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    ok(owner) && ok(name)
}

fn valid_login(login: &str) -> bool {
    !login.is_empty()
        && login.len() <= 39
        && !login.starts_with('-')
        && login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Accepts exactly `YYYY-MM-DDTHH:MM:SSZ`.
fn valid_since(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 20
        && b.iter().enumerate().all(|(i, &c)| match i {
            4 | 7 => c == b'-',
            10 => c == b'T',
            13 | 16 => c == b':',
            19 => c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

/// GitHub REST URL for one page of commits in the window by this author.
pub fn commits_url(req: &VerifyRequest) -> String {
    format!(
        "https://api.github.com/repos/{}/commits?author={}&since={}&per_page=100",
        req.repo, req.github_login, req.since
    )
}

#[derive(Deserialize)]
struct ApiCommit {
    sha: String,
    author: Option<ApiUser>,
}

#[derive(Deserialize)]
struct ApiUser {
    login: String,
}

/// Turn GitHub's `/commits` JSON into a report.
///
/// GitHub's `author=` filter also matches by email, so the login is checked
/// again here: a commit only counts if GitHub attributes it to `github_login`.
pub fn summarize(req: &VerifyRequest, commits_json: &str) -> Result<VerifyReport, String> {
    let commits: Vec<ApiCommit> =
        serde_json::from_str(commits_json).map_err(|e| format!("unexpected GitHub response: {e}"))?;
    let shas: Vec<String> = commits
        .into_iter()
        .filter(|c| {
            c.author
                .as_ref()
                .is_some_and(|a| a.login.eq_ignore_ascii_case(&req.github_login))
        })
        .map(|c| c.sha)
        .collect();
    Ok(VerifyReport {
        repo: req.repo.clone(),
        github_login: req.github_login.clone(),
        since: req.since.clone(),
        commits_by_login: shas.len() as u32,
        latest_sha: shas.first().cloned(),
        commit_shas: shas.into_iter().take(MAX_EVIDENCE).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> VerifyRequest {
        VerifyRequest {
            repo: "mateo/candy-tycoon".into(),
            github_login: "mateo".into(),
            since: "2026-11-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn parses_valid_request() {
        let raw = r#"{"repo":"mateo/candy-tycoon","github_login":"mateo","since":"2026-11-01T00:00:00Z"}"#;
        assert_eq!(parse_request(raw).unwrap(), req());
    }

    #[test]
    fn rejects_injection_in_repo() {
        for bad in ["mateo", "mateo/a/b", "mateo/a?x=1", "../etc", "mateo/..", "/x", "a b/c"] {
            let raw = format!(r#"{{"repo":"{bad}","github_login":"mateo","since":"2026-11-01T00:00:00Z"}}"#);
            assert_eq!(parse_request(&raw), Err(InputError::BadRepo), "{bad}");
        }
    }

    #[test]
    fn rejects_bad_login_and_since() {
        let raw = r#"{"repo":"a/b","github_login":"mateo&author=x","since":"2026-11-01T00:00:00Z"}"#;
        assert_eq!(parse_request(raw), Err(InputError::BadLogin));
        let raw = r#"{"repo":"a/b","github_login":"mateo","since":"2026-11-01"}"#;
        assert_eq!(parse_request(raw), Err(InputError::BadSince));
        let raw = r#"{"repo":"a/b","github_login":"mateo","since":"2026-11-01T00:00:00Z&x"}"#;
        assert_eq!(parse_request(raw), Err(InputError::BadSince));
    }

    #[test]
    fn builds_url() {
        assert_eq!(
            commits_url(&req()),
            "https://api.github.com/repos/mateo/candy-tycoon/commits?author=mateo&since=2026-11-01T00:00:00Z&per_page=100"
        );
    }

    #[test]
    fn counts_only_commits_attributed_to_login() {
        let json = r#"[
            {"sha":"c3","author":{"login":"Mateo"}},
            {"sha":"c2","author":{"login":"dad"}},
            {"sha":"c1","author":null},
            {"sha":"c0","author":{"login":"mateo"}}
        ]"#;
        let r = summarize(&req(), json).unwrap();
        assert_eq!(r.commits_by_login, 2);
        assert_eq!(r.latest_sha.as_deref(), Some("c3"));
        assert_eq!(r.commit_shas, vec!["c3", "c0"]);
    }

    #[test]
    fn empty_window_is_zero_not_error() {
        let r = summarize(&req(), "[]").unwrap();
        assert_eq!(r.commits_by_login, 0);
        assert_eq!(r.latest_sha, None);
    }

    #[test]
    fn caps_evidence_list() {
        let items: Vec<String> = (0..30)
            .map(|i| format!(r#"{{"sha":"s{i}","author":{{"login":"mateo"}}}}"#))
            .collect();
        let r = summarize(&req(), &format!("[{}]", items.join(","))).unwrap();
        assert_eq!(r.commits_by_login, 30);
        assert_eq!(r.commit_shas.len(), 20);
    }

    #[test]
    fn github_error_object_is_an_error() {
        assert!(summarize(&req(), r#"{"message":"Not Found"}"#).is_err());
    }
}
