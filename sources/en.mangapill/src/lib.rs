#![no_std]
use aidoku::{
    alloc::{String, Vec},
    imports::net::Request,
    prelude::*,
    Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Manga,
    MangaPageResult, Page, PageContext, Result, Source,
};

mod parser;

const BASE_URL: &str = "https://www.mangapill.com";

struct MangaPill;

impl Source for MangaPill {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let url = parser::get_filtered_url(query.clone(), filters, page);
        let html = Request::get(&url)?
            .header("User-Agent", parser::USER_AGENT)
            .html()?;

        let entries = if url.contains("/search") {
            parser::parse_search(&html)
        } else {
            parser::parse_recents(&html)
        };

        let has_next_page = entries.len() >= 50;

        Ok(MangaPageResult {
            entries,
            has_next_page,
        })
    }

    fn get_manga_update(
        &self,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        let url = format!("{}{}", BASE_URL, manga.key);

        if needs_details {
            let html = Request::get(&url)?
                .header("User-Agent", parser::USER_AGENT)
                .html()?;
            manga = parser::parse_manga(html, manga.key.clone())?;
        }

        if needs_chapters {
            let html = Request::get(&url)?
                .header("User-Agent", parser::USER_AGENT)
                .html()?;
            manga.chapters = Some(parser::get_chapter_list(html)?);
        }

        Ok(manga)
    }

    fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let url = format!("{}{}", BASE_URL, chapter.key);
        let html = Request::get(&url)?
            .header("Referer", BASE_URL)
            .header("User-Agent", parser::USER_AGENT)
            .html()?;
        parser::get_page_list(html)
    }
}

impl ImageRequestProvider for MangaPill {
    fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
        Ok(Request::get(&url)?
            .header("Referer", BASE_URL)
            .header("User-Agent", parser::USER_AGENT))
    }
}

impl DeepLinkHandler for MangaPill {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        let key = parser::parse_incoming_url(url);
        Ok(Some(DeepLinkResult::Manga { key }))
    }
}

register_source!(MangaPill, ImageRequestProvider, DeepLinkHandler);
