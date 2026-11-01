# Forgeline UI/UX design specification

**Audience:** implementing agents (e.g. Google Antigravity).  
**Scope:** `apps/desktop` React (Vite, TypeScript) UI only. Docs / CSS / frontend layout.  
**Out of scope:** Rust crates (`forgeline-core`, `forgeline-graph`, CLI, Roslyn helper), exact-only graph engine behavior, SQLite schema. Tauri commands stay as-is unless a change is strictly required for window chrome (title bar / vibrancy).

UI copy examples below are **Vietnamese**. Spec prose, component names, CSS tokens, and file names are **English**.

---

## 0. Current UI inventory (baseline to replace)

As of this spec, `apps/desktop/src` is a two-column prototype:

| Area | Current behavior | Replace with |
| --- | --- | --- |
| Sidebar | Brand text, inline “Tạo dự án” form, flat project buttons, security footnote | Native-feel NavigationSplitView sidebar, toolbar actions, empty state, context menus |
| Project header | H1 + inline rename/delete form | Title in toolbar / content header; rename via double-click or context menu; delete via sheet |
| Stats | Four bordered cards + monospace `stats.summary` dump | Readable stat tiles + small breakdown; no monospace wall of text |
| Sources | Card-like rows with “Sơ đồ” / “Phân tích”, inline add-source form | Table in **Nguồn** tab; **Thêm nguồn** sheet; graph in **Sơ đồ** tab |
| Graph | Embedded panel with search, kind filter, sigma.js canvas, side detail | First-class tab: toolbar, legend, inspector, zoom/fit, dark-aware colors |
| Theme | Hard-coded dark warm palette, Segoe UI | System light/dark, SF Pro / SF Mono stacks, Apple semantic tokens |
| Window | Standard Tauri title bar (`minWidth` 800, `minHeight` 560) | Overlay title bar + traffic lights; vibrancy sidebar |

**Existing Tauri commands (keep):** `list_projects`, `create_project`, `rename_project`, `delete_project`, `list_sources`, `add_source`, `index_source`, `project_stats`, `source_overview`, `export_source_overview`, `symbolNeighbors` → `symbol_neighbors`.

**Status values on sources (keep labels):** `pending` → Chờ, `indexing` → Đang phân tích, `ready` → Sẵn sàng, `error` → Lỗi.

---

## 1. Principles

### Clarity
Every screen answers one question. Labels are short Vietnamese sentence case. Prefer one primary action per region. Numbers use clear units (tệp, ký hiệu, liên kết chính xác).

### Deference
Chrome is quiet; content (project name, sources, graph) is loud. Prefer translucent materials and hairline separators over heavy borders and multi-layer shadows. No decorative gradients as the main visual idea.

### Depth
Use materials (sidebar vibrancy), soft elevation for sheets/popovers, and selection fills—not card stacks. Z-order: content → floating toolbar controls → popovers → sheets → alerts.

### Content-first
No dashboard clutter in the first view. Hero of the app is the selected project’s sources and graph, not marketing chrome. Brand appears once in the sidebar header (small wordmark), never competing with project titles.

### Native feel (macOS HIG)
Match Xcode / Finder / Notes / System Settings patterns: NavigationSplitView, toolbar segmented control, sheets for modal tasks, context menus, SF-style metrics, system appearance. Pointer cursor defaults; hand only on links.

### No clutter
Ban: pill clusters, stat strips with icons-only, floating badges on the graph, inset rounded “media cards,” emoji, glow accents, purple-on-white themes. One job per section.

### Progressive disclosure
- Project list → select project → tabs (Tổng quan / Sơ đồ / Nguồn).
- Add source lives in a sheet, not an always-visible form row.
- Graph inspector opens only after node selection.
- Advanced export (“Xuất JSON”) sits in a toolbar overflow or secondary button, not next to primary navigation.

### Every state designed
For every data surface, design: **empty**, **loading**, **error**, **partial** (e.g. some sources ready, one indexing), **success**. Never leave a blank white/black void. Error copy is Vietnamese, actionable when possible.

---

## 2. Window & layout

### Window chrome (Tauri 2, macOS-first)

In `apps/desktop/src-tauri/tauri.conf.json` (or window builder):

```json
{
  "title": "Forgeline",
  "width": 1200,
  "height": 800,
  "minWidth": 960,
  "minHeight": 640,
  "resizable": true,
  "titleBarStyle": "Overlay",
  "hiddenTitle": true,
  "trafficLightPosition": { "x": 12, "y": 18 }
}
```

