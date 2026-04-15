use aidoku::{
    alloc::{String, Vec, string::ToString},
    imports::{
        defaults::defaults_get,
        net::Request,
    },
    prelude::format,
    Chapter, FilterValue, Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Viewer,
};

use crate::helper::*;

pub fn parse_manga_list(base_url: &str, filters: Vec<FilterValue>) -> Result<MangaPageResult> {
    let (query, search) = check_for_search(filters);

    let url = if search {
        format!("{}/search?keyword={}", base_url, query)
    } else {
        // This is to handle parse_manga_listing as it passes in a full url,
        // not just the base
        if base_url.contains("genre") {
            String::from(base_url)
        } else {
            format!("{}/genre", base_url)
        }
    };

    let html = request(&url, false)?.html()?;

    let mut entries: Vec<Manga> = Vec::new();

    if let Some(manga_nodes) = html.select("#content > div.webtoon_list_wrap ul > li > a") {
        for manga_node in manga_nodes {
            let url = manga_node.attr("href").unwrap_or_default();
            let key = get_manga_id(&url);
            let cover = manga_node
                .select_first("img")
                .and_then(|img| img.attr("src"));
            let title = manga_node
                .select_first(".title")
                .and_then(|el| el.text())
                .unwrap_or_default();

            entries.push(Manga {
                key,
                cover,
                title,
                url: Some(url),
                viewer: Viewer::Webtoon,
                ..Default::default()
            });
        }
    }

    Ok(MangaPageResult {
        entries,
        has_next_page: false,
    })
}

pub fn parse_canvas_list(url: &str, page: i32) -> Result<MangaPageResult> {
    // Canvas series are series uploaded by individual artists,
    // aka unlicensed series
    let canvas_series: bool = defaults_get("canvasSeries").unwrap_or(true);
    // If canvas series are disabled, return an empty result
    if !canvas_series {
        return Ok(MangaPageResult::default());
    }

    let url = format!("{}&page={}", url, page);

    let html = request(&url, false)?.html()?;

    let mut entries: Vec<Manga> = Vec::new();

    if let Some(manga_nodes) = html.select("#content div.challenge_lst > ul > li > a") {
        for manga_node in manga_nodes {
            let url = manga_node.attr("href").unwrap_or_default();
            let key = get_manga_id(&url);
            let cover = manga_node
                .select_first("img")
                .and_then(|img| img.attr("src"));
            let title = manga_node
                .select_first(".subj")
                .and_then(|el| el.text())
                .unwrap_or_default();

            entries.push(Manga {
                key,
                cover,
                title,
                url: Some(url),
                viewer: Viewer::Webtoon,
                ..Default::default()
            });
        }
    }

    let has_next_page = html
        .select_first(
            "#content > div.cont_box > div.challenge_cont_area > div.paginate > a.pg_next",
        )
        .and_then(|el| el.text())
        .map(|t| !t.is_empty())
        .unwrap_or(false);

    Ok(MangaPageResult {
        entries,
        has_next_page,
    })
}

pub fn parse_manga_listing(
    base_url: String,
    listing_id: &str,
    page: i32,
) -> Result<MangaPageResult> {
    let url = match listing_id {
        "latest" => format!("{}/genre?sortOrder=UPDATE", base_url),
        "popular" => format!("{}/genre?sortOrder=MANA", base_url),
        "top" => format!("{}/genre?sortOrder=LIKEIT", base_url),
        "canvas_latest" => format!("{}/canvas/list?genreTab=ALL&sortOrder=UPDATE", base_url),
        "canvas_popular" => {
            format!("{}/canvas/list?genreTab=ALL&sortOrder=READ_COUNT", base_url)
        }
        "canvas_top" => format!("{}/canvas/list?genreTab=ALL&sortOrder=LIKEIT", base_url),
        _ => format!("{}/genre", base_url),
    };

    if url.contains("canvas") {
        parse_canvas_list(&url, page)
    } else {
        parse_manga_list(&url, Vec::new())
    }
}

