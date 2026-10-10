//! Parse vendors' model-list responses into one common shape (P12).
//!
//! Every function here is pure: it takes response bytes (from
//! `crate::http_client`) and returns plain data. Nothing here sends a request,
//! touches the database, or sees the API key, so parsing behaves the same whether
//! or not a key was sent.
//!
//! The response shapes come only from the evidence in
//! `docs/integrations/providers.md` ("模型列表获取（P12）"):
//!
//! - Command Code: `{ "object": "list", "data": [ { "id", "name", "context_length",
//!   "supported_endpoints", ... } ] }`, one request, no pagination. The shape was
//!   recorded from a real response (decision D16); the captured response is the
//!   test fixture `tests/fixtures/command-code-models-2026-10-10.json`.
//! - DeepSeek: `{ "object": "list", "data": [ { "id", "name", "context_window",
//!   "max_output_tokens", "input_modalities", "output_modalities", ... } ] }`.
//! - OpenRouter: `{ "data": [ { "id", "name", "context_length", "architecture",
//!   "top_provider", ... } ], "total_count", "links": { "next" } }`, paged with
//!   `offset` and `limit`.
//!
//! Parsing is deliberately forgiving about single entries and strict about the
//! envelope: an entry with a bad ID is skipped and counted, an optional field with
//! the wrong type becomes `None`, but a body without the documented envelope is
//! rejected as a whole, because then nothing in it can be trusted.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::{Map, Value};
use url::Url;

use crate::providers::{ProviderKind, ProviderProtocol, is_hidden_format_character};

/// Longest accepted model ID, in Unicode characters.
pub const MAX_MODEL_ID_CHARS: usize = 256;

/// Longest kept display name, in Unicode characters; longer names are dropped.
pub const MAX_UPSTREAM_NAME_CHARS: usize = 256;

/// Most models kept for one provider. Further models make the fetch incomplete.
pub const MAX_MODELS_PER_PROVIDER: usize = 5000;

/// OpenRouter page size (`limit`). The documented default is 500, maximum 1000.
pub const OPENROUTER_PAGE_LIMIT: usize = 500;

/// Most OpenRouter pages requested in one fetch: 10 × 500 = 5000 models.
pub const OPENROUTER_MAX_PAGES: usize = 10;

/// Most items kept from a short string list such as modalities or endpoints.
const MAX_LIST_ITEMS: usize = 16;

/// Longest kept item of such a list, in bytes (the items are short ASCII words).
const MAX_LIST_ITEM_LEN: usize = 64;

/// The response does not have the documented envelope.
///
/// The variant has no payload, so the error can never echo text from the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogError {
    /// The response does not have the documented envelope.
    UpstreamResponseInvalid,
    /// The stored base URL cannot be turned into a request URL. Saved base URLs are
    /// validated, so this means the stored row is not one this build accepts.
    InvalidStoredProvider,
}

impl CatalogError {
    /// The stable IPC error code.
    pub fn code(self) -> &'static str {
        match self {
            Self::UpstreamResponseInvalid => "upstream_response_invalid",
            Self::InvalidStoredProvider => "invalid_stored_provider",
        }
    }
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for CatalogError {}

/// One model from a vendor list, already validated and normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogModel {
    /// The vendor's model ID, unchanged. It may contain `/`, `.` or `:`.
    pub model_id: String,
    /// The vendor's display name, trimmed, when it is valid.
    pub upstream_name: Option<String>,
    /// Total context window in tokens.
    pub context_window: Option<i64>,
    /// Largest output in tokens.
    pub max_output_tokens: Option<i64>,
    /// Accepted input types, such as `text` or `image`.
    pub input_modalities: Option<Vec<String>>,
    /// Produced output types, such as `text`.
    pub output_modalities: Option<Vec<String>>,
    /// Command Code routes serving the model, normalized without `/v1`
    /// (for example `/chat/completions`). `None` for other vendors or when absent.
    pub supported_endpoints: Option<Vec<String>>,
}

/// One parsed response (one page for OpenRouter).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogPage {
    /// Valid models in response order. Duplicates are removed later by `CatalogCollector`.
    pub models: Vec<CatalogModel>,
    /// Number of entries in the response's `data` array, valid or not.
    pub entries: usize,
    /// Entries skipped because their ID (or the entry itself) was invalid.
    pub skipped_invalid: usize,
    /// OpenRouter only: whether `links.next` says another page exists.
    pub has_more: bool,
    /// OpenRouter only: `total_count`, the number of models matching the query.
    pub total_count: Option<u64>,
}

