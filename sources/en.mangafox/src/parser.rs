use aidoku::{
    alloc::{String, Vec, string::ToString},
    helpers::uri::encode_uri,
    imports::html::Document,
    prelude::*,
    Chapter, ContentRating, FilterValue, Manga, MangaPageResult, MangaStatus, Page, PageContent,
    Result, Viewer,
};

use crate::BASE_URL;

pub fn parse_directory(html: Document) -> Result<MangaPageResult> {
    let has_next_page = !is_last_page(&html);

    let entries = html
        .select("ul.line li")
        .map(|els| {
            els.filter_map(|obj| {
                let a = obj.select_first("a")?;
                let href = a.attr("href").unwrap_or_default();
                let key = href
                    .trim_start_matches("/manga/")
                    .trim_end_matches('/')
                    .to_string();
                let title = a.attr("title").unwrap_or_default();
                let cover = obj.select_first("a img").and_then(|img| img.attr("src"));
                Some(Manga {
                    key,
                    title,
                    cover,
                    status: MangaStatus::Unknown,
                    content_rating: ContentRating::Safe,
                    viewer: Viewer::RightToLeft,
                    ..Default::default()
                })
            })
            .collect::<Vec<Manga>>()
        })
        .unwrap_or_default();

    Ok(MangaPageResult {
        entries,
        has_next_page,
    })
}

pub fn parse_manga(html: Document, key: String) -> Result<Manga> {
    let cover = html
        .select_first(".detail-info-cover-img")
        .and_then(|el| el.attr("src"))
        .unwrap_or_default();
    let title = html
        .select_first("span.detail-info-right-title-font")
        .and_then(|el| el.text())
        .unwrap_or_default();
    let author = html
        .select_first("p.detail-info-right-say a")
        .and_then(|el| el.text());
    let description = html
        .select_first("p.fullcontent")
        .and_then(|el| el.text());

    let url = format!("{BASE_URL}/manga/{}", &key);

    let mut viewer = Viewer::RightToLeft;
    let mut content_rating = ContentRating::Safe;
    let mut tags: Vec<String> = Vec::new();

    if let Some(tag_els) = html.select(".detail-info-right-tag-list a") {
        for tag_el in tag_els {
            let tag = tag_el.text().unwrap_or_default();
            let tag = tag.trim().to_string();
            if tag == "Ecchi" || tag == "Mature" || tag == "Smut" || tag == "Adult" {
                content_rating = ContentRating::NSFW;
            }
            if tag == "Webtoons" {
                viewer = Viewer::Webtoon;
            }
            tags.push(tag);
        }
    }

    let status_str = html
        .select_first(".detail-info-right-title-tip")
        .and_then(|el| el.text())
        .unwrap_or_default()
        .to_lowercase();
    let status = if status_str.contains("ongoing") {
        MangaStatus::Ongoing
    } else if status_str.contains("completed") {
        MangaStatus::Completed
    } else {
        MangaStatus::Unknown
    };

    let authors = if author.is_some() {
        Some(author.into_iter().collect())
    } else {
        None
    };

    Ok(Manga {
        key,
        cover: Some(cover),
        title,
        authors,
        description,
        url: Some(url),
        tags: if tags.is_empty() { None } else { Some(tags) },
        status,
        content_rating,
        viewer,
        ..Default::default()
    })
}

