use aidoku::{
    alloc::{String, Vec, string::ToString},
    imports::html::Document,
    prelude::*,
    Chapter, ContentRating, FilterValue, Manga, MangaStatus, Page, PageContent,
    Result, Viewer,
};

pub const BASE_URL: &str = "https://www.mangapill.com";
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_3_1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/113.0.0.0 Safari/537.36";

pub fn parse_recents(html: &Document) -> Vec<Manga> {
    let mut result: Vec<Manga> = Vec::new();

    if let Some(items) = html.select("div.grid div:not([class])") {
        for obj in items {
            let key = obj
                .select_first("a.text-secondary")
                .and_then(|el| el.attr("href"))
                .unwrap_or_default();
            let title = obj
                .select_first("a.text-secondary")
                .and_then(|el| el.text())
                .unwrap_or_default();
            let cover = obj
                .select_first("a figure img")
                .and_then(|el| el.attr("data-src"));

            if !key.is_empty() && !title.is_empty() {
                result.push(Manga {
                    key,
                    cover,
                    title,
                    ..Default::default()
                });
            }
        }
    }

    result
}

pub fn parse_search(html: &Document) -> Vec<Manga> {
    let mut result: Vec<Manga> = Vec::new();

    if let Some(items) = html.select(".grid.gap-3 div") {
        for obj in items {
            let key = obj
                .select_first("a")
                .and_then(|el| el.attr("href"))
                .unwrap_or_default();
            let title = obj
                .select_first("div a")
                .and_then(|el| el.text())
                .unwrap_or_default();
            let cover = obj
                .select_first("a figure img")
                .and_then(|el| el.attr("data-src"));

            if !key.is_empty() && !title.is_empty() {
                result.push(Manga {
                    key,
                    cover,
                    title,
                    ..Default::default()
                });
            }
        }
    }

    result
}

pub fn parse_manga(html: Document, key: String) -> Result<Manga> {
    let title = html
        .select_first(".lazy")
        .and_then(|el| el.attr("alt"))
        .unwrap_or_default();
    let cover = html
        .select_first(".lazy")
        .and_then(|el| el.attr("data-src"));
    let description = html
        .select_first(".text-sm.text--secondary")
        .and_then(|el| el.text());

    let type_str = html
        .select_first(".grid.grid-cols-1.gap-3.mb-3 div:first-child div")
        .and_then(|el| el.text())
        .unwrap_or_default()
        .to_lowercase();
    let status_str = html
        .select_first(".grid.grid-cols-1.gap-3.mb-3 div:nth-child(2) div:nth-child(2)")
        .and_then(|el| el.text())
        .unwrap_or_default()
        .to_lowercase();

    let url = format!("{}{}", BASE_URL, &key);

    let mut tags: Vec<String> = Vec::new();
    if let Some(tag_els) = html.select("a[href*=genre]") {
        for el in tag_els {
            if let Some(t) = el.text() {
                tags.push(t);
            }
        }
    }

    let status = if status_str.contains("publishing") {
        MangaStatus::Ongoing
    } else if status_str.contains("finished") {
        MangaStatus::Completed
    } else {
        MangaStatus::Unknown
    };

    let content_rating = if html
        .select_first(".alert-warning")
        .and_then(|el| el.text())
        .unwrap_or_default()
        .contains("Mature")
    {
        ContentRating::NSFW
    } else if tags.contains(&String::from("Ecchi")) {
        ContentRating::Suggestive
    } else {
        ContentRating::Safe
    };

    let viewer = match type_str.as_str() {
        "manga" => Viewer::RightToLeft,
        "manhwa" => Viewer::Webtoon,
        _ => Viewer::RightToLeft,
    };

    Ok(Manga {
        key,
        cover,
        title,
        description,
        url: Some(url),
        tags: if tags.is_empty() { None } else { Some(tags) },
        status,
        content_rating,
        viewer,
        ..Default::default()
    })
}

pub fn get_chapter_list(html: Document) -> Result<Vec<Chapter>> {
    let mut chapters: Vec<Chapter> = Vec::new();

    if let Some(items) = html.select(".p-1") {
        for obj in items {
            let key = obj.attr("href").unwrap_or_default();
            if key.is_empty() || key == "Read Chapters" {
                continue;
            }

            let url = format!("{}{}", BASE_URL, &key);

            // parse chapter number from the last segment after the last '-'
            // e.g. /chapters/6290-10006000/one-piece-pirate-recipes-chapter-6 -> 6
            let chapter_number = key
                .rsplit('-')
                .next()
                .and_then(|s| s.parse::<f32>().ok());

            chapters.push(Chapter {
                key,
                chapter_number,
                url: Some(url),
                language: Some(String::from("en")),
                ..Default::default()
            });
        }
    }

    Ok(chapters)
}