/// Whether a model can be used with the instance's protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteSupport {
    /// The model lists the route of the instance's protocol.
    Supported,
    /// The model lists routes, but not the one the instance uses.
    Unsupported,
    /// The model does not say which routes serve it.
    Unknown,
    /// The vendor does not publish routes per model (DeepSeek, OpenRouter).
    NotApplicable,
}

impl RouteSupport {
    /// The value used on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
            Self::Unknown => "unknown",
            Self::NotApplicable => "not_applicable",
        }
    }
}

/// Why a fetch did not see the vendor's whole list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IncompleteReason {
    /// More than `MAX_MODELS_PER_PROVIDER` models.
    ModelLimit,
    /// OpenRouter still had pages after `OPENROUTER_MAX_PAGES`.
    PageLimit,
    /// The list was empty, which is treated as suspicious rather than as
    /// "every model was removed".
    EmptyList,
}

/// Check a model ID from a vendor or from the user.
///
/// The ID must already be trimmed, be 1 to 256 characters, and contain no
/// whitespace, control characters, or invisible format characters. Everything
/// else is allowed, including `/` (`mistral/mistral-large-4`), `.` and `:`.
pub fn is_valid_model_id(model_id: &str) -> bool {
    !model_id.is_empty()
        && model_id.chars().count() <= MAX_MODEL_ID_CHARS
        && !model_id
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || is_hidden_format_character(c))
}

/// Remove an optional `/v1` prefix from a Command Code route.
///
/// The documentation writes routes as `/v1/chat/completions`, while the live
/// list returns `/chat/completions`; both normalize to the second form.
pub fn normalize_endpoint(endpoint: &str) -> &str {
    match endpoint.strip_prefix("/v1") {
        Some(rest) if rest.starts_with('/') => rest,
        _ => endpoint,
    }
}

/// The normalized route that serves a protocol.
pub fn protocol_endpoint(protocol: ProviderProtocol) -> &'static str {
    match protocol {
        ProviderProtocol::ChatCompletions => "/chat/completions",
        ProviderProtocol::Responses => "/responses",
        ProviderProtocol::AnthropicMessages => "/messages",
    }
}

/// Decide whether a fetched model can be selected for an instance.
///
/// `supported_endpoints` must be the normalized list from `CatalogModel`.
pub fn route_support(
    kind: ProviderKind,
    protocol: ProviderProtocol,
    supported_endpoints: Option<&[String]>,
) -> RouteSupport {
    if kind != ProviderKind::CommandCode {
        return RouteSupport::NotApplicable;
    }
    match supported_endpoints {
        None => RouteSupport::Unknown,
        Some(endpoints) => {
            let wanted = protocol_endpoint(protocol);
            if endpoints.iter().any(|endpoint| endpoint == wanted) {
                RouteSupport::Supported
            } else {
                RouteSupport::Unsupported
            }
        }
    }
}

/// The model-list URL: the stored base URL plus `/models`.
///
/// The base URL is already validated by `crate::providers`; any trailing `/` is
/// removed first so `https://a.com/v1/` and `https://a.com/v1` both give
/// `https://a.com/v1/models`.
pub fn models_url(base_url: &str) -> Result<Url, CatalogError> {
    let trimmed = base_url.trim_end_matches('/');
    Url::parse(&format!("{trimmed}/models")).map_err(|_| CatalogError::InvalidStoredProvider)
}

/// The URL of one OpenRouter page. Query values are set through the `url` crate,
/// never by joining strings.
pub fn openrouter_page_url(base_url: &str, offset: usize) -> Result<Url, CatalogError> {
    openrouter_page_url_with_limit(base_url, offset, OPENROUTER_PAGE_LIMIT)
}

/// OpenRouter page URL with an explicit page size (upstream browse uses a smaller limit).
pub fn openrouter_page_url_with_limit(
    base_url: &str,
    offset: usize,
    limit: usize,
) -> Result<Url, CatalogError> {
    let mut url = models_url(base_url)?;
    url.query_pairs_mut()
        .append_pair("offset", &offset.to_string())
        .append_pair("limit", &limit.to_string());
    Ok(url)
}

/// Parse one response from the given vendor.
pub fn parse_page(kind: ProviderKind, body: &[u8]) -> Result<CatalogPage, CatalogError> {
    match kind {
        ProviderKind::CommandCode => parse_command_code(body),
        ProviderKind::DeepSeek => parse_deepseek(body),
        ProviderKind::OpenRouter => parse_openrouter_page(body),
    }
}