pub fn parse_chapters(html: Document) -> Result<Vec<Chapter>> {
    let mut chapters: Vec<Chapter> = Vec::new();

    if let Some(items) = html.select(".detail-main-list li") {
        for item in items {
            let a = match item.select_first("a") {
                Some(a) => a,
                None => continue,
            };
            let href = a.attr("href").unwrap_or_default();
            // href is like /manga/solo_leveling/v1/c1/1.html
            let key = href
                .trim_start_matches("/manga/")
                .trim_end_matches("/1.html")
                .to_string();

            let url = format!("{BASE_URL}/manga/{}", &key);

            // parse title from ".title3": e.g. "Vol.1 Ch.1 - Some Title"
            let title_str = item
                .select_first(".title3")
                .and_then(|el| el.text())
                .unwrap_or_default();
            let title = {
                let parts: Vec<&str> = title_str.as_str().splitn(2, '-').collect();
                if parts.len() > 1 {
                    parts[1].trim().to_string()
                } else {
                    String::new()
                }
            };
            let title = if title.is_empty() { None } else { Some(title) };

            // parse volume and chapter from path segments
            let mut volume_number: Option<f32> = None;
            let mut chapter_number: Option<f32> = None;

            for segment in key.as_str().split('/') {
                match segment.chars().next() {
                    Some('v') => {
                        volume_number = segment.trim_start_matches('v').parse::<f32>().ok();
                    }
                    Some('c') => {
                        chapter_number = segment.trim_start_matches('c').parse::<f32>().ok();
                    }
                    _ => {}
                }
            }

            // parse date from ".title2"
            let date_uploaded = item
                .select_first(".title2")
                .and_then(|el| el.text())
                .and_then(|s| aidoku::imports::std::parse_date(s, "MMM dd,yyyy"));

            chapters.push(Chapter {
                key,
                title,
                volume_number,
                chapter_number,
                date_uploaded,
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

    if let Some(imgs) = html.select("#viewer img") {
        for img in imgs {
            let raw = img
                .attr("data-original")
                .unwrap_or_default();
            let url = format!("https://{}", raw.trim_start_matches("//"));
            pages.push(Page {
                content: PageContent::url(url),
                ..Default::default()
            });
        }
    }

    if pages.is_empty() {
        pages.push(Page {
            content: PageContent::url("https://i.imgur.com/5mNXCgV.png"),
            ..Default::default()
        });
    }

    Ok(pages)
}

pub fn get_filtered_url(query: Option<String>, filters: Vec<FilterValue>, page: i32) -> String {
    let mut is_searching = false;
    let mut search_query = String::new();
    let mut url = String::from(BASE_URL);

    let mut genres = String::new();
    let mut nogenres = String::new();

    // If there's a text query, treat it as a search
    if let Some(ref q) = query {
        if !q.is_empty() {
            search_query.push_str("&title=");
            search_query.push_str(&q.to_lowercase());
            is_searching = true;
        }
    }

    for filter in filters {
        match filter {
            FilterValue::Text { id: _, value } => {
                if !value.is_empty() {
                    search_query.push_str("&title=");
                    search_query.push_str(&value.to_lowercase());
                    is_searching = true;
                }
            }
            FilterValue::Select { id, value } => {
                if id == "Language" {
                    if let Ok(v) = value.parse::<i32>() {
                        if v > 0 {
                            search_query.push_str("&type=");
                            search_query.push_str(&v.to_string());
                            is_searching = true;
                        }
                    }
                } else if id == "Rating" {
                    if let Ok(v) = value.parse::<i32>() {
                        if v > 0 {
                            search_query.push_str("&rating_method=eq&rating=");
                            search_query.push_str(&v.to_string());
                            is_searching = true;
                        }
                    }
                } else if id == "Completed" {
                    if let Ok(v) = value.parse::<i32>() {
                        search_query.push_str("&st=");
                        search_query.push_str(&v.to_string());
                        if v > 0 {
                            is_searching = true;
                        }
                    }
                }
            }
            FilterValue::Check { id, value } => {
                // genre check: value 0 = exclude, 1 = include, -1 = ignore
                if value == 0 {
                    nogenres.push_str(&id);
                    nogenres.push(',');
                    is_searching = true;
                } else if value == 1 {
                    genres.push_str(&id);
                    genres.push(',');
                    is_searching = true;
                }
            }
            FilterValue::MultiSelect {
                included,
                excluded,
                ..
            } => {
                for g in &included {
                    genres.push_str(g);
                    genres.push(',');
                    is_searching = true;
                }
                for g in &excluded {
                    nogenres.push_str(g);
                    nogenres.push(',');
                    is_searching = true;
                }
            }
            _ => {}
        }
    }

    if is_searching {
        let search_string = if page == 1 {
            format!(
                "/search?title=&stype=1&author_method=cw&author=&artist_method=cw&released_method=eq&released=&genres={}&nogenres={}{search_query}",
                genres.trim_end_matches(','),
                nogenres.trim_end_matches(','),
            )
        } else {
            format!(
                "/search?page={page}&author_method=cw&author=&artist_method=cw&genres={}&nogenres={}&released_method=eq&released=&stype=1{search_query}",
                genres.trim_end_matches(','),
                nogenres.trim_end_matches(','),
            )
        };
        url.push_str(search_string.as_str());
    } else {
        let list_string = format!("/directory?page={}.html?rating", &page.to_string());
        url.push_str(list_string.as_str());
    }

    encode_uri(url)
}

pub fn parse_incoming_url(url: String) -> String {
    // https://fanfox.net/manga/solo_leveling
    // https://fanfox.net/manga/solo_leveling/c183/1.html#ipg2
    // https://m.fanfox.net/manga/chainsaw_man/
    // https://m.fanfox.net/manga/onepunch_man/vTBD/c178/1.html
    let after = match url.find("/manga/") {
        Some(pos) => &url[pos + "/manga/".len()..],
        None => return url,
    };
    // strip anything after the first '/'
    let manga_id = match after.find('/') {
        Some(pos) => &after[..pos],
        None => after.trim_end_matches('/'),
    };
    manga_id.to_string()
}

pub fn is_last_page(html: &Document) -> bool {
    if let Some(pages) = html.select("div.pager-list-left a") {
        let count = pages.size();
        if count == 0 {
            return false;
        }
        // re-select to iterate (ElementList is consumed on iteration)
        if let Some(pages2) = html.select("div.pager-list-left a") {
            for (index, page) in pages2.enumerate() {
                let href = page.attr("href").unwrap_or_default();
                if index == count - 1 && href == "javascript:void(0)" {
                    return true;
                }
            }
        }
    }
    false
}