pub fn parse_manga_details(base_url: &str, manga_key: String) -> Result<Manga> {
    let url = get_manga_url(&manga_key, base_url);

    let html = request(&url, false)?.html()?;

    let cover = html
        .select_first("head meta[property=\"og:image\"]")
        .and_then(|el| el.attr("content"));

    let title = html
        .select_first(
            "#content > div.cont_box > div.detail_header > div.info > .subj",
        )
        .and_then(|el| el.text())
        .unwrap_or_default();

    let author_artist_raw = html
        .select_first(
            "#content > div.cont_box > div.detail_header > div.info > .author_area",
        )
        .and_then(|el| el.text())
        .unwrap_or_default()
        .replace("author info", "");

    let author_artist: Vec<&str> = author_artist_raw.split(',').collect();

    let author = author_artist
        .first()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let artist = if author_artist.len() > 1 {
        let a = author_artist[1].trim().to_string();
        if a.is_empty() { None } else { Some(a) }
    } else {
        None
    };

    let description = html
        .select_first("#_asideDetail > .summary")
        .and_then(|el| el.text());

    let status = {
        let status_text = html
            .select_first("#_asideDetail > .day_info")
            .and_then(|el| el.text())
            .unwrap_or_default()
            .to_lowercase();

        let series_note = html
            .select_first(
                "#content > div.cont_box > div.detail_body div.detail_paywall",
            )
            .and_then(|el| el.text())
            .unwrap_or_default()
            .to_lowercase();

        // Even if a series is on hiatus it will have "every x" in the status text
        // So we have to check the series note for hiatus before checking for ongoing
        if status_text.contains("completed") {
            MangaStatus::Completed
        } else if series_note.contains("will return") {
            MangaStatus::Hiatus
        } else if status_text.contains("every") {
            MangaStatus::Ongoing
        } else {
            MangaStatus::Unknown
        }
    };

    let mut tags: Vec<String> = Vec::new();
    if let Some(category_nodes) = html.select(
        "#content > div.cont_box > div.detail_header > div.info > .genre",
    ) {
        for category_node in category_nodes {
            if let Some(cat) = category_node.text() {
                tags.push(cat);
            }
        }
    }

    let url = html
        .select_first("head meta[property=\"og:url\"]")
        .and_then(|el| el.attr("content"))
        .or_else(|| Some(url.clone()));

    let authors = author.map(|a| {
        let mut v = Vec::new();
        v.push(a);
        v
    });
    let artists = artist.map(|a| {
        let mut v = Vec::new();
        v.push(a);
        v
    });

    Ok(Manga {
        key: manga_key,
        cover,
        title,
        authors,
        artists,
        description,
        url,
        tags: if tags.is_empty() { None } else { Some(tags) },
        status,
        viewer: Viewer::Webtoon,
        ..Default::default()
    })
}

pub fn parse_chapter_list(manga_key: String) -> Result<Vec<Chapter>> {
    let base_url = get_base_url_no_lang(true);
    let api_url = if let Some(canvas_id) = manga_key.strip_suffix("-canvas") {
        format!(
            "{}/api/v1/canvas/{}/episodes?pageSize=100000",
            base_url, canvas_id
        )
    } else {
        format!(
            "{}/api/v1/webtoon/{}/episodes?pageSize=100000",
            base_url, manga_key
        )
    };

    let response = request(&api_url, true)?.send()?;
    let data = response.get_data()?;
    let json_str = aidoku::alloc::string::String::from_utf8(data)
        .map_err(|_| aidoku::error!("utf8 error"))?;

    let lang = get_lang_code().unwrap_or(String::from("en"));

    let mut chapters: Vec<Chapter> = Vec::new();

    // Parse JSON manually using simple string operations
    // The JSON structure is: {"result":{"episodeList":[{...}, ...]}}
    // We'll extract the episodeList array and parse each episode
    parse_episode_list_json(&json_str, &manga_key, &lang, &base_url, &mut chapters);

    Ok(chapters)
}

fn parse_episode_list_json(
    json: &str,
    manga_key: &str,
    lang: &str,
    base_url: &str,
    chapters: &mut Vec<Chapter>,
) {
    // Find "episodeList":[
    let marker = "\"episodeList\":[";
    let start = match json.find(marker) {
        Some(pos) => pos + marker.len(),
        None => return,
    };

    let json_tail = &json[start..];

    // Find each episode object {...}
    let mut depth = 0i32;
    let mut obj_start: Option<usize> = None;
    let mut episode_jsons: Vec<&str> = Vec::new();

    for (i, c) in json_tail.char_indices() {
        match c {
            '{' => {
                if depth == 0 {
                    obj_start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(start) = obj_start {
                        episode_jsons.push(&json_tail[start..=i]);
                        obj_start = None;
                    }
                }
            }
            ']' if depth == 0 => break,
            _ => {}
        }
    }

    // Parse each episode JSON object in reverse (oldest first)
    for episode_json in episode_jsons.iter().rev() {
        if let Some(chapter) = parse_episode_object(episode_json, manga_key, lang, base_url) {
            chapters.push(chapter);
        }
    }
}

fn json_string_value<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let search = format!("\"{}\":\"", key);
    let start = json.find(search.as_str())? + search.len();
    let rest = &json[start..];
    // Find closing quote (not escaped)
    let mut end = 0;
    let mut chars = rest.char_indices();
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            chars.next(); // skip escaped char
            continue;
        }
        if c == '"' {
            end = i;
            break;
        }
    }
    Some(&rest[..end])
}

