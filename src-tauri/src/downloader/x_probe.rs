use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;

const X_GUEST_BEARER_TOKEN: &str =
    "AAAAAAAAAAAAAAAAAAAAANRILgAAAAAAnNwIzUejRCOuH5E6I8xnZz4puTs%3D1Zv7ttfk8LF81IUq16cHjhLTvJu4FA33AGWWjCpTnA";
const X_TWEET_RESULT_QUERY_ID: &str = "8CEYnZhCp0dx9DFyyEBlbQ";
const X_TWEET_RESULT_FEATURES: &str = r#"{"responsive_web_graphql_exclude_directive_enabled":true,"verified_phone_label_enabled":false,"creator_subscriptions_tweet_preview_api_enabled":true,"responsive_web_graphql_timeline_navigation_enabled":true,"responsive_web_graphql_skip_user_profile_image_extensions_enabled":false,"premium_content_api_read_enabled":false,"communities_web_enable_tweet_community_results_fetch":true,"c9s_tweet_anatomy_moderator_badge_enabled":true,"responsive_web_grok_analyze_button_fetch_trends_enabled":false,"responsive_web_grok_analyze_post_followups_enabled":true,"responsive_web_jetfuel_frame":false,"responsive_web_grok_share_attachment_enabled":true,"responsive_web_edit_tweet_api_enabled":true,"graphql_is_translatable_rweb_tweet_is_translatable_enabled":true,"view_counts_everywhere_api_enabled":true,"longform_notetweets_consumption_enabled":true,"responsive_web_twitter_article_tweet_consumption_enabled":true,"tweet_awards_web_tipping_enabled":false,"freedom_of_speech_not_reach_fetch_enabled":true,"standardized_nudges_misinfo":true,"tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled":true,"longform_notetweets_rich_text_read_enabled":true,"longform_notetweets_inline_media_enabled":true,"responsive_web_grok_image_annotation_enabled":true,"responsive_web_enhance_cards_enabled":false}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XRestrictionInfo {
    pub kind: String,
    pub message: String,
    pub author_screen_name: Option<String>,
}

pub fn extract_status_id(url: &str) -> Option<String> {
    let marker = "/status/";
    let status_index = url.find(marker)?;
    let rest = &url[(status_index + marker.len())..];
    let digits = rest
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();

    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

pub fn inspect_tweet_result_payload(payload: &str) -> Option<XRestrictionInfo> {
    let envelope: TweetResultEnvelope = serde_json::from_str(payload).ok()?;
    let result = envelope.data.tweet_result.result;

    if result.typename != "TweetTombstone" {
        return None;
    }

    let tombstone = result.tombstone?;
    if tombstone.typename.as_deref()? != "BlurredMediaTombstone" {
        return None;
    }

    Some(XRestrictionInfo {
        kind: "blurred_media".into(),
        message: tombstone.text?.text,
        author_screen_name: tombstone
            .user_results
            .and_then(|user_results| user_results.result)
            .and_then(|user| user.core)
            .and_then(|core| core.screen_name),
    })
}

pub fn probe_x_guest_restriction(url: &str) -> Result<Option<XRestrictionInfo>, String> {
    let status_id =
        extract_status_id(url).ok_or_else(|| "无法从 X 链接中提取 status id。".to_string())?;
    let client = Client::builder()
        .build()
        .map_err(|error| format!("创建 X 探测请求客户端失败：{error}"))?;
    let guest_token = fetch_guest_token(&client, url)?;

    let mut request_url = reqwest::Url::parse(&format!(
        "https://x.com/i/api/graphql/{X_TWEET_RESULT_QUERY_ID}/TweetResultByRestId"
    ))
    .map_err(|error| format!("构建 X 探测地址失败：{error}"))?;
    request_url
        .query_pairs_mut()
        .append_pair(
            "variables",
            &format!(
                r#"{{"tweetId":"{status_id}","withCommunity":false,"includePromotedContent":false,"withVoice":true}}"#
            ),
        )
        .append_pair("features", X_TWEET_RESULT_FEATURES);

    let mut headers = default_headers()?;
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {X_GUEST_BEARER_TOKEN}"))
            .map_err(|error| format!("构建 X Bearer token 头失败：{error}"))?,
    );
    headers.insert(
        "x-guest-token",
        HeaderValue::from_str(&guest_token)
            .map_err(|error| format!("构建 X guest token 头失败：{error}"))?,
    );
    headers.insert("x-twitter-active-user", HeaderValue::from_static("yes"));
    headers.insert("x-twitter-client-language", HeaderValue::from_static("en"));

    let response = client
        .get(request_url)
        .headers(headers)
        .send()
        .map_err(|error| format!("请求 X tweetResult 接口失败：{error}"))?;
    let status = response.status();
    let payload = response
        .text()
        .map_err(|error| format!("读取 X tweetResult 返回失败：{error}"))?;

    if !status.is_success() {
        return Err(format!("X tweetResult 接口返回异常：{status}"));
    }

    Ok(inspect_tweet_result_payload(&payload))
}

