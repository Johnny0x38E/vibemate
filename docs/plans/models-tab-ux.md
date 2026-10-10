# Models tab UX (P12 revision)

Authoritative interaction spec for the provider **Models** tab. It supersedes
the c.1 behavior that merged the full upstream catalog into SQLite, exposed an
**All / Selected** filter, and saved each checkbox immediately.

## Persistence rule

- **Only checked models are persisted** in `provider_model` (plus manual rows
  when P12.c.3 ships).
- **Uncheck + Save** on the selected list **removes** the row from local storage
  (not merely `selected = 0` on a full catalog snapshot).
- **Upstream browse** does not write to SQLite until the user clicks **Save** on
  that view. Lazy loading applies **only** while browsing upstream during
  **Fetch models**; the selected list loads all persisted rows at once.

## Two views

| View         | Enter                               | List source                                                                                              | Scroll / load                        | Save                                                                    |
| ------------ | ----------------------------------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------ | ----------------------------------------------------------------------- |
| **Selected** | Default when opening the Models tab | `list_provider_models` with `filter: "selected"`, no query; load all rows (internal Rust page size only) | Single scroll region in the tab      | Applies draft check changes: removals delete rows; no add-without-fetch |
| **Upstream** | User clicks **Fetch models**        | `browse_upstream_models_page` (provider `GET …/models` via existing catalog adapters)                    | Scroll triggers the next result page | Persists checked models, then returns to **Selected**                   |

List header (fixed above the scroll region): **Selected** / 「已选」 in the
selected view, **All** / 「全部」 in upstream browse (labels
`providers.models.filter.selected` / `filter.all`).

### Toolbar

- **Search** (narrow field, left): debounced; applies to the active view.
- **Selected view:** **Fetch models** (secondary), **Save** (primary, right).
- **Upstream view:** **Cancel** while a browse download runs (secondary),
  **Back to selected** / 「切换到已选」 (secondary), **Save** (primary).

There is no **All / Selected** filter toggle on the toolbar.

## Draft checkboxes and Save

- Checkbox changes update **draft state** only until **Save**.
- **Selected view:** Save submits deselections (and any other allowed edits) in
  one batch to Rust (`save_provider_model_selections`).
- **Upstream view:** Save submits new/changed selections in one batch, then
  switches to **Selected** and reloads the persisted list.

## Upstream list and existing selections

When upstream browse shows a model ID that is already persisted as selected, the
checkbox must appear **checked** automatically so behavior matches the selected
list. Draft edits remain unsaved until **Save**.

## Search

Search always targets the **active view**:

- **Selected:** fuzzy search over persisted selected rows (`list_provider_models`
    - `query`); summary shows total matches when Rust returns `totalMatches`.
- **Upstream:** fuzzy search over model ID and upstream name via
  `browse_upstream_models_page` + `query` (same rules as the selected list).
  Rust keeps one in-memory catalog per browse session in `ModelFetchRegistry`
  (OpenRouter grows page by page; single-page vendors load once). UI `offset`
  values always refer to result rows or ranked search rows, not raw API offsets.

Debounced search, `searchSummary` / `searchSummaryUpstream`, and the 200-character
limit match the selected-list rules.

## List footer (upstream)

One centered status line under the list (not a second banner while loading):

- More result pages and idle → scroll hint.
- More result pages and download in progress → loading-more hint.
- No more pages → all loaded hint.

Initial upstream load uses the in-list loading message only while the first page
has no rows yet.

## Unsaved changes

If the user has **dirty** draft checkboxes and tries to leave the Models tab,
switch to the API tab, go **Back** to the provider list, or hide the provider
page, show a blocking **unsaved changes** prompt (bilingual). **Back** on the
provider page remains coordinated with `formBusy` / `modelsBusy`; dirty drafts
add a separate guard.

Fetching upstream may still show **Cancel** for an in-flight download; cancelling
does not apply draft checks.

## Flow summary

1. Open Models tab → **Selected** list, full load, empty state guides user to
   **Fetch models** when nothing is persisted.
2. **Fetch models** → **Upstream** browse, lazy pages, checkboxes (pre-check
   persisted IDs) → **Save** → persist → **Selected** list.
3. On **Selected**, uncheck → **Save** → rows removed. To add again, repeat
   step 2.

## Rust surface (implemented)

- `browse_upstream_models_page` — lazy browse + optional `query`; session cache
  in `ModelFetchRegistry`; does not merge into SQLite.
- `save_provider_model_selections` — atomic save/removal from upstream or
  selected drafts.
- `fetch_provider_models` — full merge path remains for tests and legacy callers;
  the Models tab hot path uses browse + save only.

**Remaining (P12.c.3):** manual model add (`ManualModelForm`: model ID, optional
alias, field errors, delete manual row). Rust commands `add_manual_provider_model`
and `delete_manual_provider_model` already exist; wire into the selected view and
the same draft/Save model where appropriate. Track in [todo.md](todo.md).

## Related docs

- [frontend.md](../frontend.md) — Providers shell and tab layout
- [todo.md](todo.md) — P12 task checklist
- [architecture.md](../architecture.md) — persistence and command overview