pub fn get_page_list(html: Document) -> Result<Vec<Page>> {
    let mut pages: Vec<Page> = Vec::new();

    if let Some(imgs) = html.select("picture img") {
        for img in imgs {
            let url = img.attr("data-src").unwrap_or_default();
            if !url.is_empty() {
                pages.push(Page {
                    content: PageContent::url(url),
                    ..Default::default()
                });
            }
        }
    }

    Ok(pages)
}

pub fn get_filtered_url(query: Option<String>, filters: Vec<FilterValue>, page: i32) -> String {
    let mut is_searching = false;
    let mut qs = String::new();
    let mut search_string = String::new();

    // Handle text query
    if let Some(ref q) = query {
        if !q.is_empty() {
            search_string.push_str(&urlencode(q.to_lowercase()));
            is_searching = true;
        }
    }

    for filter in filters {
        match filter {
            FilterValue::Select { id, value } => {
                if id == "type" {
                    let type_val = match value.parse::<i32>().unwrap_or(0) {
                        1 => "manga",
                        2 => "novel",
                        3 => "one-shot",
                        4 => "doujinshi",
                        5 => "manhwa",
                        6 => "manhua",
                        7 => "oel",
                        _ => "",
                    };
                    if !type_val.is_empty() {
                        qs.push_str("&type=");
                        qs.push_str(type_val);
                        is_searching = true;
                    }
                } else if id == "status" {
                    let status_val = match value.parse::<i32>().unwrap_or(0) {
                        1 => "publishing",
                        2 => "finished",
                        3 => "on+haitus",
                        4 => "discontinued",
                        _ => "",
                    };
                    if !status_val.is_empty() {
                        qs.push_str("&status=");
                        qs.push_str(status_val);
                        is_searching = true;
                    }
                }
            }
            _ => {}
        }
    }

    if is_searching {
        let mut url = String::from(BASE_URL);
        url.push_str("/search?q=");
        url.push_str(&search_string);
        url.push_str(&qs);
        url.push_str("&page=");
        url.push_str(&page.to_string());
        url
    } else {
        // default recents/browse page
        String::from(BASE_URL)
    }
}

pub fn parse_incoming_url(url: String) -> String {
    // https://mangapill.com/manga/6290/one-piece-pirate-recipes
    // https://mangapill.com/chapters/6290-10006000/one-piece-pirate-recipes-chapter-6
    let segments: Vec<&str> = url.split('/').collect();

    if url.contains("/chapters/") {
        // Find the segment right after "chapters" and extract manga id
        let mut manga_id = "";
        for (i, seg) in segments.iter().enumerate() {
            if *seg == "chapters" && i + 1 < segments.len() {
                manga_id = segments[i + 1].split('-').next().unwrap_or("");
                break;
            }
        }
        // Build key like /manga/6290/slug using last segment as slug
        let slug = segments.last().copied().unwrap_or("");
        let mut key = String::from("/manga/");
        key.push_str(manga_id);
        key.push('/');
        key.push_str(slug);
        key
    } else if let Some(pos) = url.find("/manga/") {
        // /manga/6290/one-piece-pirate-recipes -> key is /manga/6290/one-piece-pirate-recipes
        url[pos..].trim_end_matches('/').to_string()
    } else {
        // fallback: build from last two segments
        let mut key = String::from("/manga/");
        if segments.len() >= 2 {
            key.push_str(segments[segments.len() - 2]);
            key.push('/');
            key.push_str(segments[segments.len() - 1]);
        }
        key
    }
}

fn urlencode(string: String) -> String {
    let mut result: Vec<u8> = Vec::with_capacity(string.len() * 3);
    let hex = b"0123456789abcdef";
    let bytes = string.as_bytes();

    for byte in bytes {
        let curr = *byte;
        if curr.is_ascii_lowercase() || curr.is_ascii_uppercase() || curr.is_ascii_digit() {
            result.push(curr);
        } else {
            result.push(b'%');
            result.push(hex[(curr >> 4) as usize]);
            result.push(hex[(curr & 15) as usize]);
        }
    }

    String::from_utf8(result).unwrap_or_default()
}