fn fetch_guest_token(client: &Client, page_url: &str) -> Result<String, String> {
    let response = client
        .get(page_url)
        .headers(default_headers()?)
        .send()
        .map_err(|error| format!("请求 X 页面失败：{error}"))?;
    let status = response.status();
    let html = response
        .text()
        .map_err(|error| format!("读取 X 页面失败：{error}"))?;

    if !status.is_success() {
        return Err(format!("X 页面返回异常：{status}"));
    }

    let marker = r#"document.cookie="gt="#;
    let start = html
        .find(marker)
        .ok_or_else(|| "未能从 X 页面中提取 guest token。".to_string())?
        + marker.len();
    let rest = &html[start..];
    let end = rest
        .find(';')
        .ok_or_else(|| "X guest token 格式异常。".to_string())?;

    Ok(rest[..end].to_string())
}

fn default_headers() -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json, text/html"));
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36",
        ),
    );
    Ok(headers)
}

#[derive(Deserialize)]
struct TweetResultEnvelope {
    data: TweetResultData,
}

#[derive(Deserialize)]
struct TweetResultData {
    #[serde(rename = "tweetResult")]
    tweet_result: TweetResultNode,
}

#[derive(Deserialize)]
struct TweetResultNode {
    result: TweetResult,
}

#[derive(Deserialize)]
struct TweetResult {
    #[serde(rename = "__typename")]
    typename: String,
    tombstone: Option<TweetTombstone>,
}

#[derive(Deserialize)]
struct TweetTombstone {
    #[serde(rename = "__typename")]
    typename: Option<String>,
    text: Option<TombstoneText>,
    user_results: Option<TombstoneUserResults>,
}

#[derive(Deserialize)]
struct TombstoneText {
    text: String,
}

#[derive(Deserialize)]
struct TombstoneUserResults {
    result: Option<TombstoneUser>,
}

#[derive(Deserialize)]
struct TombstoneUser {
    core: Option<TombstoneUserCore>,
}

#[derive(Deserialize)]
struct TombstoneUserCore {
    screen_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{extract_status_id, inspect_tweet_result_payload};

    const BLURRED_MEDIA_TOMBSTONE: &str = r#"{
      "data": {
        "tweetResult": {
          "result": {
            "__typename": "TweetTombstone",
            "tombstone": {
              "__typename": "BlurredMediaTombstone",
              "text": {
                "text": "Age-restricted adult content. This content might not be appropriate for people under 18 years old. To view this media, you’ll need to log in to X. Learn more"
              },
              "user_results": {
                "result": {
                  "core": {
                    "screen_name": "Caughtgirls1"
                  }
                }
              }
            }
          }
        }
      }
    }"#;

    const NORMAL_TWEET_RESULT: &str = r#"{
      "data": {
        "tweetResult": {
          "result": {
            "__typename": "Tweet"
          }
        }
      }
    }"#;

    #[test]
    fn extracts_status_id_from_x_video_url() {
        assert_eq!(
            extract_status_id("https://x.com/Caughtgirls1/status/2066861838516564280/video/1"),
            Some("2066861838516564280".to_string())
        );
    }

    #[test]
    fn detects_blurred_media_tombstone_from_guest_payload() {
        let result = inspect_tweet_result_payload(BLURRED_MEDIA_TOMBSTONE)
            .expect("blurred media tombstone should be detected");

        assert_eq!(result.kind, "blurred_media");
        assert!(result.message.contains("Age-restricted adult content"));
        assert_eq!(result.author_screen_name.as_deref(), Some("Caughtgirls1"));
    }

    #[test]
    fn ignores_normal_tweet_payload() {
        assert_eq!(inspect_tweet_result_payload(NORMAL_TWEET_RESULT), None);
    }
}
