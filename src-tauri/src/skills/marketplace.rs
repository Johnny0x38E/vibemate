//! Browse and install skills from the skills.sh catalog.
//!
//! Search uses the documented JSON API. Leaderboards read the public HTML pages
//! and extract the embedded skill objects; that adapter is isolated so a site
//! redesign fails with a clear error instead of inventing rows. Only public
//! GitHub `owner/repo` sources are installable; site-only hosts are refused.
//! Install reuses the Git acquisition path at `HEAD` for the named skill.

#[cfg(test)]
mod tests;

use super::SkillLibrary;
use super::install::{self, preview_skill_install};
use super::types::{
    GitInstallSource, MarketplaceBoard, MarketplaceInstallRequest, MarketplaceSkill, SkillError,
    SkillInstallPreview, SkillInstallReport, SkillInstallSource,
};
use super::validation;
use crate::storage::Storage;
use serde::Deserialize;
use std::collections::HashSet;
use std::time::Duration;

const SEARCH_URL: &str = "https://skills.sh/api/search";
const USER_AGENT: &str = "vibemate/0.1 (+https://github.com/johnnyhuang/vibemate)";
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_SEARCH_LIMIT: usize = 60;
const MAX_LEADERBOARD: usize = 100;
const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;
/// Marketplace installs follow the repository default branch tip.
const MARKETPLACE_REVISION: &str = "HEAD";

impl MarketplaceBoard {
    fn url(self) -> &'static str {
        match self {
            Self::AllTime => "https://www.skills.sh/",
            Self::Trending => "https://www.skills.sh/trending",
            Self::Hot => "https://www.skills.sh/hot",
        }
    }
}

/// Search the skills.sh catalog. The query must be at least two characters.
///
/// # Errors
/// Rejects a short query, network failures, timeouts, and unrecognized JSON.
pub async fn search_marketplace_skills(
    query: &str,
    limit: usize,
) -> Result<Vec<MarketplaceSkill>, SkillError> {
    let trimmed = query.trim();
    if trimmed.chars().count() < 2 || trimmed.len() > 200 {
        return Err(SkillError::InvalidRequest);
    }
    let limit = limit.clamp(1, MAX_SEARCH_LIMIT);
    let client = http_client()?;
    // Build the URL by hand so we do not depend on reqwest's optional query
    // serializer while default features stay off.
    let url = format!(
        "{SEARCH_URL}?q={}&limit={limit}",
        urlencoding_encode(trimmed)
    );
    let response = client.get(url).send().await.map_err(map_http_error)?;
    if !response.status().is_success() {
        return Err(SkillError::MarketplaceUnavailable);
    }
    let body = response.bytes().await.map_err(map_http_error)?;
    if body.len() > MAX_HTML_BYTES {
        return Err(SkillError::MarketplaceResponseInvalid);
    }
    let parsed: SearchResponse =
        serde_json::from_slice(&body).map_err(|_| SkillError::MarketplaceResponseInvalid)?;
    Ok(filter_github(
        parsed.skills.into_iter().filter_map(catalog_row),
    ))
}

/// Load one skills.sh leaderboard page and return GitHub-installable rows.
///
/// # Errors
/// Rejects network failures, timeouts, and pages with no parseable skills.
pub async fn list_marketplace_skills(
    board: MarketplaceBoard,
) -> Result<Vec<MarketplaceSkill>, SkillError> {
    let client = http_client()?;
    let response = client
        .get(board.url())
        .send()
        .await
        .map_err(map_http_error)?;
    if !response.status().is_success() {
        return Err(SkillError::MarketplaceUnavailable);
    }
    let body = response.text().await.map_err(map_http_error)?;
    if body.len() > MAX_HTML_BYTES {
        return Err(SkillError::MarketplaceResponseInvalid);
    }
    let skills = parse_leaderboard_html(&body)?;
    let filtered = filter_github(skills);
    if filtered.is_empty() {
        return Err(SkillError::MarketplaceResponseInvalid);
    }
    Ok(filtered.into_iter().take(MAX_LEADERBOARD).collect())
}

/// Validate a marketplace row and show the Git command that would run.
///
/// No network request or library write happens here.
pub fn preview_marketplace_install(
    request: MarketplaceInstallRequest,
) -> Result<SkillInstallPreview, SkillError> {
    preview_skill_install(install_source(&request)?)
}

/// Install one marketplace skill into the central library from GitHub at HEAD.
///
/// # Errors
/// Same as a Git install, plus refusal of non-GitHub marketplace sources.
pub fn install_marketplace_skill(
    library: &SkillLibrary,
    storage: &Storage,
    request: MarketplaceInstallRequest,
    now_ms: i64,
) -> Result<SkillInstallReport, SkillError> {
    install::install_skill(library, storage, install_source(&request)?, now_ms)
}