/// Parse Command Code's list: `data[].id/name/context_length/supported_endpoints`.
pub fn parse_command_code(body: &[u8]) -> Result<CatalogPage, CatalogError> {
    let envelope = parse_envelope(body)?;
    Ok(parse_entries(data_array(&envelope)?, |entry, model| {
        model.context_window = positive_integer(entry.get("context_length"));
        model.supported_endpoints = string_list(entry.get("supported_endpoints")).map(|list| {
            let mut normalized: Vec<String> = Vec::new();
            for endpoint in list {
                let endpoint = normalize_endpoint(&endpoint).to_owned();
                if !normalized.contains(&endpoint) {
                    normalized.push(endpoint);
                }
            }
            normalized
        });
    }))
}

/// Parse DeepSeek's list: `data[].id/name/context_window/max_output_tokens/
/// input_modalities/output_modalities`.
pub fn parse_deepseek(body: &[u8]) -> Result<CatalogPage, CatalogError> {
    let envelope = parse_envelope(body)?;
    Ok(parse_entries(data_array(&envelope)?, |entry, model| {
        model.context_window = positive_integer(entry.get("context_window"));
        model.max_output_tokens = positive_integer(entry.get("max_output_tokens"));
        model.input_modalities = string_list(entry.get("input_modalities"));
        model.output_modalities = string_list(entry.get("output_modalities"));
    }))
}

/// Parse one OpenRouter page: `data[].id/name/context_length`,
/// `architecture.input_modalities/output_modalities`,
/// `top_provider.max_completion_tokens`, plus the required `total_count` and
/// `links.next`.
pub fn parse_openrouter_page(body: &[u8]) -> Result<CatalogPage, CatalogError> {
    let envelope = parse_envelope(body)?;
    let total_count = envelope
        .get("total_count")
        .and_then(Value::as_u64)
        .ok_or(CatalogError::UpstreamResponseInvalid)?;
    // `links.next` is required: a string when another page exists, else null.
    // Only its presence is used; the URL itself is never requested.
    let has_more = match envelope
        .get("links")
        .and_then(Value::as_object)
        .and_then(|links| links.get("next"))
    {
        Some(Value::String(_)) => true,
        Some(Value::Null) => false,
        _ => return Err(CatalogError::UpstreamResponseInvalid),
    };
    let mut page = parse_entries(data_array(&envelope)?, |entry, model| {
        model.context_window = positive_integer(entry.get("context_length"));
        let architecture = entry.get("architecture").and_then(Value::as_object);
        model.input_modalities =
            string_list(architecture.and_then(|object| object.get("input_modalities")));
        model.output_modalities =
            string_list(architecture.and_then(|object| object.get("output_modalities")));
        model.max_output_tokens = positive_integer(
            entry
                .get("top_provider")
                .and_then(Value::as_object)
                .and_then(|object| object.get("max_completion_tokens")),
        );
    });
    page.has_more = has_more;
    page.total_count = Some(total_count);
    Ok(page)
}

/// Collects the pages of one fetch: removes duplicate IDs (the first one wins),
/// applies the per-provider cap, and works out whether the fetch is complete.
#[derive(Debug, Default)]
pub struct CatalogCollector {
    models: Vec<CatalogModel>,
    seen: HashSet<String>,
    skipped_invalid: usize,
    duplicates: usize,
    over_model_limit: bool,
}

/// The result of a whole fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    /// Unique valid models, at most `MAX_MODELS_PER_PROVIDER`, in response order.
    pub models: Vec<CatalogModel>,
    /// Entries skipped because they were invalid.
    pub skipped_invalid: usize,
    /// Entries skipped because an earlier entry had the same ID.
    pub duplicates: usize,
    /// `None` when the whole list was seen; otherwise why not.
    pub incomplete: Option<IncompleteReason>,
}

impl CatalogCollector {
    /// Start an empty collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one parsed page.
    pub fn add_page(&mut self, page: CatalogPage) {
        self.skipped_invalid += page.skipped_invalid;
        if page
            .total_count
            .is_some_and(|total| total > MAX_MODELS_PER_PROVIDER as u64)
        {
            self.over_model_limit = true;
        }
        for model in page.models {
            if self.seen.contains(&model.model_id) {
                self.duplicates += 1;
            } else if self.models.len() >= MAX_MODELS_PER_PROVIDER {
                self.over_model_limit = true;
            } else {
                self.seen.insert(model.model_id.clone());
                self.models.push(model);
            }
        }
    }

    /// Finish the collection. `page_limit_reached` is true when OpenRouter still
    /// reported more pages after the last allowed one.
    pub fn finish(self, page_limit_reached: bool) -> Catalog {
        let incomplete = if self.over_model_limit {
            Some(IncompleteReason::ModelLimit)
        } else if page_limit_reached {
            Some(IncompleteReason::PageLimit)
        } else if self.models.is_empty() {
            Some(IncompleteReason::EmptyList)
        } else {
            None
        };
        Catalog {
            models: self.models,
            skipped_invalid: self.skipped_invalid,
            duplicates: self.duplicates,
            incomplete,
        }
    }
}