- Traffic lights remain visible and clickable; do not draw custom close/minimize.
- Top toolbar / title region must leave **≥ 78px** left inset so content never sits under traffic lights.
- On non-macOS, fall back to a normal titled window; keep the same three-pane layout without vibrancy.

### Vibrancy / sidebar material

- Prefer `window-vibrancy` (Rust) with `NSVisualEffectMaterial::Sidebar` on the sidebar HWND/NSView region.
- Web CSS fallback when vibrancy unavailable:

```css
.sidebar {
  background: color-mix(in srgb, var(--material-sidebar) 72%, transparent);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
}
```

- Content pane uses opaque `--bg-content` so text contrast stays AA over vibrancy edges.

### Three-pane NavigationSplitView

```text
┌────────────┬──────────────────────────────────────────────┬────────────┐
│ Sidebar    │ Content                                      │ Inspector  │
│ projects   │ toolbar: [ Tổng quan | Sơ đồ | Nguồn ]       │ (optional) │
│ + actions  │ project body for selected tab                │ graph only │
│            │                                              │ or detail  │
└────────────┴──────────────────────────────────────────────┴────────────┘
```

| Pane | Default width | Min | Max | Persist key |
| --- | --- | --- | --- | --- |
| Sidebar | 220px | 180px | 320px | `forgeline.ui.sidebarWidth` |
| Content | flex | 480px | — | — |
| Inspector | 280px | 220px | 400px | `forgeline.ui.inspectorWidth` |

- Persist widths in `localStorage` (or Tauri store if already introduced). Restore on launch.
- Drag handles: 1px separator with 6px hit target; `col-resize` cursor.
- Collapse: double-click separator or View menu (settings later) may collapse inspector; sidebar collapse optional (Cmd+Ctrl+S if implemented).
- When no project selected: content shows first-run empty state; inspector hidden.
- Graph tab shows inspector; Tổng quan / Nguồn hide inspector (or show project-level detail only if useful—default **hide**).

### Toolbar

Fixed under the title-bar safe area:

1. **Leading:** project title (truncated with ellipsis) or “Forgeline” when empty.
2. **Center:** segmented control — `Tổng quan` | `Sơ đồ` | `Nguồn` (Cmd+1 / 2 / 3).
3. **Trailing:** icon buttons — search (Cmd+F), add source (Cmd+Shift+O), more menu.

Segmented control is disabled (visually dimmed) when no project is selected.

### Min size & density

- Minimum window: **960×640**. Comfortable default: **1200×800**.
- Control heights: 22px (compact toolbar), 28px (dialogs / primary forms). Row height in lists: 28–32px. Table row: 36px.

---

## 3. Design tokens (CSS variables)

Define in `apps/desktop/src/styles/tokens.css` (or equivalent). Follow **system appearance** via `prefers-color-scheme` and `color-scheme: light dark` on `:root`. Do **not** ship remote fonts or CDNs.

### Typography

```css
:root {
  --font-ui: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif;
  --font-mono: "SF Mono", ui-monospace, Menlo, Consolas, monospace;

  /* macOS-oriented type scale (px at 1x) */
  --text-large-title: 26px;   /* weight 700 / bold */
  --text-title1: 22px;        /* weight 700 */
  --text-title2: 17px;        /* weight 600 / semibold */
  --text-title3: 15px;        /* weight 600 */
  --text-body: 13px;          /* weight 400; line-height 1.35 */
  --text-callout: 12px;       /* weight 400 */
  --text-caption1: 11px;      /* weight 400 */
  --text-caption2: 10px;      /* weight 400; rare */
}
```

Use large title only for empty-state headlines and settings section titles. In-app project name in content: **title2** (17 semibold). Body for table cells and forms: **13**.

### Spacing (4 / 8 pt grid)

```css
:root {
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-5: 20px;
  --space-6: 24px;
  --space-8: 32px;
  --space-10: 40px;
}
```

Padding inside panes: 12–16px. Gap between sections: 16–24px.

### Corner radii

```css
:root {
  --radius-sm: 6px;   /* buttons, text fields, badges */
  --radius-md: 8px;   /* segmented control, list selection */
  --radius-lg: 10px;  /* popovers, table containers */
  --radius-xl: 12px;  /* sheets, large panels */
}
```