fn install_source(request: &MarketplaceInstallRequest) -> Result<SkillInstallSource, SkillError> {
    let source = request.source.trim();
    let skill_id = request.skill_id.trim();
    if source.is_empty() || skill_id.is_empty() {
        return Err(SkillError::InvalidRequest);
    }
    if !is_github_owner_repo(source) {
        return Err(SkillError::MarketplaceSourceUnsupported);
    }
    // Canonicalize through the same GitHub validator the Git installer uses.
    let repository = validation::github_repository(source)?;
    let skill = validation::skill_selector(Some(skill_id))?.map(str::to_string);
    let skill = skill.ok_or(SkillError::InvalidRequest)?;
    Ok(SkillInstallSource::Git(GitInstallSource {
        repository,
        revision: MARKETPLACE_REVISION.to_string(),
        skill: Some(skill),
    }))
}

fn http_client() -> Result<reqwest::Client, SkillError> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(HTTP_TIMEOUT)
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|_| SkillError::MarketplaceUnavailable)
}

/// Percent-encode a query value with the same set `application/x-www-form-urlencoded` uses.
fn urlencoding_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => {
                use std::fmt::Write;
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

fn map_http_error(error: reqwest::Error) -> SkillError {
    if error.is_timeout() {
        SkillError::MarketplaceTimedOut
    } else {
        SkillError::MarketplaceUnavailable
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    skills: Vec<CatalogRow>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogRow {
    source: Option<String>,
    skill_id: Option<String>,
    name: Option<String>,
    installs: Option<u64>,
}

fn catalog_row(row: CatalogRow) -> Option<MarketplaceSkill> {
    let source = row.source?.trim().to_string();
    let skill_id = row.skill_id?.trim().to_string();
    if source.is_empty() || skill_id.is_empty() {
        return None;
    }
    let name = row
        .name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| skill_id.clone());
    Some(MarketplaceSkill {
        id: format!("{source}/{skill_id}"),
        source,
        skill_id,
        name,
        installs: row.installs.unwrap_or(0),
    })
}

fn filter_github(skills: impl IntoIterator<Item = MarketplaceSkill>) -> Vec<MarketplaceSkill> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for skill in skills {
        if !is_github_owner_repo(&skill.source) {
            continue;
        }
        if !seen.insert(skill.id.clone()) {
            continue;
        }
        out.push(skill);
    }
    out
}

/// skills.sh GitHub rows use `owner/repo` with exactly one slash.
fn is_github_owner_repo(source: &str) -> bool {
    validation::github_repository(source).is_ok() && source.matches('/').count() == 1
}

/// Extract skill objects from skills.sh HTML (RSC / escaped JSON payloads).
fn parse_leaderboard_html(html: &str) -> Result<Vec<MarketplaceSkill>, SkillError> {
    let mut skills = Vec::new();
    let mut seen = HashSet::new();
    let mut rest = html;
    while let Some(start) = rest.find(r#"\"source\":\""#) {
        rest = &rest[start + r#"\"source\":\""#.len()..];
        let Some((source, after_source)) = split_escaped(rest) else {
            break;
        };
        rest = after_source;
        let Some(skill_mark) = rest.find(r#"\"skillId\":\""#) else {
            continue;
        };
        // Refuse a huge gap so an unrelated later skillId is not paired wrongly.
        if skill_mark > 400 {
            continue;
        }
        rest = &rest[skill_mark + r#"\"skillId\":\""#.len()..];
        let Some((skill_id, after_skill)) = split_escaped(rest) else {
            break;
        };
        rest = after_skill;
        let name = take_escaped_field(rest, r#"\"name\":\""#).unwrap_or_else(|| skill_id.clone());
        let installs = take_installs(rest).unwrap_or(0);
        let id = format!("{source}/{skill_id}");
        if !seen.insert(id.clone()) {
            continue;
        }
        skills.push(MarketplaceSkill {
            id,
            source,
            skill_id,
            name,
            installs,
        });
    }
    if skills.is_empty() {
        return Err(SkillError::MarketplaceResponseInvalid);
    }
    Ok(skills)
}

fn split_escaped(input: &str) -> Option<(String, &str)> {
    let end = input.find(r#"\""#)?;
    let value = input[..end].replace(r#"\""#, "\"");
    if value.is_empty() || value.contains('\\') {
        return None;
    }
    Some((value, &input[end + 2..]))
}

fn take_escaped_field(input: &str, marker: &str) -> Option<String> {
    let start = input.find(marker)?;
    if start > 400 {
        return None;
    }
    let rest = &input[start + marker.len()..];
    split_escaped(rest).map(|(value, _)| value)
}

fn take_installs(input: &str) -> Option<u64> {
    let marker = r#"\"installs\":"#;
    let start = input.find(marker)?;
    if start > 400 {
        return None;
    }
    let rest = &input[start + marker.len()..];
    let digits: String = rest
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    digits.parse().ok()
}