/// What to do after an OpenRouter page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageStep {
    /// Request the page at this offset next.
    Fetch { offset: usize },
    /// The list is finished.
    Done,
    /// More pages exist, but the page cap is reached.
    PageLimitReached,
}

/// Plans OpenRouter paging. vibemate computes each offset itself and never
/// follows the `links.next` URL from the server.
#[derive(Debug, Default)]
pub struct OpenRouterPager {
    next_offset: usize,
    pages: usize,
}

impl OpenRouterPager {
    /// Start at offset 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// The offset of the next page to request.
    pub fn next_offset(&self) -> usize {
        self.next_offset
    }

    /// Record a parsed page and decide the next step.
    pub fn record(&mut self, page: &CatalogPage) -> PageStep {
        self.pages += 1;
        // An empty page also ends the list, so a server that keeps saying "more"
        // without sending entries cannot keep us looping.
        if !page.has_more || page.entries == 0 {
            return PageStep::Done;
        }
        self.next_offset += page.entries;
        if self.pages >= OPENROUTER_MAX_PAGES {
            PageStep::PageLimitReached
        } else {
            PageStep::Fetch {
                offset: self.next_offset,
            }
        }
    }
}

/// Parse the body as a JSON object.
fn parse_envelope(body: &[u8]) -> Result<Map<String, Value>, CatalogError> {
    match serde_json::from_slice(body) {
        Ok(Value::Object(object)) => Ok(object),
        _ => Err(CatalogError::UpstreamResponseInvalid),
    }
}

/// The required `data` array.
fn data_array(envelope: &Map<String, Value>) -> Result<&Vec<Value>, CatalogError> {
    envelope
        .get("data")
        .and_then(Value::as_array)
        .ok_or(CatalogError::UpstreamResponseInvalid)
}

/// Turn `data` entries into models. Common fields (`id`, `name`) are read here;
/// `fill` reads the vendor-specific fields.
fn parse_entries(
    entries: &[Value],
    fill: impl Fn(&Map<String, Value>, &mut CatalogModel),
) -> CatalogPage {
    let mut models = Vec::new();
    let mut skipped_invalid = 0;
    for entry in entries {
        let Some(entry) = entry.as_object() else {
            skipped_invalid += 1;
            continue;
        };
        let Some(model_id) = entry
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| is_valid_model_id(id))
        else {
            skipped_invalid += 1;
            continue;
        };
        let mut model = CatalogModel {
            model_id: model_id.to_owned(),
            upstream_name: display_name(entry.get("name")),
            context_window: None,
            max_output_tokens: None,
            input_modalities: None,
            output_modalities: None,
            supported_endpoints: None,
        };
        fill(entry, &mut model);
        models.push(model);
    }
    CatalogPage {
        models,
        entries: entries.len(),
        skipped_invalid,
        has_more: false,
        total_count: None,
    }
}

/// A trimmed display name, or `None` when it is missing, empty, too long, or
/// contains control or invisible format characters.
fn display_name(value: Option<&Value>) -> Option<String> {
    let name = value?.as_str()?.trim();
    let valid = !name.is_empty()
        && name.chars().count() <= MAX_UPSTREAM_NAME_CHARS
        && !name
            .chars()
            .any(|c| c.is_control() || is_hidden_format_character(c));
    valid.then(|| name.to_owned())
}

/// A positive integer that fits SQLite's `INTEGER`, or `None`.
fn positive_integer(value: Option<&Value>) -> Option<i64> {
    value?
        .as_u64()
        .filter(|number| *number > 0)
        .and_then(|number| i64::try_from(number).ok())
}

/// A short list of short, printable ASCII words without spaces. Invalid items and
/// repeats are dropped and at most `MAX_LIST_ITEMS` are kept; a value that is not
/// an array becomes `None`.
fn string_list(value: Option<&Value>) -> Option<Vec<String>> {
    let items = value?.as_array()?;
    let mut list: Vec<String> = Vec::new();
    for item in items {
        let Some(text) = item.as_str() else {
            continue;
        };
        let valid = !text.is_empty()
            && text.len() <= MAX_LIST_ITEM_LEN
            && text.bytes().all(|byte| byte.is_ascii_graphic());
        if valid && !list.iter().any(|kept| kept == text) {
            list.push(text.to_owned());
            if list.len() == MAX_LIST_ITEMS {
                break;
            }
        }
    }
    Some(list)
}

#[cfg(test)]
mod tests;