fn json_number_value(json: &str, key: &str) -> Option<f64> {
    let search = format!("\"{}\":", key);
    let start = json.find(search.as_str())? + search.len();
    let rest = &json[start..];
    // Read until non-numeric char
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
        .unwrap_or(rest.len());
    rest[..end].parse::<f64>().ok()
}

fn parse_episode_object(
    json: &str,
    _manga_key: &str,
    lang: &str,
    base_url: &str,
) -> Option<Chapter> {
    let viewer_link = json_string_value(json, "viewerLink")?;
    let url = format!("{}{}", base_url, viewer_link);
    let key = get_chapter_id(&url);

    let raw_title = json_string_value(json, "episodeTitle")
        .map(|s| unescape_json_string(s))
        .unwrap_or_default();

    let mut title_parts: Vec<&str> = raw_title.split_whitespace().collect();

    let mut volume = None::<f32>;

    // Remove leading volume text and set volume accordingly
    // This is for titles like "(S1) Chapter 1 - PeePeePooPoo"
    if !title_parts.is_empty() {
        let title_chars: Vec<char> = title_parts[0].chars().collect();

        if title_chars.len() >= 3
            && (title_chars[1] == 'S' || title_chars[1] == 'T')
            && String::from(title_chars[2]).parse::<f64>().is_ok()
        {
            volume = String::from(title_chars[2]).parse::<f32>().ok();
            title_parts.remove(0);
        }

        // Remove leading episode text "Ep.1 - ..."
        if !title_parts.is_empty() {
            let tc: Vec<char> = title_parts[0].chars().collect();
            if tc.len() >= 4
                && (tc[0] == 'E' && (tc[1] == 'p' || tc[1] == 'P') && tc[2] == '.')
                && tc[3..].iter().collect::<String>().parse::<f64>().is_ok()
            {
                title_parts.remove(0);
            }
        }
    }

    // Remove leading season text "[Season 1] Chapter 1 - ..."
    if title_parts.len() >= 2
        && title_parts[0] == "[Season"
        && title_parts[1].replace(']', "").parse::<f64>().is_ok()
    {
        volume = title_parts[1].replace(']', "").parse::<f32>().ok();
        title_parts.remove(0);
        title_parts.remove(0);
    }

    // Remove leading chapter/episode text
    if title_parts.len() >= 2
        && (title_parts[0] == "Chapter"
            || title_parts[0] == "Episode"
            || title_parts[0] == "Ch."
            || title_parts[0] == "CH."
            || title_parts[0] == "Ep."
            || title_parts[0] == "EP"
            || title_parts[0] == "EP.")
        && title_parts[1].replace(':', "").parse::<f64>().is_ok()
    {
        title_parts.remove(0);
        title_parts.remove(0);
    }

    // Remove leading symbols
    if !title_parts.is_empty() && (title_parts[0] == "-" || title_parts[0] == ":") {
        title_parts.remove(0);
    }

    let title = title_parts.join(" ");
    let title = if title.is_empty() { None } else { Some(title) };

    let chapter_number =
        json_number_value(json, "episodeNo").map(|f| f as f32);

    let date_uploaded = json_number_value(json, "exposureDateMillis")
        .map(|f| (f / 1000.0) as i64);

    Some(Chapter {
        key,
        title,
        volume_number: volume,
        chapter_number,
        date_uploaded,
        url: Some(url),
        language: Some(String::from(lang)),
        ..Default::default()
    })
}

/// Unescape basic JSON string escapes
fn unescape_json_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('/') => result.push('/'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => {}
            }
        } else {
            result.push(c);
        }
    }
    result
}

pub fn parse_page_list(
    base_url: &str,
    manga_key: &str,
    chapter_key: &str,
) -> Result<Vec<Page>> {
    let url = get_chapter_url(chapter_key, manga_key, base_url);

    let html = request(&url, false)?.html()?;

    let mut pages: Vec<Page> = Vec::new();

    // Optional pages contain "?type=opti" at the end of the url
    let optional_pages: bool = defaults_get("optionalPages").unwrap_or(true);

    if let Some(img_nodes) = html.select("div#_imageList > img") {
        for img_node in img_nodes {
            let url = img_node.attr("data-url").unwrap_or_default();

            // Skip optional pages if optionalPages is false
            if url.ends_with("?type=opti") && !optional_pages {
                continue;
            }

            pages.push(Page {
                content: PageContent::url(url),
                ..Default::default()
            });
        }
    }

    Ok(pages)
}

pub fn get_image_request(base_url: &str, url: String) -> Result<Request> {
    Ok(Request::get(&url)?
        .header("Referer", base_url))
}

pub fn handle_url(url: String) -> Result<String> {
    Ok(get_manga_id(&url))
}