No `border-radius: 9999px` pills except tiny status dots (4px).

### Semantic colors — light

Approximate Apple Aqua values (sRGB). Prefer semantic names so dark mode swaps cleanly.

```css
:root,
[data-theme="light"] {
  color-scheme: light;

  --label: rgba(0, 0, 0, 0.85);
  --secondary-label: rgba(0, 0, 0, 0.50);
  --tertiary-label: rgba(0, 0, 0, 0.25);
  --quaternary-label: rgba(0, 0, 0, 0.10);
  --placeholder: rgba(0, 0, 0, 0.25);

  --separator: rgba(0, 0, 0, 0.10);
  --grid: rgba(0, 0, 0, 0.10);

  --bg-window: #ececec;
  --bg-content: #ffffff;
  --bg-control: #ffffff;
  --bg-selected: rgba(0, 122, 255, 0.15);
  --bg-selected-unemph: #dcdcdc;
  --bg-hover: rgba(0, 0, 0, 0.05);

  --accent: #007aff;           /* controlAccent / systemBlue */
  --accent-emphasis: #0066d6;
  --on-accent: #ffffff;

  --system-green: #28cd41;
  --system-orange: #ff9500;
  --system-red: #ff3b30;
  --system-yellow: #ffcc00;
  --system-gray: #8e8e93;
  --system-brown: #a2845e;
  --system-purple: #5856d6;
  --system-pink: #ff2d55;
  --link: #0068da;

  --material-sidebar: rgba(246, 246, 246, 0.72);
  --material-header: rgba(255, 255, 255, 0.65);

  --shadow-popover: 0 8px 24px rgba(0, 0, 0, 0.18), 0 0 1px rgba(0, 0, 0, 0.12);
  --shadow-sheet: 0 16px 40px rgba(0, 0, 0, 0.22);
  --focus-ring: 0 0 0 3px rgba(0, 122, 255, 0.45);

  /* Graph node fills (light) */
  --graph-folder: #c4a574;
  --graph-file: #7aaf66;
  --graph-component: #5aa7c0;
  --graph-type: #e09a45;      /* class / record / struct */
  --graph-interface: #9b7bc9;
  --graph-fn: #d06a5c;        /* function / method */
  --graph-edge: #9a8f7e;
  --graph-canvas: #f5f5f7;
  --graph-label: rgba(0, 0, 0, 0.75);
}
```

### Semantic colors — dark

```css
@media (prefers-color-scheme: dark) {
  :root {
    color-scheme: dark;

    --label: rgba(255, 255, 255, 0.85);
    --secondary-label: rgba(255, 255, 255, 0.55);
    --tertiary-label: rgba(255, 255, 255, 0.25);
    --quaternary-label: rgba(255, 255, 255, 0.10);
    --placeholder: rgba(255, 255, 255, 0.50);

    --separator: rgba(255, 255, 255, 0.10);
    --grid: rgba(255, 255, 255, 0.10);

    --bg-window: #1e1e1e;
    --bg-content: #1e1e1e;
    --bg-control: #2a2a2a;
    --bg-selected: rgba(10, 132, 255, 0.28);
    --bg-selected-unemph: #464646;
    --bg-hover: rgba(255, 255, 255, 0.06);

    --accent: #0a84ff;
    --accent-emphasis: #409cff;
    --on-accent: #ffffff;

    --system-green: #32d74b;
    --system-orange: #ff9f0a;
    --system-red: #ff453a;
    --system-yellow: #ffd60a;
    --system-gray: #98989d;
    --system-brown: #ac8e68;
    --system-purple: #6260e0;
    --system-pink: #ff375f;
    --link: #419cff;

    --material-sidebar: rgba(40, 40, 40, 0.72);
    --material-header: rgba(30, 30, 30, 0.72);

    --shadow-popover: 0 8px 28px rgba(0, 0, 0, 0.55), 0 0 1px rgba(0, 0, 0, 0.4);
    --shadow-sheet: 0 16px 48px rgba(0, 0, 0, 0.6);
    --focus-ring: 0 0 0 3px rgba(10, 132, 255, 0.55);

    --graph-folder: #c4a574;
    --graph-file: #9cbe86;
    --graph-component: #6db3c9;
    --graph-type: #e0a45a;
    --graph-interface: #b08fd6;
    --graph-fn: #d4776a;
    --graph-edge: #8a7a62;
    --graph-canvas: #161616;
    --graph-label: rgba(255, 255, 255, 0.8);
  }
}
```

