#![no_std]
use aidoku::{
    alloc::{String, Vec},
    imports::net::Request,
    prelude::*,
    Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
    ListingProvider, Manga, MangaPageResult, Page, PageContext, Result, Source,
};

mod parser;

const BASE_URL: &str = "https://fanfox.net";

struct MangaFox;

impl Source for MangaFox {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let url = parser::get_filtered_url(query, filters, page);
        let html = Request::get(url)?.html()?;
        parser::parse_directory(html)
    }

    fn get_manga_update(
        &self,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        let url = format!("{BASE_URL}/manga/{}", manga.key);
        if needs_details {
            let html = Request::get(&url)?.html()?;
            manga = parser::parse_manga(html, manga.key.clone())?;
        }
        if needs_chapters {
            let html = Request::get(&url)?
                .header("Cookie", "isAdult=1")
                .html()?;
            manga.chapters = Some(parser::parse_chapters(html)?);
        }
        Ok(manga)
    }

    fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let url = format!("https://m.fanfox.net/roll_manga/{}/1.html", chapter.key);
        let html = Request::get(url)?
            .header("Cookie", "readway=2")
            .html()?;
        parser::get_page_list(html)
    }
}

impl ListingProvider for MangaFox {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let url_query = match listing.id.as_str() {
            "latest" => "latest",
            "rating" => "rating",
            _ => "",
        };
        let url = format!("{BASE_URL}/directory/updated/{page}.html?{url_query}");
        let html = Request::get(url)?.html()?;
        parser::parse_directory(html)
    }
}

impl ImageRequestProvider for MangaFox {
    fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
        Ok(Request::get(&url)?.header("Referer", "https://m.fanfox.net/"))
    }
}

impl DeepLinkHandler for MangaFox {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        let key = parser::parse_incoming_url(url);
        Ok(Some(DeepLinkResult::Manga { key }))
    }
}

register_source!(MangaFox, ListingProvider, ImageRequestProvider, DeepLinkHandler);
