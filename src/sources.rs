//! Works out where a download came from.
//!
//! When a browser, Mail or a chat app saves a file, macOS stores the download URL
//! (and usually the page it was on) in the `com.apple.metadata:kMDItemWhereFroms`
//! extended attribute as a binary property list. We read it, pick the most useful
//! URL and map well known hosts to friendly names such as "Gmail" or "Slack".

use std::path::Path;

use egui_phosphor::regular as ph;

use crate::model::Source;

const WHERE_FROMS: &str = "com.apple.metadata:kMDItemWhereFroms";

/// (match, key, label). `match` is tested against the URL host with `ends_with`
/// (so `mail.google.com` matches `mail.google.com` and `x.mail.google.com`).
const BRANDS: &[(&str, &str, &str)] = &[
    ("mail.google.com", "gmail", "Gmail"),
    ("mail-attachment.googleusercontent.com", "gmail", "Gmail"),
    ("drive.google.com", "gdrive", "Google Drive"),
    ("drive.usercontent.google.com", "gdrive", "Google Drive"),
    ("docs.google.com", "gdrive", "Google Drive"),
    ("doc-0s-docs.googleusercontent.com", "gdrive", "Google Drive"),
    ("chatgpt.com", "chatgpt", "ChatGPT"),
    ("chat.openai.com", "chatgpt", "ChatGPT"),
    ("oaiusercontent.com", "chatgpt", "ChatGPT"),
    ("claude.ai", "claude", "Claude"),
    ("canva.com", "canva", "Canva"),
    ("canva-user-content.com", "canva", "Canva"),
    ("x.com", "x", "X"),
    ("twitter.com", "x", "X"),
    ("twimg.com", "x", "X"),
    ("slack.com", "slack", "Slack"),
    ("slack-edge.com", "slack", "Slack"),
    ("slack-files.com", "slack", "Slack"),
    ("whatsapp.com", "whatsapp", "WhatsApp"),
    ("whatsapp.net", "whatsapp", "WhatsApp"),
    ("github.com", "github", "GitHub"),
    ("githubusercontent.com", "github", "GitHub"),
    ("figma.com", "figma", "Figma"),
    ("youtube.com", "youtube", "YouTube"),
    ("ytimg.com", "youtube", "YouTube"),
    ("reddit.com", "reddit", "Reddit"),
    ("redd.it", "reddit", "Reddit"),
    ("instagram.com", "instagram", "Instagram"),
    ("cdninstagram.com", "instagram", "Instagram"),
    ("pinterest.com", "pinterest", "Pinterest"),
    ("pinimg.com", "pinterest", "Pinterest"),
    ("discord.com", "discord", "Discord"),
    ("discordapp.com", "discord", "Discord"),
    ("discordapp.net", "discord", "Discord"),
    ("dropbox.com", "dropbox", "Dropbox"),
    ("dropboxusercontent.com", "dropbox", "Dropbox"),
    ("telegram.org", "telegram", "Telegram"),
    ("t.me", "telegram", "Telegram"),
    ("linkedin.com", "linkedin", "LinkedIn"),
    ("licdn.com", "linkedin", "LinkedIn"),
    ("notion.so", "notion", "Notion"),
    ("notion.site", "notion", "Notion"),
    ("apple.com", "apple", "Apple"),
    ("icloud.com", "apple", "Apple"),
    ("outlook.live.com", "outlook", "Outlook"),
    ("outlook.office.com", "outlook", "Outlook"),
    ("outlook.office365.com", "outlook", "Outlook"),
];

/// Read and interpret the "where from" attribute of a file.
pub fn read_source(path: &Path) -> Option<Source> {
    let raw = xattr::get(path, WHERE_FROMS).ok()??;
    let value: plist::Value = plist::from_bytes(&raw).ok()?;
    let urls: Vec<String> = value
        .as_array()?
        .iter()
        .filter_map(|v| v.as_string().map(str::to_owned))
        .filter(|s| !s.is_empty())
        .collect();
    classify(urls)
}