Optional: Settings toggle “Giao diện” = Hệ thống / Sáng / Tối via `data-theme` override. Default = Hệ thống.

### Status mapping

| Source status | Badge fill / text | Token |
| --- | --- | --- |
| `pending` | Gray text | `--system-gray` |
| `indexing` | Orange | `--system-orange` |
| `ready` | Green | `--system-green` |
| `error` | Red | `--system-red` |

### Focus ring

`:focus-visible { outline: none; box-shadow: var(--focus-ring); }` on interactive controls. Never remove focus styles without a replacement.

---

## 4. Components

Implement as React components under `apps/desktop/src/components/`. Prefer Radix UI primitives (`@radix-ui/react-dialog`, `dropdown-menu`, `popover`, `tooltip`, `progress`, `tabs` or custom segmented) for a11y. Icons: `lucide-react` (bundled), sized 14–16px in toolbars, 20–24px in empty states. Map to SF Symbol metaphors (folder, git-branch, search, plus, trash, refresh-cw, network, settings).

### 4.1 Sidebar list rows (`SidebarProjectRow`)

- Height 28–32px; left icon (16px `Folder` or project glyph) + name (body 13, truncate).
- **Default:** transparent background, `--label` text.
- **Hover:** `--bg-hover`.
- **Selected:** `--bg-selected`, accent left bar **optional** (2px) or full-row fill; selected text stays `--label` (not forced white—macOS sidebar selection is translucent accent).
- **Inline rename:** double-click or context “Đổi tên” → row becomes a text field (height 24, radius 6); Enter commits via `rename_project`; Escape cancels; blur commits if changed and non-empty.
- **Context menu (Radix):** `Đổi tên`, separator, `Xóa…` (destructive).
- **Delete confirmation sheet:** title `Xóa dự án “{name}”?`, body `Thao tác này không thể hoàn tác. Nguồn và chỉ mục gắn với dự án sẽ bị xóa khỏi Forgeline.`, buttons `Hủy` (secondary) / `Xóa` (destructive). Call `delete_project` only on confirm.
- Keyboard: ↑/↓ move selection; Enter activate; F2 or Enter on selected can start rename if that matches Notes-like patterns—prefer context menu + double-click for discoverability.

### 4.2 Push buttons (`Button`)

| Variant | Background | Text | Border | Use |
| --- | --- | --- | --- | --- |
| primary | `--accent` | `--on-accent` | none | Confirm, Phân tích, Thêm |
| secondary | `--bg-control` | `--label` | 1px `--separator` | Cancel, secondary |
| destructive | transparent or light red wash | `--system-red` | 1px separator | Xóa |
| toolbar icon | transparent | `--label` | none | Icon-only 28×28 hit |

Heights: **22px** toolbar, **28px** dialogs. Horizontal padding 10–12px. Disabled: opacity 0.4, `cursor: default`, no hover.

### 4.3 Text fields (`TextField`)

- Height 28 (forms) / 22 (toolbar search).
- Radius `--radius-sm`, border 1px `--separator`, bg `--bg-control`.
- Placeholder: `--placeholder`.
- Error: border `--system-red`, caption below in callout 12.
- Autofill name from folder basename when picking local path (preserve current `folderName` behavior).

### 4.4 Segmented control (`SegmentedControl`)

- Height 22–28; equal segments; selected segment white/elevated fill on subtle track (`--bg-hover` track).
- Labels: `Tổng quan`, `Sơ đồ`, `Nguồn`.
- ARIA: `role="tablist"` / `tab`; `aria-selected`.
- Disabled when `selectedProjectId == null`.

### 4.5 Search field (`SearchField`)

- Leading magnifying glass icon; optional clear (×) when non-empty.
- Placeholder examples: `Tìm dự án`, `Tìm nguồn`, `Tìm tên / đường dẫn`.
- Debounce 200ms for graph search (match current `SourceGraphView`).
- Cmd+F focuses the active pane’s search (sidebar filter or graph/sources search).

### 4.6 Popovers

- Radius `--radius-lg`, shadow `--shadow-popover`, padding 8–12.
- Used for: toolbar overflow, graph legend, kind filter if not inline.
- Dismiss on outside click / Escape; focus trap when containing form controls.

### 4.7 Sheets (`Sheet`) — Thêm nguồn

