#![no_std]
use aidoku::{
	Chapter, FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult,
	MangaStatus, Page, PageContent, PageContext, Result, Source, Viewer,
	alloc::{String, Vec, string::ToString},
	helpers::uri::encode_uri_component,
	imports::{
		html::{Document, Html},
		js::WebView,
		net::Request,
		std::{parse_date, print, sleep},
	},
	prelude::*,
};

const BASE_URL: &str = "https://readcomicsonline.ru";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
// Self-hosted FlareSolverr on the homelab, reachable over Tailscale. Only ever hit if
// the fast path 403s; if it's unreachable (off the tailnet, box down), every call here
// just fails fast and the on-device WebView fallback takes over - never a hard dependency.
const FLARESOLVERR_URL: &str = "http://100.93.178.119:8191/v1";

struct ReadComicOnline;

// Ask FlareSolverr (real desktop Chrome, better IP reputation than a mobile WebView) to
// solve the challenge and hand back the resolved page body. Returns None on any failure
// so callers can fall through to the WebView path without special-casing errors.
fn fetch_via_flaresolverr(url: &str) -> Option<String> {
	let escaped_url = url.replace('\\', "\\\\").replace('"', "\\\"");
	let body = format!(r#"{{"cmd":"request.get","url":"{escaped_url}","maxTimeout":30000}}"#);
	print(format!("[RCO] trying flaresolverr for {url}"));
	let response = Request::post(FLARESOLVERR_URL)
		.ok()?
		.header("Content-Type", "application/json")
		.body(body.as_bytes())
		.send()
		.ok()?;
	if response.status_code() != 200 {
		print(format!(
			"[RCO] flaresolverr http status={}",
			response.status_code()
		));
		return None;
	}
	let text = response.get_string().ok()?;
	let json: serde_json::Value = serde_json::from_str(&text).ok()?;
	if json.get("status")?.as_str()? != "ok" {
		print(format!("[RCO] flaresolverr status={:?}", json.get("message")));
		return None;
	}
	let html = json.get("solution")?.get("response")?.as_str()?.to_string();
	print(format!("[RCO] flaresolverr succeeded, length={}", html.len()));
	Some(html)
}

// The challenge page itself finishes "loading" (and unblocks load_blocking) well before
// its JS actually computes the proof-of-work and reloads into the real page - so after
// the initial load, poll the live title and give it real wall-clock time to redirect
// before giving up and reading whatever's there.
fn wait_past_cloudflare(wv: &WebView) {
	for i in 0..5 {
		let title = wv.eval("document.title").unwrap_or_default();
		print(format!("[RCO] cf-wait #{i}: title={title:?}"));
		if !title.contains("Just a moment") {
			return;
		}
		sleep(3);
	}
	print("[RCO] cf-wait: gave up after 5 tries");
}

// The site is behind Cloudflare's automatic JS challenge: a plain HTTP request gets a
// non-200 response until the challenge script runs. Try a cheap direct request first
// (works once a prior WebView load has warmed up the session's clearance cookie), and
// only fall back to a real WebView load - slow, but capable of running the challenge JS -
// when the fast path doesn't come back with a 200.
fn fetch_document(url: &str) -> Result<Document> {
	print(format!("[RCO] fetch_document: {url}"));
	let request = Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.header("User-Agent", USER_AGENT);
	if let Ok(response) = request.send() {
		let status = response.status_code();
		print(format!("[RCO] fast path status={status}"));
		if status == 200 {
			if let Ok(doc) = response.get_html() {
				print("[RCO] fast path succeeded");
				return Ok(doc);
			}
			print("[RCO] fast path get_html() failed to parse");
		}
	} else {
		print("[RCO] fast path request.send() errored");
	}

	if let Some(html) = fetch_via_flaresolverr(url) {
		return Html::parse_with_url(html, url).map_err(|_| error!("failed to parse page"));
	}

	print("[RCO] falling back to WebView");
	let wv = WebView::new();
	// Deliberately bare: no forced User-Agent (WebView is a real mobile WebKit engine,
	// claiming "desktop Chrome" is a fingerprint mismatch) and no forced Referer (a fresh
	// top-level load has no real previous page, so a real browser wouldn't send one
	// either). Let the WebView look exactly like what it actually is.
	wv.load_blocking(Request::get(url)?)?;
	wait_past_cloudflare(&wv);
	let html = wv.eval("document.documentElement.outerHTML")?;
	print(format!("[RCO] webview html length={}", html.len()));
	Html::parse_with_url(html, url).map_err(|_| error!("failed to parse page"))
}

fn fetch_text(url: &str) -> Result<String> {
	print(format!("[RCO] fetch_text: {url}"));
	let request = Request::get(url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.header("User-Agent", USER_AGENT);
	if let Ok(response) = request.send() {
		let status = response.status_code();
		print(format!("[RCO] fast path status={status}"));
		if status == 200 {
			if let Ok(text) = response.get_string() {
				print("[RCO] fast path succeeded");
				return Ok(text);
			}
		}
	} else {
		print("[RCO] fast path request.send() errored");
	}

	if let Some(text) = fetch_via_flaresolverr(url) {
		// Chrome (which FlareSolverr drives) wraps non-HTML responses like our JSON
		// endpoint in its own viewer: <html>...<body><pre>{...}</pre></body></html>.
		// Unwrap that back to the raw body if present.
		let unwrapped = text
			.find("<pre")
			.and_then(|start| text[start..].find('>').map(|i| start + i + 1))
			.and_then(|body_start| {
				text[body_start..]
					.find("</pre>")
					.map(|end| text[body_start..body_start + end].to_string())
			})
			.unwrap_or(text);
		return Ok(unwrapped);
	}

	print("[RCO] falling back to WebView");
	let wv = WebView::new();
	wv.load_blocking(Request::get(url)?)?;
	wait_past_cloudflare(&wv);
	let text = wv
		.eval("document.body.textContent||document.body.innerText||''")
		.unwrap_or_default();
	print(format!("[RCO] webview text length={}", text.len()));
	Ok(text)
}

impl Source for ReadComicOnline {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(query) = query.filter(|q| !q.is_empty()) {
			let url = format!("{BASE_URL}/search?query={}", encode_uri_component(&query));
			let text = fetch_text(&url)?;
			return Ok(parse_search_suggestions(&text));
		}

		let url = format!("{BASE_URL}/comic-list?page={page}");
		let html = fetch_document(&url)?;
		Ok(parse_comic_list(html, page))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = fetch_document(&url)?;

		if needs_details {
			manga.title = html
				.select_first("h1")
				.and_then(|el| el.text())
				.unwrap_or(manga.title);
			manga.cover = html
				.select_first("img[src*='/cover/']")
				.and_then(|el| el.attr("abs:src"));
			print(format!("[RCO] cover={:?}", manga.cover));
			manga.description = html.select_first("p.leading-relaxed").and_then(|el| el.text());
			manga.tags = html
				.select("a[href*='/comic-list/category/']")
				.map(|els| els.filter_map(|el| el.text()).collect::<Vec<_>>());
			manga.status = html
				.select("span")
				.map(|els| {
					els.filter_map(|el| el.text()).find_map(|text| {
						if text == "Ongoing" {
							Some(MangaStatus::Ongoing)
						} else if text == "Completed" {
							Some(MangaStatus::Completed)
						} else {
							None
						}
					})
				})
				.unwrap_or_default()
				.unwrap_or(MangaStatus::Unknown);
			manga.viewer = Viewer::LeftToRight;
		}

		if needs_chapters {
			manga.chapters = html.select("a:has(span.text-brand-400)").map(|els| {
				els.filter_map(|el| {
					let url = el.attr("abs:href")?;
					let key = url.strip_prefix(BASE_URL)?.into();
					let number_text = el.select_first("span.text-brand-400")?.text()?;
					let chapter_number = number_text.trim_start_matches('#').trim().parse::<f32>().ok();
					let title = el.select_first("span.font-medium").and_then(|el| el.text());
					let date_uploaded = el
						.select_first("span.text-xs.text-slate-500")
						.and_then(|el| el.text())
						.and_then(|str| parse_date(str, "d MMM yyyy"));
					Some(Chapter {
						key,
						title,
						chapter_number,
						date_uploaded,
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
			})
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		let html = fetch_document(&url)?;

		let links = html
			.select("img[src*='/chapters/']")
			.map(|els| {
				els.filter_map(|el| el.attr("abs:src"))
					.collect::<Vec<String>>()
			})
			.unwrap_or_default();
		print(format!("[RCO] get_page_list: {} images found", links.len()));
		if let Some(first) = links.first() {
			print(format!("[RCO] first image: {first}"));
		}

		Ok(links
			.into_iter()
			.map(|link| Page {
				content: PageContent::url(link),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for ReadComicOnline {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = format!("{BASE_URL}/comic-list/category/{}?page={page}", listing.id);
		let html = fetch_document(&url)?;
		Ok(parse_comic_list(html, page))
	}
}

fn parse_search_suggestions(text: &str) -> MangaPageResult {
	let entries = serde_json::from_str::<serde_json::Value>(text)
		.ok()
		.and_then(|json| json.get("suggestions").cloned())
		.and_then(|suggestions| suggestions.as_array().cloned())
		.map(|suggestions| {
			suggestions
				.into_iter()
				.filter_map(|s| {
					let title = s.get("value")?.as_str()?.to_string();
					let slug = s.get("data")?.as_str()?.to_string();
					let cover = s
						.get("cover")
						.and_then(|c| c.as_str())
						.map(|s| s.to_string());
					let url = s
						.get("url")
						.and_then(|u| u.as_str())
						.map(|s| s.to_string())
						.unwrap_or_else(|| format!("{BASE_URL}/comic/{slug}"));
					Some(Manga {
						key: format!("/comic/{slug}"),
						title,
						cover,
						url: Some(url),
						..Default::default()
					})
				})
				.collect::<Vec<Manga>>()
		})
		.unwrap_or_default();

	MangaPageResult {
		entries,
		has_next_page: false,
	}
}

fn parse_comic_list(html: Document, page: i32) -> MangaPageResult {
	let entries = html
		.select("a.line-clamp-2")
		.map(|elements| {
			elements
				.filter_map(|element| {
					let url = element.attr("abs:href")?;
					let key = url.strip_prefix(BASE_URL).map(String::from)?;
					let title = element.text().unwrap_or_default();
					let cover = element
						.parent()
						.and_then(|el| el.parent())
						.and_then(|el| el.select_first("img"))
						.and_then(|el| el.attr("abs:src"));
					Some(Manga {
						key,
						title,
						cover,
						url: Some(url),
						..Default::default()
					})
				})
				.collect::<Vec<Manga>>()
		})
		.unwrap_or_default();

	let next_page = page + 1;
	let has_next_page = html
		.select_first(&format!("a[href*='page={next_page}']"))
		.is_some();

	MangaPageResult {
		entries,
		has_next_page,
	}
}

impl ImageRequestProvider for ReadComicOnline {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		// cdn.readcomicsonline.ru doesn't need Referer/User-Agent (unlike the old
		// blogspot-hosted images) - a bare request works. Just log what's requested.
		print(format!("[RCO] get_image_request: {url}"));
		Ok(Request::get(url)?)
	}
}

register_source!(ReadComicOnline, ListingProvider, ImageRequestProvider);
