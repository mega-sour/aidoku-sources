use aidoku::{
    alloc::{String, Vec},
    helpers::uri::encode_uri_component,
    imports::{defaults::defaults_get, net::Request},
    prelude::format,
    FilterValue,
};

pub fn get_base_url_no_lang(mobile: bool) -> String {
    if mobile {
        String::from("https://m.webtoons.com")
    } else {
        String::from("https://www.webtoons.com")
    }
}

/// Returns the correct url for the selected language and mobile/desktop
pub fn get_base_url(mobile: bool) -> String {
    let base = get_base_url_no_lang(mobile);
    let lang = get_lang_code().unwrap_or(String::from("en"));
    format!("{}/{}", base, lang)
}

/// Returns the currently selected language
pub fn get_lang_code() -> Option<String> {
    let languages: Vec<String> = defaults_get("languages")?;
    languages.into_iter().next()
}

/// Returns a useragent string
pub fn get_user_agent(mobile: bool) -> String {
    if mobile {
        String::from("Mozilla/5.0 (iPhone; CPU iPhone OS 16_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.1 Mobile/15E148 Safari/604.1")
    } else {
        String::from("Mozilla/5.0 (Macintosh; Intel Mac OS X 13_1) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.1 Safari/605.1.15")
    }
}

/// Request wrapper to set cookies for each request
pub fn request(url: &str, mobile: bool) -> Result<Request, aidoku::imports::net::RequestError> {
    let locale = get_lang_code().unwrap_or(String::from("en"));
    let age_gate_pass = true;
    let need_gdpr = false;
    let need_ccpa = true;
    let need_coppa = false;
    let user_agent = get_user_agent(mobile);

    let cookie_string = format!(
        "locale={}; ageGatePass={}; needGDPR={}; needCCPA={}; needCOPPA={}",
        locale, age_gate_pass, need_gdpr, need_ccpa, need_coppa
    );

    Ok(Request::get(url)?
        .header("Referer", get_base_url_no_lang(mobile).as_str())
        .header("Cookie", cookie_string.as_str())
        .header("User-Agent", user_agent.as_str()))
}

/// Returns the ID of a manga from a URL.
pub fn get_manga_id(url: &str) -> String {
    // Example Url: https://www.webtoons.com/en/action/ultra-alternate-character/list?title_no=3581
    // Example Url: https://www.webtoons.com/episodeList?titleNo=3581
    // parse "3581" from the url

    // Webtoons also has a different category called "Canvas" titles which are in a
    // different format. They contain "canvas" or "challenge" in the url,
    // so we have to account for that;
    // simple solution, append "canvas" to the id if it's a canvas title
    // Example Url: https://www.webtoons.com/en/canvas/meme-girls/list?title_no=304446
    // Example Url: https://www.webtoons.com/challenge/episodeList?titleNo=304446
    // parse "304446-canvas" from the url

    if url.contains("title_no=") || url.contains("titleNo=") {
        let split_url = {
            if url.contains("title_no=") {
                url.split("title_no=").collect::<Vec<&str>>()
            } else if url.contains("titleNo=") {
                url.split("titleNo=").collect::<Vec<&str>>()
            } else {
                Vec::new()
            }
        };

        if !split_url.is_empty() {
            let manga_id = split_url[1];

            // Append "canvas" to the id if it's a canvas title
            if url.contains("canvas") || url.contains("challenge") {
                format!("{}-canvas", manga_id)
            } else {
                String::from(manga_id)
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}

/// Returns the ID of a chapter from a URL.
pub fn get_chapter_id(url: &str) -> String {
    // Example Url: https://www.webtoons.com/en/action/ultra-alternate-character/ep-1-healer-servant/viewer?title_no=3581&episode_no=1
    // Example Url: https://www.webtoons.com/viewer?titleNo=3581&episodeNo=1
    // parse "1" from the url

    if url.contains("episode_no=") || url.contains("episodeNo=") {
        let split_url = {
            if url.contains("episode_no=") {
                url.split("episode_no=").collect::<Vec<&str>>()
            } else if url.contains("episodeNo=") {
                url.split("episodeNo=").collect::<Vec<&str>>()
            } else {
                Vec::new()
            }
        };

        if !split_url.is_empty() {
            let chapter_id = split_url[1];

            // Append "canvas" to the id if it's a canvas title
            if url.contains("canvas") || url.contains("challenge") {
                format!("{}-canvas", chapter_id)
            } else {
                String::from(chapter_id)
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}

/// Returns full URL of a manga from a manga ID.
pub fn get_manga_url(manga_id: &str, base_url: &str) -> String {
    // Example manga id: 3581
    // return "https://www.webtoons.com/episodeList?titleNo=3581"

    // For canvas titles, we have to remove the "canvas" from the id
    // Example manga id: 304446-canvas
    // return "https://www.webtoons.com/challenge/episodeList?titleNo=304446"

    // Removing the language tag from the url, because it is not required
    let mut split_url = base_url.split('/').collect::<Vec<&str>>();
    split_url.pop();

    let base_url_no_lang = split_url.join("/");

    // Removing the "canvas" from the id if it's a canvas title
    if let Some(canvas_id) = manga_id.strip_suffix("-canvas") {
        format!(
            "{}/challenge/episodeList?titleNo={}",
            base_url_no_lang, canvas_id
        )
    } else {
        format!("{}/episodeList?titleNo={}", base_url_no_lang, manga_id)
    }
}

/// Returns full URL of a chapter from a chapter ID and manga ID.
pub fn get_chapter_url(chapter_id: &str, manga_id: &str, base_url: &str) -> String {
    // Removing the language tag from the url, because it is not required
    let mut split_url = base_url.split('/').collect::<Vec<&str>>();
    split_url.pop();

    let base_url = split_url.join("/");

    // Removing the "canvas" from the id's if it's a canvas title
    if manga_id.contains("-canvas") && chapter_id.contains("-canvas") {
        let manga_id = manga_id.replace("-canvas", "");
        let chapter_id = chapter_id.replace("-canvas", "");
        format!(
            "{}/challenge/viewer?titleNo={}&episodeNo={}",
            base_url, manga_id, chapter_id
        )
    } else {
        format!(
            "{}/viewer?titleNo={}&episodeNo={}",
            base_url, manga_id, chapter_id
        )
    }
}

/// Returns the search status as a boolean and the search string if there is one
pub fn check_for_search(filters: Vec<FilterValue>) -> (String, bool) {
    let mut search_string = String::new();
    let mut search = false;

    for filter in filters {
        if let FilterValue::Text { value, .. } = filter {
            if !value.is_empty() {
                search_string
                    .push_str(encode_uri_component(value.to_lowercase().as_str()).as_str());
                search = true;
                break;
            }
        }
    }
    (search_string, search)
}