Replace the inline add-source form row entirely.

**Trigger:** toolbar “Thêm nguồn”, sidebar/project empty CTA, Cmd+Shift+O. Requires selected project.

**Layout (centered modal sheet, ~480px wide):**

1. Title: `Thêm nguồn`
2. Segmented or radio: `Thư mục cục bộ` | `URL git`
3. **Local:** read-only path field + `Chọn thư mục…` (existing `@tauri-apps/plugin-dialog` `open({ directory: true })`). On pick, set location and auto-fill name from last path segment if name empty.
4. **Git:** text field placeholder `https://github.com/org/repo.git`
5. Name field: `Tên nguồn` (required)
6. Footer: `Hủy` | `Thêm` (primary, disabled until name + location valid)

On submit: `add_source(projectId, name, kind, location)` then close and switch to **Nguồn** tab with the new row selected. Optional: offer `Phân tích ngay` checkbox (default on) calling `index_source` after add—if engine is slow, default **off** and show toast “Đã thêm nguồn. Chọn Phân tích khi sẵn sàng.”

### 4.8 Confirm sheets / alerts

Use for delete project (and later delete source if added). Never use `window.confirm` in production UI.

### 4.9 Toasts / banners

- **Toast:** bottom-center or top-trailing, 3–5s, auto-dismiss; for success (“Đã sao chép JSON sơ đồ.”) and mild notices.
- **Banner:** sticky under toolbar for persistent errors (API failures); dismissible; icon + message + optional `Thử lại`.
- Do not stack more than 3 toasts.

### 4.10 Progress (`IndexingProgress`)

While `source.status === "indexing"` (or busy flag around `index_source`):

- Determinate bar if file counts available (`reindexedFiles`, `fileCount`, etc.); otherwise indeterminate.
- Copy: `Đang phân tích “{name}”…` and detail `Đã xử lý {n}/{total} tệp` when known.
- Secondary action: `Hủy` — only if/when a cancel command exists; **until then omit Cancel** rather than faking it. Spec note: do not invent Rust cancel without an explicit engine task.
- Block duplicate Phân tích on the same source while indexing (disable button).

### 4.11 Status badges (`StatusBadge`)

Compact capsule (radius 6, not full pill): caption 11, padding 2×6. Text color per status table; light translucent fill optional (`color-mix` 15%).

Labels: `Chờ` | `Đang phân tích` | `Sẵn sàng` | `Lỗi`.

### 4.12 Stat tiles (`StatTile`)

Replace the four crude cards + monospace dump.

- Grid of 4 tiles: **Nguồn**, **Ký hiệu**, **Liên kết chính xác**, **Chưa xác định**.
- Each: caption label (secondary), large number (title1/title2), optional micro spark/breakdown line (callout): e.g. under exact — `theo nguồn` not required if no data; under unresolved — show count only.
- Parse `stats.summary` only if needed for a **short** secondary breakdown list (bullet or definition list), **not** a `<pre>` dump. Prefer structured fields already on `ProjectStats` / `Source`.
- Terminology: UI label **Liên kết chính xác** (maps from old “Lời gọi chính xác”); **Chưa xác định** (maps from “Chưa phân giải”). Keep API field names unchanged.

### 4.13 Empty states (`EmptyState`)

Illustration-free: one lucide icon (40–48px, secondary label color), one-line title (title2), one supporting sentence (body secondary), one primary button.

Examples:

| Context | Title | Body | CTA |
| --- | --- | --- | --- |
| No projects | `Chưa có dự án` | `Tạo dự án để gắn nguồn mã và xem sơ đồ liên kết chính xác.` | `Tạo dự án` |
| No selection | `Chọn một dự án` | `Chọn dự án ở thanh bên, hoặc tạo dự án mới.` | `Tạo dự án` |
| No sources | `Chưa có nguồn` | `Thêm thư mục cục bộ hoặc URL git để bắt đầu phân tích.` | `Thêm nguồn` |
| Graph empty | `Chưa có sơ đồ` | `Phân tích nguồn ở trạng thái Sẵn sàng rồi mở tab Sơ đồ.` | `Đến Nguồn` |

### 4.14 Tooltips

Radix Tooltip, delay 400ms. Icon-only buttons must have tooltips + `aria-label`. Include shortcut when relevant: `Thêm nguồn (⇧⌘O)`.

