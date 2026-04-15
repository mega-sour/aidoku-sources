#![no_std]
use aidoku::{
    alloc::{String, Vec},
    imports::net::Request,
    prelude::*,
    Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
    ListingProvider, Manga, MangaPageResult, Page, PageContext, Result, Source,
};

mod helper;
mod parser;

use helper::get_base_url;

struct Webtoon;

impl Source for Webtoon {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        _page: i32,
        mut filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        // If a query string is present, inject it as a Text filter
        if let Some(q) = query {
            if !q.is_empty() {
                filters.push(FilterValue::Text {
                    id: String::from("title"),
                    value: q,
                });
            }
        }
        let base_url = get_base_url(false);
        parser::parse_manga_list(&base_url, filters)
    }

    fn get_manga_update(
        &self,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        let base_url = get_base_url(false);
        if needs_details {
            manga = parser::parse_manga_details(&base_url, manga.key.clone())?;
        }
        if needs_chapters {
            manga.chapters = Some(parser::parse_chapter_list(manga.key.clone())?);
        }
        Ok(manga)
    }

    fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let base_url = get_base_url(false);
        parser::parse_page_list(&base_url, &manga.key, &chapter.key)
    }
}

impl ListingProvider for Webtoon {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let base_url = get_base_url(false);
        parser::parse_manga_listing(base_url, &listing.id, page)
    }
}

impl ImageRequestProvider for Webtoon {
    fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
        let base_url = get_base_url(false);
        parser::get_image_request(&base_url, url)
    }
}

impl DeepLinkHandler for Webtoon {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        let key = parser::handle_url(url)?;
        if key.is_empty() {
            Ok(None)
        } else {
            Ok(Some(DeepLinkResult::Manga { key }))
        }
    }
}

register_source!(Webtoon, ListingProvider, ImageRequestProvider, DeepLinkHandler);