/// Turn a list of URLs into a [`Source`]. Public for tests.
pub fn classify(urls: Vec<String>) -> Option<Source> {
    if urls.is_empty() {
        return None;
    }
    // Browser extensions save with chrome-extension:// or moz-extension:// URLs.
    if urls
        .iter()
        .any(|u| u.starts_with("chrome-extension://") || u.starts_with("moz-extension://") || u.starts_with("safari-web-extension://"))
    {
        return Some(Source { key: "extensions".into(), label: "Browser Extensions".into(), urls });
    }

    let hosts: Vec<String> = urls.iter().filter_map(|u| host_of(u)).collect();
    if hosts.is_empty() {
        return None;
    }

    // A brand match on any URL wins (the referrer is often the brand, the download a CDN).
    for host in &hosts {
        for (pat, key, label) in BRANDS {
            if host == pat || host.ends_with(&format!(".{pat}")) {
                return Some(Source { key: (*key).into(), label: (*label).into(), urls });
            }
        }
    }

    // Otherwise group by site. Prefer the referrer page (2nd URL) because the
    // download URL is frequently a CDN host nobody recognises.
    let host = hosts.get(1).unwrap_or(&hosts[0]).clone();
    let site = registrable_domain(&host);
    Some(Source { key: site.clone(), label: site, urls })
}

fn host_of(url: &str) -> Option<String> {
    let url = url.strip_prefix("blob:").unwrap_or(url);
    let rest = url.split_once("://")?.1;
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let host = host.split(':').next()?.to_ascii_lowercase();
    if host.is_empty() { None } else { Some(host) }
}

/// `files.cdn.example.co.uk` → `example.co.uk`, `www.example.com` → `example.com`.
fn registrable_domain(host: &str) -> String {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() <= 2 || host.parse::<std::net::Ipv4Addr>().is_ok() {
        return host.trim_start_matches("www.").to_string();
    }
    let n = parts.len();
    let second_level = ["co", "com", "org", "net", "ac", "gov", "edu"];
    let take = if parts[n - 1].len() == 2 && second_level.contains(&parts[n - 2]) { 3 } else { 2 };
    parts[n - take..].join(".")
}

/// Icon for a source card.
pub fn icon_for(key: &str) -> &'static str {
    match key {
        "gmail" | "outlook" => ph::ENVELOPE_SIMPLE,
        "gdrive" => ph::GOOGLE_DRIVE_LOGO,
        "chatgpt" => ph::OPEN_AI_LOGO,
        "claude" => ph::SPARKLE,
        "x" => ph::X_LOGO,
        "slack" => ph::SLACK_LOGO,
        "whatsapp" => ph::WHATSAPP_LOGO,
        "github" => ph::GITHUB_LOGO,
        "figma" => ph::FIGMA_LOGO,
        "youtube" => ph::YOUTUBE_LOGO,
        "reddit" => ph::REDDIT_LOGO,
        "instagram" => ph::INSTAGRAM_LOGO,
        "pinterest" => ph::PINTEREST_LOGO,
        "discord" => ph::DISCORD_LOGO,
        "dropbox" => ph::DROPBOX_LOGO,
        "telegram" => ph::TELEGRAM_LOGO,
        "linkedin" => ph::LINKEDIN_LOGO,
        "notion" => ph::NOTION_LOGO,
        "apple" => ph::APPLE_LOGO,
        "canva" => ph::PALETTE,
        "extensions" => ph::PUZZLE_PIECE,
        _ => ph::GLOBE_SIMPLE,
    }
}

/// Words people use for a source in search ("from gmail", "from drive").
pub fn alias_to_key(word: &str) -> Option<&'static str> {
    Some(match word {
        "gmail" | "email" | "mail" => "gmail",
        "drive" | "gdrive" => "gdrive",
        "chatgpt" | "openai" | "gpt" => "chatgpt",
        "claude" => "claude",
        "canva" => "canva",
        "x" | "twitter" | "tweet" | "tweets" => "x",
        "slack" => "slack",
        "whatsapp" => "whatsapp",
        "github" => "github",
        "figma" => "figma",
        "youtube" => "youtube",
        "reddit" => "reddit",
        "instagram" | "insta" => "instagram",
        "pinterest" => "pinterest",
        "discord" => "discord",
        "dropbox" => "dropbox",
        "telegram" => "telegram",
        "linkedin" => "linkedin",
        "notion" => "notion",
        "outlook" => "outlook",
        "extension" | "extensions" => "extensions",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(urls: &[&str]) -> Source {
        classify(urls.iter().map(|s| s.to_string()).collect()).unwrap()
    }

    #[test]
    fn brands() {
        assert_eq!(src(&["https://mail-attachment.googleusercontent.com/x", "https://mail.google.com/"]).key, "gmail");
        assert_eq!(src(&["blob:https://chatgpt.com/abc"]).label, "ChatGPT");
        assert_eq!(src(&["https://pbs.twimg.com/media/a.jpg"]).key, "x");
    }

    #[test]
    fn sites() {
        assert_eq!(src(&["https://cdn.files.example.co.uk/a.zip", "https://www.example.co.uk/page"]).key, "example.co.uk");
        assert_eq!(src(&["https://dl.pixabay.com/x.mp3"]).key, "pixabay.com");
    }
}