### 4.15 Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| ⌘N | New project (focus name field or open mini sheet) |
| ⇧⌘O | Add source sheet |
| ⌘R | Re-analyze selected source (or all ready sources if none selected—prefer **selected source only**) |
| ⌘F | Focus search |
| ⌘1 / ⌘2 / ⌘3 | Tổng quan / Sơ đồ / Nguồn |
| ⌘, | Open Settings window |
| Delete / ⌫ | Delete selected project (with confirmation sheet); ignore if renaming |
| Escape | Close sheet/popover; cancel rename |

Register via `window` keydown when not typing in inputs (except Escape/Enter). Prevent browser defaults where appropriate inside Tauri WebView.

---

## 5. Screens

### 5.1 First-run / onboarding (empty app)

When `list_projects()` returns `[]`:

- Sidebar: brand “Forgeline”, empty list with caption `Chưa có dự án`, primary `Tạo dự án` at top of sidebar (not a permanent text field—button opens inline create or small sheet with one name field).
- Content: EmptyState as above; security one-liner in sidebar footer (caption): `Dữ liệu chỉ nằm trên máy này. Không telemetry.`
- Creating a project calls `create_project`, selects it, focuses **Thêm nguồn** suggestion.

### 5.2 Project overview (`Tổng quan`)

- Header: project name (title2), relative updated time optional.
- Stat tiles row.
- Compact “breakdown” using structured stats (source count, totals)—no monospace block.
- Recent sources strip: up to 5 rows with status badge; link “Xem tất cả” → Nguồn tab.
- Primary actions: `Thêm nguồn`, `Phân tích tất cả` (iterate `index_source` sequentially with progress—optional milestone; if too heavy, omit and analyze per-row only).

### 5.3 Sources list (`Nguồn`)

**Table columns (exact order):**

| Column | Content |
| --- | --- |
| Tên | `source.name` |
| Loại | Cục bộ / Git (`local_path` / `git_url`) |
| Đường dẫn | `location` (truncate middle) |
| Tệp | `fileCount` |
| Ký hiệu | `symbolCount` |
| Chính xác | `exactCallCount` |
| Chưa xác định | `unresolvedCallCount` |
| Phân tích gần nhất | `lastIndexedAt` formatted locale `vi-VN` or relative; `—` if null |
| Trạng thái | StatusBadge |
| (actions) | `Phân tích`, overflow: `Mở sơ đồ`, … |

- Toolbar above table: search filter, `Thêm nguồn`.
- Row double-click → switch to Sơ đồ for that source (set `graphSourceId`).
- `statusMessage`: show as subtitle under name or error icon tooltip when non-null.

### 5.4 Add source sheet

See §4.7. Vietnamese validation errors:

- Empty name: `Nhập tên nguồn.`
- Empty location: `Chọn thư mục hoặc nhập URL git.`
- Backend errors: surface returned Vietnamese strings as-is.

### 5.5 Analysis in progress

- Row status → `Đang phân tích`; progress region under toolbar or in-row mini bar.
- Global busy: do not freeze the whole app—only disable conflicting actions on that source.
- On success: badge `Sẵn sàng`; toast optional.
- On error: badge `Lỗi`; banner with `statusMessage`.

### 5.6 Graph view (`Sơ đồ`)

Requires selected project. If multiple sources, a source picker (select or segmented) chooses which source feeds `source_overview`—default last-ready or previously viewed.

**Toolbar:**

- Search field (debounced → `source_overview` search arg)
- Kind filter select: `Mọi loại` + existing KIND_OPTIONS
- `Lên cấp` (disabled at root focus)
- Zoom − / + , `Vừa khung` (fit view via Sigma camera)
- `Xuất JSON` (secondary)
- Legend popover

**Canvas:** sigma.js + graphology (keep). Colors from CSS variables (read computed style once per theme change). Edges use `--graph-edge`. Background `--graph-canvas`. Labels `--graph-label`.

**Legend:** folder / file / component / type / interface / function.

**Inspector (right pane):**

- Empty: `Chọn một nút để xem chi tiết.`
- Folder/file: label, kind, symbolCount, exactOut/In; hint `Nhấp lại để khoan sâu.`
- Symbol level: button `Người gọi / Được gọi` → `symbol_neighbors`; lists Callers / Callees with `qualifiedName @ path:line` (keep evidence fields). Prefer Vietnamese headings: `Người gọi`, `Được gọi`.

