//! Public release metadata checks. No vault data, tokens or account details are
//! used here. Installation remains the responsibility of the package manager.
use chrono::{SecondsFormat, Utc};
use reqwest::{Client, Response, StatusCode};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    time::{Duration, Instant},
};
use tauri::State;
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex;

const REPOSITORY_URL: &str = "https://github.com/Lucas-Gruber/Veyra";
const RELEASES_API: &str = "https://api.github.com/repos/Lucas-Gruber/Veyra/releases?per_page=100";
const MAX_BODY_BYTES: usize = 512 * 1024;
const SUCCESS_CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const ERROR_CACHE_TTL: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateError {
    code: &'static str,
    message: &'static str,
}

impl UpdateError {
    fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }
}

fn invalid_response() -> UpdateError {
    UpdateError::new(
        "invalid_response",
        "The release service returned unsupported data. No update status could be verified.",
    )
}
fn network_error() -> UpdateError {
    UpdateError::new(
        "network_error",
        "Could not check for updates securely. Check your connection and try again later.",
    )
}
fn unsupported_version() -> UpdateError {
    UpdateError::new(
        "unsupported_version",
        "This application has an invalid version and cannot compare releases.",
    )
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    channel: &'static str,
    platform: &'static str,
    architecture: &'static str,
    license: &'static str,
    repository_url: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum UpdateState {
    UpToDate,
    Available,
    NoReleases,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    state: UpdateState,
    current_version: String,
    latest_version: Option<String>,
    release_url: Option<String>,
    published_at: Option<String>,
    checked_at: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
}

fn parse_version(value: &str) -> Result<Version, UpdateError> {
    if value.len() > 128 {
        return Err(unsupported_version());
    }
    let version = Version::parse(value).map_err(|_| unsupported_version())?;
    if version.to_string() != value {
        return Err(unsupported_version());
    }
    Ok(version)
}

fn release_version(tag: &str) -> Option<Version> {
    parse_version(tag.strip_prefix('v')?).ok()
}

fn release_url(version: &Version) -> String {
    // Canonical SemVer contains no slashes, query, fragment or other origins.
    format!("{REPOSITORY_URL}/releases/tag/v{version}")
}

fn select_release(
    current: &Version,
    releases: Vec<GithubRelease>,
    checked_at: String,
) -> Result<UpdateStatus, UpdateError> {
    let allow_prerelease = !current.pre.is_empty();
    let mut latest: Option<(Version, String)> = None;
    for release in releases {
        if release.draft {
            continue;
        }
        if release.tag_name.len() > 129 {
            return Err(invalid_response());
        }
        let Some(version) = release_version(&release.tag_name) else {
            continue;
        };
        if !allow_prerelease && (release.prerelease || !version.pre.is_empty()) {
            continue;
        }
        let published_at = release.published_at.ok_or_else(invalid_response)?;
        if published_at.len() > 64 || chrono::DateTime::parse_from_rfc3339(&published_at).is_err() {
            return Err(invalid_response());
        }
        if latest
            .as_ref()
            .is_none_or(|(previous, _)| version.cmp_precedence(previous) == Ordering::Greater)
        {
            latest = Some((version, published_at));
        }
    }
    let Some((latest, published_at)) = latest else {
        return Ok(UpdateStatus {
            state: UpdateState::NoReleases,
            current_version: current.to_string(),
            latest_version: None,
            release_url: None,
            published_at: None,
            checked_at,
        });
    };
    Ok(UpdateStatus {
        state: if latest.cmp_precedence(current) == Ordering::Greater {
            UpdateState::Available
        } else {
            UpdateState::UpToDate
        },
        current_version: current.to_string(),
        latest_version: Some(latest.to_string()),
        release_url: Some(release_url(&latest)),
        published_at: Some(published_at),
        checked_at,
    })
}

fn parse_releases(
    current: &Version,
    body: &[u8],
    checked_at: String,
) -> Result<UpdateStatus, UpdateError> {
    if body.len() > MAX_BODY_BYTES {
        return Err(invalid_response());
    }
    let releases: Vec<GithubRelease> =
        serde_json::from_slice(body).map_err(|_| invalid_response())?;
    if releases.len() > 100 {
        return Err(invalid_response());
    }
    select_release(current, releases, checked_at)
}

async fn bounded_body(mut response: Response) -> Result<Vec<u8>, UpdateError> {
    if response
        .content_length()
        .is_some_and(|bytes| bytes > MAX_BODY_BYTES as u64)
    {
        return Err(invalid_response());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| network_error())? {
        if body.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
            return Err(invalid_response());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn response_error(status: StatusCode, rate_limit_remaining: Option<&str>) -> Option<UpdateError> {
    match status {
        StatusCode::TOO_MANY_REQUESTS => Some(UpdateError::new(
            "rate_limited",
            "The release service rate limit was reached. Try again later.",
        )),
        StatusCode::FORBIDDEN if rate_limit_remaining == Some("0") => Some(UpdateError::new(
            "rate_limited",
            "The release service rate limit was reached. Try again later.",
        )),
        StatusCode::OK => None,
        _ => Some(UpdateError::new(
            "check_unavailable",
            "The release service is unavailable. No update status could be verified.",
        )),
    }
}

async fn fetch_releases(current: &Version) -> Result<UpdateStatus, UpdateError> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(format!("Veyra/{current}"))
        .build()
        .map_err(|_| network_error())?;
    // Fixed public destination. Never forward authentication or vault context.
    let response = client
        .get(RELEASES_API)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|_| network_error())?;
    if let Some(error) = response_error(
        response.status(),
        response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|value| value.to_str().ok()),
    ) {
        return Err(error);
    }
    let body = bounded_body(response).await?;
    parse_releases(
        current,
        &body,
        Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    )
}

