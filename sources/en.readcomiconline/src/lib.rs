#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
	HomeLayout, Listing, ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent,
	Result, Source, Viewer,
	alloc::{String, Vec, format, string::ToString, vec},
	helpers::{
		string::StripPrefixOrSelf,
		uri::encode_uri_component,
	},
	imports::{
		html::Document,
		js::JsContext,
		net::Request,
		std::{parse_date, send_partial_result},
	},
	prelude::*,
};

const BASE_URL: &str = "https://readcomicsonline.ru";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";

struct ReadComicOnline;

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
		if let Some(ref q) = query {
			let url = format!("{BASE_URL}/search?query={}", encode_uri_component(q));
			let text = Request::get(&url)?
				.header("Referer", &format!("{BASE_URL}/"))
				.header("User-Agent", USER_AGENT)
				.string()?;
			Ok(parse_search_suggestions(&text))
		} else {
			let url = format!("{BASE_URL}/filterList?page={page}&sortBy=views&asc=false");
			let html = Request::get(&url)?
				.header("Referer", &format!("{BASE_URL}/"))
				.header("User-Agent", USER_AGENT)
				.html()?;
			Ok(parse_comic_list(html))
		}
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = Request::get(&url)?
			.header("Referer", &format!("{BASE_URL}/"))
			.header("User-Agent", USER_AGENT)
			.html()?;

		if needs_details {
			manga.title = html
				.select_first("h2.listmanga-header")
				.and_then(|el| el.text())
				.unwrap_or(manga.title);
			manga.cover = html
				.select_first("img.img-responsive")
				.and_then(|el| el.attr("abs:src"));
			manga.authors = html
				.select_first("a[href*='/author/']")
				.and_then(|el| el.text())
				.map(|s| vec![s]);
			manga.description = html
				.select_first("div.well p")
				.and_then(|el| el.text());
			manga.tags = html
				.select("dd.tag-links > a")
				.map(|els| els.filter_map(|el| el.text()).collect::<Vec<_>>());
			manga.status = html
				.select_first("dd > span.label")
				.and_then(|el| el.text())
				.map(|s| {
					if s.contains("Ongoing") {
						MangaStatus::Ongoing
					} else if s.contains("Completed") {
						MangaStatus::Completed
					} else {
						MangaStatus::Unknown
					}
				})
				.unwrap_or_default();
			manga.viewer = Viewer::LeftToRight;

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = html.select("ul.chapters > li").map(|els| {
				els.filter_map(|el| {
					let anchor = el.select_first("h5.chapter-title-rtl > a")?;
					let url = anchor.attr("abs:href")?;
					let key = url.strip_prefix(BASE_URL)?.into();

					let mut chapter_number = None;
					let title = anchor.text().map(|text| {
						let text = text.strip_prefix_or_self(&manga.title).trim();
						if let Some(idx) = text.find('#') {
							chapter_number = text[idx + 1..].parse::<f32>().ok();
						}
						text.into()
					});

					Some(Chapter {
						key,
						title,
						chapter_number,
						date_uploaded: el
							.select_first(".date-chapter-title-rtl")
							.and_then(|el| el.text())
							.and_then(|s| parse_date(s, "d MMM. yyyy")),
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		let html = Request::get(url)?
			.header("Referer", &format!("{BASE_URL}/"))
			.header("User-Agent", USER_AGENT)
			.html()?;

		// todo: if the site changes often, this may need to be put in a separate file to request so that it can be updated without users updating the source
		// (this is what the mihon source does)
		const IMG_DECRYPT_EVAL: &str = "const assignRegex = /(_[^\\s=]*xnz)\\s*=\\s*['\"]([^'\"]+)['\"]/g;const matches = [..._encryptedString.matchAll(assignRegex)];const pageLinks = matches.map(m => decryptLink(m[2]));function atob(t){const e=\"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=\";let s=String(t).replace(/=+$/,\"\");if(s.length%4===1)throw new Error(\"'atob' failed: The string to be decoded is not correctly encoded.\");let n=\"\";for(let t=0,r,c,i=0;c=s.charAt(i++);~c&&(r=t%4?r*64+c:c,t++%4)?n+=String.fromCharCode(255&r>>(-2*t&6)):0)c=e.indexOf(c);return n}function decryptLink(t){let e=t.replace(/\\w{5}__\\w{3}__/g,\"g\").replace(/\\w{2}__\\w{6}_/g,\"g\").replace(/b/g,\"pw_.g28x\").replace(/h/g,\"d2pr.x_27\").replace(/pw_.g28x/g,\"b\").replace(/d2pr.x_27/g,\"h\");if(!e.startsWith(\"https\")){const t=e.indexOf(\"?\");const s=e.substring(t);const n=e.includes(\"=s0?\");const r=n?e.indexOf(\"=s0?\"):e.indexOf(\"=s1600?\");let c=e.substring(0,r);c=c.substring(15,33)+c.substring(50);const i=c.length;c=c.substring(0,i-11)+c[i-2]+c[i-1];const g=atob(c);let o=decodeURIComponent(g);o=o.substring(0,13)+o.substring(17);o=o.substring(0,o.length-2)+(n?\"=s0\":\"=s1600\");const a=!_useServer2?\"https://2.bp.blogspot.com\":\"https://img1.whatsnew247.net/pic\";e=`${a}/${o}${s}${_useServer2?\"&t=10\":\"\"}`}return e}JSON.stringify(pageLinks);";

		let scripts = html
			.select("script")
			.ok_or(error!("html select `script` failed"))?;

		let mut links = Vec::new();

		for script in scripts {
			let Some(data) = script.data().and_then(|s| {
				let s = s.trim();
				if s.is_empty() {
					return None;
				}
				serde_json::to_string(&s).ok()
			}) else {
				continue;
			};

			let js_string =
				format!("let _encryptedString = {data};let _useServer2 = false;{IMG_DECRYPT_EVAL}");
			let result = JsContext::new().eval(&js_string)?;

			if result.starts_with('[') && result.ends_with(']') {
				let new_links: Vec<String> = result[1..result.len() - 1]
					.split(',')
					.map(|s| s.trim_matches(|c| c == '"' || c == '\''))
					.filter(|s| !s.is_empty())
					.map(|s| s.to_string())
					.collect();
				links.extend(new_links);
			}
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
		let cat = match listing.id.as_str() {
			"marvel" => "34",
			"dc" => "33",
			_ => "",
		};
		let url = if cat.is_empty() {
			format!("{BASE_URL}/filterList?page={page}&sortBy=views&asc=false")
		} else {
			format!("{BASE_URL}/filterList?page={page}&sortBy=views&asc=false&cat={cat}")
		};
		let html = Request::get(url)?
			.header("Referer", &format!("{BASE_URL}/"))
			.header("User-Agent", USER_AGENT)
			.html()?;
		Ok(parse_comic_list(html))
	}
}

fn parse_search_suggestions(text: &str) -> MangaPageResult {
	let mut entries = Vec::new();
	let mut remaining = text;

	while let Some(value_pos) = remaining.find("\"value\":\"") {
		remaining = &remaining[value_pos + 9..];
		let Some(value_end) = remaining.find('"') else {
			break;
		};
		let title = &remaining[..value_end];
		remaining = &remaining[value_end..];

		let Some(data_pos) = remaining.find("\"data\":\"") else {
			break;
		};
		remaining = &remaining[data_pos + 8..];
		let Some(data_end) = remaining.find('"') else {
			break;
		};
		let slug = &remaining[..data_end];
		remaining = &remaining[data_end..];

		let cover = format!("{BASE_URL}/uploads/manga/{slug}/cover/cover_250x350.jpg");
		let url = format!("{BASE_URL}/comic/{slug}");
		entries.push(Manga {
			key: format!("/comic/{slug}"),
			title: title.into(),
			cover: Some(cover),
			url: Some(url),
			..Default::default()
		});
	}

	MangaPageResult {
		entries,
		has_next_page: false,
	}
}

fn parse_comic_list(html: Document) -> MangaPageResult {
	let entries = html
		.select("div.media")
		.map(|elements| {
			elements
				.filter_map(|el| {
					let link = el.select_first("div.media-left > a")?;
					let url = link.attr("abs:href")?;
					let key = url.strip_prefix(BASE_URL).map(String::from)?;
					let title = el
						.select_first("a.chart-title")
						.and_then(|e| e.text())
						.unwrap_or_default();
					let cover = el
						.select_first("div.media-left img")
						.and_then(|e| e.attr("abs:src"));
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

	let has_next_page = !entries.is_empty();

	MangaPageResult {
		entries,
		has_next_page,
	}
}

impl Home for ReadComicOnline {
	fn get_home(&self) -> Result<HomeLayout> {
		let popular_html = Request::get(&format!("{BASE_URL}/filterList?page=1&sortBy=views&asc=false"))?
			.header("User-Agent", USER_AGENT)
			.header("Referer", &format!("{BASE_URL}/"))
			.html()?;

		let latest_html = Request::get(&format!("{BASE_URL}/filterList?page=1&sortBy=date&asc=false"))?
			.header("User-Agent", USER_AGENT)
			.header("Referer", &format!("{BASE_URL}/"))
			.html()?;

		let mut components = Vec::new();

		let popular = parse_comic_list(popular_html);
		if !popular.entries.is_empty() {
			components.push(HomeComponent {
				title: Some("Most Popular".into()),
				value: HomeComponentValue::Scroller {
					entries: popular.entries.into_iter().map(Into::into).collect(),
					listing: None,
				},
				..Default::default()
			});
		}

		let latest = parse_comic_list(latest_html);
		if !latest.entries.is_empty() {
			components.push(HomeComponent {
				title: Some("Latest Update".into()),
				value: HomeComponentValue::Scroller {
					entries: latest.entries.into_iter().map(Into::into).collect(),
					listing: None,
				},
				..Default::default()
			});
		}

		Ok(HomeLayout { components })
	}
}

impl DeepLinkHandler for ReadComicOnline {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		if !path.starts_with("/comic/") {
			return Ok(None);
		}

		let mut segments = path.split('/').filter(|s| !s.is_empty());
		let _comic = segments.next(); // "comic"
		let slug = segments.next();

		if let Some(slug) = slug {
			Ok(Some(DeepLinkResult::Manga {
				key: format!("/comic/{slug}"),
			}))
		} else {
			Ok(None)
		}
	}
}

register_source!(ReadComicOnline, ListingProvider, Home, DeepLinkHandler);
