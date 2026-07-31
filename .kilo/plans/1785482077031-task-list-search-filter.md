# Task List Search Filter

Add a query input box to the **left** of the task-list toolbar that filters the task list by name/URL as the user types.

## Context

- Toolbar in `src/ui/task_list.rs:106-146` is `row![Space(Fill), new_btn, actions…]` — everything is right-aligned. The search box goes at the far left, before the `Space`.
- Existing filter pipeline lives in `src/app.rs:1298-1319`: `task_order` → filter by `TaskFilter` (status) → `sort_tasks` → `task_list::view`. Search filtering follows the same place (status filter + sort already happen in app.rs, so search filtering belongs there too).
- `text_input` pattern to copy: `src/ui/settings_page.rs:719-730` (`theme::style::input::standard`). Existing theme styles are in `src/ui/theme.rs:580`.
- Empty-state message logic is in `task_list.rs:148-172` — must distinguish "no tasks at all" from "no matches for query".

## Changes

### 1. `src/message.rs`
Add to `Message` enum (near `SortOrder` messages, ~line 74):
```rust
SearchChanged(String),
```

### 2. `src/app.rs`
- Add field `search_query: String` to `Remotrix` struct (next to `sort_field`/`sort_order`, ~line 47).
- Init `search_query: String::new()` in `init()` (near `sort_order`, ~line 138).
- Add update arm:
```rust
Message::SearchChanged(query) => {
    state.search_query = query;
}
```
- In the `Page::Tasks` view branch (~line 1298-1311), extend the filter chain:
  - Lowercase the query once: `let query = state.search_query.trim().to_lowercase();`
  - In the `.filter(...)` closure after the status check, add a match helper (case-insensitive substring on `t.name` and `t.url`):
    ```rust
    fn matches_search(t: &DownloadTask, q: &str) -> bool {
        q.is_empty()
            || t.name.to_lowercase().contains(q)
            || t.url.to_lowercase().contains(q)
    }
    ```
    (place as a module-level fn in `app.rs`, or inline closure; either is fine)
  - Pass `&state.search_query` to `task_list::view`.

### 3. `src/ui/task_list.rs`
- Change `view()` signature to accept `search_query: &str` (add after `sort_menu_open`).
- Build the input (left of the `Space` in the toolbar):
  ```rust
  let search_input = iced::widget::text_input(&fluent.get(Tr::Search), search_query)
      .on_input(Message::SearchChanged)
      .width(Length::Fixed(220.0))
      .padding([6, 10])
      .size(13)
      .style(theme::style::input::standard);
  ```
- Optional but recommended: when `!search_query.is_empty()`, append a clear button (existing `icon::x()`, codepoint `\u{E1B2}`, already generated) that sends `Message::SearchChanged(String::new())`; style like `toolbar_icon(false)`.
- Toolbar becomes:
  ```rust
  row![]
      .push(search_input)              // + clear btn if non-empty
      .push(iced::widget::Space::new().width(Length::Fill))
      .push(new_btn)
      .push(actions_row)
      ...
  ```
- Empty state (`tasks.is_empty()`, line 148): if `!search_query.is_empty()`, render a "no results" hint (use new `Tr::NoResults`); otherwise keep existing `Tr::NoTasks`/`Tr::NoTasksHint`. Note this is safe because filtering already happened in app.rs, so an empty list + non-empty query means "no matches".

### 4. i18n
- `src/i18n.rs`: add `Tr::Search` and `Tr::NoResults` to the enum; map to keys `"search"` and `"no-results"` in `key()`.
- `i18n/locales/en/main.ftl`:
  ```
  search = Search…
  no-results = No matching tasks
  ```
- `i18n/locales/zh-CN/main.ftl`:
  ```
  search = 搜索…
  no-results = 没有匹配的任务
  ```

## Out of Scope
- Debounce/throttle typing (filtering is in-memory, list sizes are small).
- Matching on save_dir or GID (name + URL covers visible identity).
- Persisting the query across app restarts.

## Validation
```bash
cargo clippy --workspace   # no warnings
cargo fmt --check
cargo build                # offline, aria2 fetched at runtime
```
Manual: type a substring of a task name → list filters live; case-insensitive; URL matches too; clearing/emptying query restores full list; no-match query shows the no-results hint; query survives switching the category (All/Downloading/Completed) filter.