struct CachedCheck {
    at: Instant,
    current_version: String,
    outcome: Result<UpdateStatus, UpdateError>,
}

#[derive(Default)]
pub struct UpdateService {
    cache: Mutex<Option<CachedCheck>>,
}

#[tauri::command]
pub fn app_info(app: tauri::AppHandle) -> Result<AppInfo, UpdateError> {
    let version = parse_version(&app.package_info().version.to_string())?;
    Ok(AppInfo {
        channel: if version.pre.is_empty() {
            "stable"
        } else {
            "prerelease"
        },
        version: version.to_string(),
        platform: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        license: env!("CARGO_PKG_LICENSE"),
        repository_url: REPOSITORY_URL,
    })
}

#[tauri::command]
pub async fn check_for_updates(
    app: tauri::AppHandle,
    service: State<'_, UpdateService>,
) -> Result<UpdateStatus, UpdateError> {
    let current = parse_version(&app.package_info().version.to_string())?;
    // Serialize only update checks, never vault work. This prevents several
    // windows from simultaneously spending GitHub's unauthenticated quota.
    let mut cache = service.cache.lock().await;
    if let Some(cached) = cache.as_ref() {
        let ttl = if cached.outcome.is_ok() {
            SUCCESS_CACHE_TTL
        } else {
            ERROR_CACHE_TTL
        };
        if cached.current_version == current.to_string() && cached.at.elapsed() < ttl {
            return cached.outcome.clone();
        }
    }
    let outcome = fetch_releases(&current).await;
    *cache = Some(CachedCheck {
        at: Instant::now(),
        current_version: current.to_string(),
        outcome: outcome.clone(),
    });
    outcome
}

