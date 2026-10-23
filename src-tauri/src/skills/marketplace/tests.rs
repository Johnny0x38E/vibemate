//! Marketplace catalog parsing and GitHub-only install mapping.

use super::{
    filter_github, install_source, is_github_owner_repo, parse_leaderboard_html,
    preview_marketplace_install,
};
use crate::skills::types::{MarketplaceInstallRequest, MarketplaceSkill, SkillError};

#[test]
fn parses_escaped_rsc_leaderboard_payload() {
    let html = r#"
        <script>self.__next_f.push([1,"...\n[{\"source\":\"anthropics/skills\",\"skillId\":\"template-skill\",\"name\":\"template-skill\",\"installs\":238},{\"source\":\"vercel/ai\",\"skillId\":\"ai-sdk\",\"name\":\"ai-sdk\",\"installs\":265},{\"source\":\"open.feishu.cn\",\"skillId\":\"lark-doc\",\"name\":\"lark-doc\",\"installs\":10}]...\n"])</script>
        "#;
    let skills = parse_leaderboard_html(html).expect("parse");
    assert_eq!(skills.len(), 3);
    assert_eq!(skills[0].id, "anthropics/skills/template-skill");
    assert_eq!(skills[0].installs, 238);
    let github = filter_github(skills);
    assert_eq!(github.len(), 2);
    assert!(github.iter().all(|row| is_github_owner_repo(&row.source)));
}

#[test]
fn refuses_non_github_marketplace_sources() {
    let error = preview_marketplace_install(MarketplaceInstallRequest {
        source: "open.feishu.cn".to_string(),
        skill_id: "lark-doc".to_string(),
    })
    .expect_err("site source");
    assert_eq!(error, SkillError::MarketplaceSourceUnsupported);

    let preview = preview_marketplace_install(MarketplaceInstallRequest {
        source: "mattpocock/skills".to_string(),
        skill_id: "tdd".to_string(),
    })
    .expect("github source");
    assert_eq!(preview.source_url, "https://github.com/mattpocock/skills");
    assert_eq!(preview.revision.as_deref(), Some("HEAD"));
    assert!(preview.command.contains("git fetch --depth 1"));
    assert!(preview.command.contains("HEAD"));
}

#[test]
fn install_source_requires_skill_id() {
    assert_eq!(
        install_source(&MarketplaceInstallRequest {
            source: "a/b".to_string(),
            skill_id: "  ".to_string(),
        })
        .unwrap_err(),
        SkillError::InvalidRequest
    );
}

#[test]
fn filter_github_deduplicates_ids() {
    let rows = vec![
        MarketplaceSkill {
            id: "a/b/s".to_string(),
            source: "a/b".to_string(),
            skill_id: "s".to_string(),
            name: "s".to_string(),
            installs: 1,
        },
        MarketplaceSkill {
            id: "a/b/s".to_string(),
            source: "a/b".to_string(),
            skill_id: "s".to_string(),
            name: "s".to_string(),
            installs: 2,
        },
    ];
    assert_eq!(filter_github(rows).len(), 1);
}
