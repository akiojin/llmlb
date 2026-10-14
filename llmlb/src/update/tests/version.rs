use super::*;

#[test]
fn test_parse_tag_to_version() {
    assert_eq!(
        parse_tag_to_version("v3.1.0").unwrap(),
        Version::parse("3.1.0").unwrap()
    );
    assert_eq!(
        parse_tag_to_version("3.1.0").unwrap(),
        Version::parse("3.1.0").unwrap()
    );
}

// =======================================================================
// check_only: GitHub API失敗時にキャッシュフォールバック
// (SPEC-a6e55b37 ユーザーストーリー10シナリオ4)
// =======================================================================
// =======================================================================
// parse_tag_to_version: edge cases
// =======================================================================
#[test]
fn parse_tag_to_version_with_v_prefix() {
    let v = parse_tag_to_version("v1.2.3").unwrap();
    assert_eq!(v, Version::new(1, 2, 3));
}

#[test]
fn parse_tag_to_version_without_prefix() {
    let v = parse_tag_to_version("1.2.3").unwrap();
    assert_eq!(v, Version::new(1, 2, 3));
}

#[test]
fn parse_tag_to_version_prerelease() {
    let v = parse_tag_to_version("v2.0.0-beta.1").unwrap();
    assert_eq!(v.major, 2);
    assert!(!v.pre.is_empty());
}

#[test]
fn parse_tag_to_version_invalid() {
    assert!(parse_tag_to_version("not-a-version").is_err());
}

#[test]
fn parse_tag_to_version_empty() {
    assert!(parse_tag_to_version("").is_err());
}

#[test]
fn parse_tag_to_version_v_only() {
    assert!(parse_tag_to_version("v").is_err());
}

#[test]
fn parse_tag_to_version_partial() {
    // semver requires major.minor.patch
    assert!(parse_tag_to_version("v1.2").is_err());
}