#[tauri::command]
pub async fn open_release_page(
    app: tauri::AppHandle,
    version: String,
    service: State<'_, UpdateService>,
) -> Result<(), UpdateError> {
    let requested = parse_version(&version)?;
    let expected = release_url(&requested);
    let cache = service.cache.lock().await;
    let permitted = cache
        .as_ref()
        .and_then(|cached| cached.outcome.as_ref().ok())
        .is_some_and(|result| {
            result.latest_version.as_deref() == Some(version.as_str())
                && result.release_url.as_deref() == Some(expected.as_str())
        });
    if !permitted {
        return Err(UpdateError::new(
            "check_required",
            "Check for updates again before opening this release.",
        ));
    }
    app.opener().open_url(expected, None::<&str>).map_err(|_| {
        UpdateError::new(
            "browser_unavailable",
            "Could not open the release page in your browser.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, prerelease: bool) -> GithubRelease {
        GithubRelease {
            tag_name: tag.into(),
            draft: false,
            prerelease,
            published_at: Some("2026-10-26T12:00:00Z".into()),
        }
    }
    fn check(current: &str, releases: Vec<GithubRelease>) -> UpdateStatus {
        select_release(
            &Version::parse(current).unwrap(),
            releases,
            "2026-10-27T12:00:00Z".into(),
        )
        .unwrap()
    }

    #[test]
    fn stable_channel_does_not_offer_preview_versions() {
        let result = check(
            "1.0.0",
            vec![
                release("v1.1.0-alpha.1", true),
                release("v1.0.1", false),
                release("v2.0.0", true),
            ],
        );
        assert_eq!(result.state, UpdateState::Available);
        assert_eq!(result.latest_version.as_deref(), Some("1.0.1"));
    }

    #[test]
    fn preview_channel_uses_semver_numeric_identifiers_and_stable_promotion() {
        let result = check(
            "0.1.0-alpha.2",
            vec![
                release("v0.1.0-alpha.9", true),
                release("v0.1.0-alpha.10", true),
                release("v0.1.0-alpha.3", true),
            ],
        );
        assert_eq!(result.latest_version.as_deref(), Some("0.1.0-alpha.10"));
        assert_eq!(
            check("0.1.0-alpha.10", vec![release("v0.1.0", false)]).state,
            UpdateState::Available
        );
    }

    #[test]
    fn does_not_offer_downgrades_or_build_metadata_as_an_update() {
        assert_eq!(
            check("2.0.0", vec![release("v1.9.9", false)]).state,
            UpdateState::UpToDate
        );
        assert_eq!(
            check("1.0.0+local.1", vec![release("v1.0.0+build.2", false)]).state,
            UpdateState::UpToDate
        );
    }

    #[test]
    fn drafts_noncanonical_tags_and_other_paths_cannot_become_release_urls() {
        let mut draft = release("v9.0.0", false);
        draft.draft = true;
        let result = check(
            "1.0.0",
            vec![
                draft,
                release("1.2.0", false),
                release("v01.2.0", false),
                release("v1.2.0/../../evil", false),
                release("v1.2.0?redirect=evil", false),
            ],
        );
        assert_eq!(result.state, UpdateState::NoReleases);
        assert!(result.release_url.is_none());
        assert_eq!(
            release_url(&Version::parse("1.2.0-alpha.1").unwrap()),
            "https://github.com/Lucas-Gruber/Veyra/releases/tag/v1.2.0-alpha.1"
        );
    }

    #[test]
    fn empty_releases_are_distinct_from_a_successful_up_to_date_check() {
        let result = check("0.1.0-alpha.1", vec![]);
        assert_eq!(result.state, UpdateState::NoReleases);
        assert!(result.latest_version.is_none());
    }

    #[test]
    fn malformed_or_oversized_data_is_an_error_not_up_to_date() {
        let current = Version::parse("1.0.0").unwrap();
        assert!(parse_releases(
            &current,
            br#"{"message":"not a releases list"}"#,
            String::new()
        )
        .is_err());
        assert!(parse_releases(&current, &vec![b' '; MAX_BODY_BYTES + 1], String::new()).is_err());
        let malformed = GithubRelease {
            published_at: Some("not a date".into()),
            ..release("v1.1.0", false)
        };
        assert!(select_release(&current, vec![malformed], String::new()).is_err());
        let missing = GithubRelease {
            published_at: None,
            ..release("v1.1.0", false)
        };
        assert!(select_release(&current, vec![missing], String::new()).is_err());
    }

    #[test]
    fn rate_limits_and_http_failures_remain_explicit_failures() {
        assert_eq!(
            response_error(StatusCode::TOO_MANY_REQUESTS, None)
                .unwrap()
                .code,
            "rate_limited"
        );
        assert_eq!(
            response_error(StatusCode::FORBIDDEN, Some("0"))
                .unwrap()
                .code,
            "rate_limited"
        );
        assert_eq!(
            response_error(StatusCode::NOT_FOUND, None).unwrap().code,
            "check_unavailable"
        );
        assert!(response_error(StatusCode::OK, None).is_none());
    }

    #[tokio::test]
    async fn chunked_or_declared_oversized_responses_are_bounded() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        for wire in [
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                MAX_BODY_BYTES + 1
            )
            .into_bytes(),
            {
                let body = vec![b'x'; MAX_BODY_BYTES + 1];
                let mut wire = format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n", body.len()).into_bytes();
                wire.extend(body);
                wire.extend_from_slice(b"\r\n0\r\n\r\n");
                wire
            },
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut buffer = [0; 4096];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(&wire);
            });
            let response = Client::new()
                .get(format!("http://{address}"))
                .send()
                .await
                .unwrap();
            assert!(
                matches!(bounded_body(response).await, Err(error) if error.code == "invalid_response")
            );
            server.join().unwrap();
        }
    }
}