**Performance acceptance:** pan/zoom remains interactive (~60fps) on overview graphs with aggregation at folder level; for 10k symbols the API already aggregates—UI must not expand all symbols at once. Destroy Sigma on tab hide.

### 5.7 Errors (Vietnamese copy)

| Situation | Copy |
| --- | --- |
| Generic failure | `Không thực hiện được.` |
| Load projects failed | `Không tải được danh sách dự án.` |
| Load sources failed | `Không tải được nguồn.` |
| Index failed | Use server `statusMessage` or `Không phân tích được nguồn.` |
| Graph load failed | `Không tải được sơ đồ.` |
| Clipboard export OK | `Đã sao chép JSON sơ đồ vào clipboard.` |
| Duplicate project name | Show backend Vietnamese error unchanged |

Banner + inline field errors; never English stack traces in UI.

### 5.8 Settings window (⌘,)

Separate Tauri window or modal sheet labeled `Cài đặt`:

- **Giao diện:** Hệ thống / Sáng / Tối
- **Thư mục dữ liệu:** show path (read-only) + caption that `FORGELINE_DATA_DIR` overrides—no file moves in v1
- **Giới thiệu:** app name, version from Tauri, one-liner privacy

No telemetry toggles (there is no telemetry). Keep window small (~520×420).

---

## 6. Motion

- Duration **150–250ms**, easing `cubic-bezier(0.25, 0.1, 0.25, 1)` (ease-out).
- Use for: tab content cross-fade, sheet enter/exit (fade + 8px translateY), sidebar selection background, inspector appear.
- Graph camera animations ≤ 250ms.
- `@media (prefers-reduced-motion: reduce)`: set durations to 0.01ms or disable non-essential transitions; keep opacity fades optional at 0.

---

## 7. Accessibility

- Contrast: text vs background **WCAG AA** (≥ 4.5:1 body, ≥ 3:1 large). Verify accent-on-white buttons.
- Full keyboard: tab order = sidebar → toolbar → content → inspector. Skip link optional.
- VoiceOver: every icon button has `aria-label` in Vietnamese; sheets `role="dialog"` `aria-modal`; live region for toasts (`aria-live="polite"`).
- Hit targets ≥ 22×22px (prefer 28×28 for toolbar icons).
- Do not rely on color alone for status—include text labels.
- Focus order preserved when opening/closing sheets; restore focus to trigger.

---

## 8. Vietnamese copy guidelines

- Short, **sentence case** (capitalize first word only, except proper nouns: Forgeline, Git, URL).
- Prefer verbs for actions: `Tạo`, `Thêm`, `Xóa`, `Phân tích`, `Đổi tên`, `Xuất`.
- Ellipsis on menu items that open sheets: `Xóa…`, `Thêm nguồn…` optional; buttons that open sheets may omit ellipsis if label is clear.
- No exclamation marks except rare success toasts.
- Consistent glossary:

| English / code | Vietnamese UI | Notes |
| --- | --- | --- |
| project | dự án | |
| source | nguồn | “nguồn mã” in longer sentences |
| symbol | ký hiệu | |
| exact call / exact edge | liên kết chính xác | Prefer over “lời gọi chính xác” in new UI |
| unresolved | chưa xác định | Prefer over “chưa phân giải” |
| analyze / index | phân tích | Button label `Phân tích` |
| graph / overview | sơ đồ | Tab `Sơ đồ` |
| local path | cục bộ / thư mục cục bộ | |
| git URL | URL git | |
| callers | người gọi | |
| callees | được gọi | |
| settings | cài đặt | |
| ready / pending / indexing / error | Sẵn sàng / Chờ / Đang phân tích / Lỗi | |

---

## 9. Implementation plan (for the implementing agent)

### Constraints

- **Do not** change Rust core or exact-only engine behavior.
- **Do not** change Tauri command signatures unless strictly required for window chrome / vibrancy / second settings window.
- Keep React + Vite + TypeScript; keep sigma.js + graphology for the graph.
- No remote fonts/CDNs; bundle all icons and CSS.
- UI strings Vietnamese; code/comments English.

### Recommended stack

| Layer | Choice |
| --- | --- |
| Styling | CSS variables + CSS Modules **or** Tailwind configured to emit the tokens above (either is fine; tokens are mandatory) |
| Primitives | Radix UI: Dialog, DropdownMenu, Popover, Tooltip, Progress |
| Icons | `lucide-react` |
| State | Local React state is enough; optional thin context for `selectedProjectId`, `tab`, `graphSourceId` |
| Persist | `localStorage` for pane widths, theme override, last tab |

