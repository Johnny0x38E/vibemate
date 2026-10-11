# Models tab UX (P12 revision)

Current interaction spec for the provider **Models** tab. It supersedes the c.1
behavior that merged the full upstream catalog into SQLite, exposed an
**All / Selected** filter, and saved each checkbox immediately. Shared page
layout follows [secondary-page-layout.md](secondary-page-layout.md); sidebar
activation follows [sidebar-module-home.md](sidebar-module-home.md).

## Persistence and terminology

- Only checked models are persisted in `provider_model`, including manual rows.
- **Uncheck + Save** on the added list removes the row from local storage, not
  merely `selected = 0` on a full catalog snapshot.
- Upstream browsing does not persist the fetched catalog. **Save** persists
  checkbox choices; **Add and save** separately persists a manual model.
- Lazy loading applies only to upstream browse. The added list loads all
  persisted rows, using cursor pages internally.

**Added** / 「已添加」 names saved membership. Checkbox actions and unsaved drafts
still use selection terminology. Internal `selected` fields, command parameters
and translation keys remain unchanged.

## Two views

| View         | Enter                       | List source                                                                                 | Scroll / load                            | Save                                             |
| ------------ | --------------------------- | ------------------------------------------------------------------------------------------- | ---------------------------------------- | ------------------------------------------------ |
| **Added**    | Default when opening Models | `list_provider_models` with `filter: "selected"`; all persisted rows, optional search query | Independent list scroll; full local load | Applies draft deselections by deleting rows      |
| **Upstream** | **Fetch models**            | `browse_upstream_models_page` through provider catalog adapters                             | Scroll triggers the next result page     | Persists checked models and returns to **Added** |

The fixed list header is **Added** / 「已添加」 or **All** / 「全部」, using the
existing `providers.models.filter.selected` and `filter.all` keys. Manual
addition is the first row below the header in both views.

### Toolbar and footer

- **Search** is a narrow field on the left and applies to the active view.
- The added view offers **Fetch models** beside search.
- Upstream offers **Cancel** while downloading and **View added models** /
  「查看已添加」.
- The primary checkbox **Save** is in the fixed bottom footer, opposite the
  page-level **Back** action. It is not in the search toolbar.

There is no **All / Selected** toolbar filter toggle. The toolbar and list header
remain fixed while the model list scrolls independently.

## Draft checkboxes

Checkbox changes update draft state only until **Save**. In the added view,
Save submits deselections in one atomic batch to `save_provider_model_selections`.
In upstream, Save submits new/changed selections, then reloads the added list.
An upstream model that is already persisted starts checked. Unchecking a manual
model and saving deletes it through the same batch command.

## Search

Search targets the active view:

- **Added:** fuzzy search over persisted rows through `list_provider_models`
  with `query`; the summary uses `totalMatches` when Rust supplies it.
- **Upstream:** fuzzy search over model ID and upstream name through
  `browse_upstream_models_page` with `query`.

Rust keeps one in-memory catalog per browse session in `ModelFetchRegistry`.
OpenRouter grows page by page; single-page vendors load once. UI offsets refer
to result rows or ranked search rows, never raw API offsets. Debouncing and the
200-character query limit apply to both views.

## Upstream list footer

One centered status line covers result pagination:

- More pages and idle: scroll hint.
- More pages and downloading: loading-more hint.
- No more pages: all-loaded hint.

Initial upstream loading uses the in-list message while there are no rows.
Appending pages does not add a banner above the list.

## Unsaved changes and navigation

Dirty checkboxes require a bilingual discard confirmation when switching to API,
using page **Back**, re-clicking the active Providers sidebar menu, or changing
between added and upstream views. Busy form/model operations separately block
page Back and home navigation.

Sidebar navigation to another module hides Providers; it is not a global
leave-page confirmation. Hidden model views invalidate pending reads and cancel
upstream fetching. Returning through the sidebar requests the Providers list,
not the previous editor. See [sidebar-module-home.md](sidebar-module-home.md).
Cancelling a fetch never applies draft checkbox changes.

## Manual models (P12.c.3)

Both views include a native `<details>` disclosure named **Add a model manually**.
Its left-side plus/minus icon is plain, without a checkbox-like box or selected
fill. Expanded model ID, optional alias and **Add and save** stay inline.
Accessible labels remain; visible field headings and explanatory prose do not.
Errors and recovery feedback appear below the inputs.

**Add and save** immediately persists and selects one manual row through
`add_manual_provider_model`, independently of checkbox Save. Addition is blocked
while checkbox drafts or other model operations are pending. Capability metadata
remains unverified; an alias is only a display label. Successful creation clears
the inputs and updates the saved baseline, list and count without a provider
network request.

An unknown result blocks another submission until **Check saved models** reads
saved cursor pages and compares the exact ID, including IDs longer than the
search limit. Similar IDs do not confirm a save. A failed check remains retryable.
Pending manual writes block fetching, checkbox saves and page Back. Hiding the
model view unmounts the form; late replies are ignored and subsequent saved-list
reads reconcile persisted state.

Automated tests cover both locales, upstream manual addition and returning to the
added list, field errors, draft blocking, exact-ID recovery, deletion and late
replies. Real Tauri add/restart/delete and visual review remain pending in
[todo.md](todo.md); DOM tests do not establish native persistence.

## Rust surface

- `browse_upstream_models_page`: lazy browse and optional query; session cache
  in `ModelFetchRegistry`; no catalog merge into SQLite.
- `save_provider_model_selections`: atomic checkbox save/removal.
- `add_manual_provider_model`: immediate manual-row save.
- `fetch_provider_models`: full merge path retained for tests and legacy callers;
  the Models tab uses browse and save instead.

## Related docs

- [frontend.md](../frontend.md): Providers shell, controls and checks.
- [todo.md](todo.md): P12 task checklist and native acceptance.
- [architecture.md](../architecture.md): persistence and command overview.