### Suggested file structure

```text
apps/desktop/src/
  main.tsx
  App.tsx                          # shell only
  api.ts                           # unchanged commands
  styles/
    tokens.css
    reset.css
    app.css
  components/
    shell/
      AppShell.tsx
      Sidebar.tsx
      Toolbar.tsx
      Inspector.tsx
      PaneDivider.tsx
    controls/
      Button.tsx
      TextField.tsx
      SearchField.tsx
      SegmentedControl.tsx
      StatusBadge.tsx
      StatTile.tsx
      EmptyState.tsx
      ProgressBar.tsx
      Toast.tsx
      Banner.tsx
    project/
      ProjectOverview.tsx
      SourcesTable.tsx
      AddSourceSheet.tsx
      CreateProjectSheet.tsx
      DeleteConfirmSheet.tsx
    graph/
      SourceGraphView.tsx          # refactor existing
      GraphToolbar.tsx
      GraphLegend.tsx
      GraphInspector.tsx
    settings/
      SettingsWindow.tsx
  hooks/
    useProjects.ts
    useTheme.ts
    usePaneWidths.ts
    useHotkeys.ts
```

### Ordered milestones & acceptance

#### M1 — Tokens + shell
- [ ] `tokens.css` light/dark; SF stacks; spacing/radius
- [ ] Overlay title bar + traffic-light safe padding
- [ ] Three-pane shell with resizable sidebar; widths persist
- [ ] Vibrancy or CSS fallback sidebar
- [ ] Screenshots: light + dark empty shell
- [ ] Keyboard: focus ring visible; no layout shift on load

#### M2 — Sidebar projects
- [ ] List, selection, empty state, create project (⌘N)
- [ ] Inline rename; context menu Đổi tên / Xóa… with confirm sheet
- [ ] Security caption in footer
- [ ] Keyboard ↑/↓/Delete(confirm); VoiceOver labels
- [ ] Screenshots light/dark with selection

#### M3 — Project overview
- [ ] Stat tiles; no monospace dump
- [ ] Empty / loading / error states
- [ ] Segmented control wired (⌘1)
- [ ] Screenshots with sample stats

#### M4 — Sources table + add sheet
- [ ] Table columns as specified
- [ ] Add source sheet (local + git); auto name from folder
- [ ] Remove inline form row
- [ ] ⇧⌘O; Phân tích per row
- [ ] Screenshots; keyboard-only pass on sheet

#### M5 — Progress & errors
- [ ] Indexing progress UI; banners/toasts
- [ ] Vietnamese error copy
- [ ] Partial state (mixed source statuses) looks coherent

#### M6 — Graph polish
- [ ] Tab Sơ đồ with toolbar, legend, inspector, zoom/fit
- [ ] Dark-mode-aware node/edge colors from tokens
- [ ] Người gọi / Được gọi lists
- [ ] Fit view; destroy Sigma on hide
- [ ] Acceptance: smooth pan on aggregated ~10k-symbol source overview (folder-level nodes); no janky full-symbol dump
- [ ] Screenshots light/dark

#### M7 — Settings
- [ ] ⌘, settings; theme override; about/privacy blurb
- [ ] Checklist: reduced-motion respected; AA contrast spot-check

### Global acceptance checklist

- [ ] Light and dark screenshots for every milestone surface
- [ ] Keyboard-only critical paths: create project → add source → analyze → open graph
- [ ] No CLS when data loads (reserve skeleton/placeholder heights)
- [ ] Graph pan ~60fps on aggregated overview
- [ ] No Rust/engine behavior changes; `api.ts` commands unchanged unless windowing requires it
- [ ] UI follows this document; deviations noted in the PR

---

## 10. Reference mapping (old → new copy)

| Old UI string | New UI string |
| --- | --- |
| Lời gọi chính xác | Liên kết chính xác |
| Chưa phân giải | Chưa xác định |
| Callers / callees | Người gọi / Được gọi |
| (inline) Thêm nguồn form | Sheet `Thêm nguồn` |
| Monospace `stats.summary` | Stat tiles + short breakdown |

---

*End of specification. Implementing agents should treat this file as the source of truth for UI work and keep `docs/architecture.md` as the source of truth for engine behavior.*
