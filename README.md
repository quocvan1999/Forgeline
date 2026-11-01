# Forgeline

Forgeline là ứng dụng desktop local-first, ưu tiên bảo mật, cho công việc phần mềm nhiều agent. Mỗi cuộc hội thoại là một dự án. Một dự án gắn được nhiều nguồn mã, ví dụ backend C# và web React.

Giai đoạn này là khung và lõi đồ thị, chưa điều phối agent. Bạn tạo dự án, thêm nguồn, phân tích mã (tree-sitter + lớp chính xác tùy chọn), rồi xem ký hiệu, lời gọi `file:dòng`, và **Sơ đồ** tổng quan theo thư mục. **Chỉ cạnh exact được lưu**; lời gọi không chắc chắn được đếm là unresolved, không thành cạnh.

Nguyên tắc:

- Không telemetry, không tự cập nhật, không tự cài parser hay git.
- Parser chỉ đọc cú pháp, không chạy mã đang phân tích. Roslyn chạy như subprocess với argv tường minh, không qua shell.
- Dữ liệu nằm trong một thư mục trên máy bạn; **một database SQLite dùng chung** cho mọi dự án.
- Tên dự án duy nhất, không phân biệt hoa thường.

Tài liệu: [kiến trúc](docs/architecture.md), [lộ trình](docs/roadmap.md).

## Yêu cầu

- Rust stable (rustup), kèm `rustfmt` và `clippy`. File `rust-toolchain.toml` chọn toolchain stable.
- Trình biên dịch C: MSVC trên Windows, Xcode Command Line Tools trên macOS, GCC trên Linux.
- Node.js 20 trở lên và npm, chỉ khi build app desktop.
- Git có sẵn trên `PATH` chỉ khi bạn phân tích một URL git (`git clone --depth 1`, không submodule). Forgeline không cài git giúp bạn.
- **.NET SDK 8** (tùy chọn) để bật lớp Roslyn cho C#. Nếu thiếu SDK, indexer vẫn chạy bằng tree-sitter và hiện thông báo tiếng Việt trên CLI/UI.

Thư viện hệ thống cho vỏ desktop:

- **Linux:** `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf`, `pkg-config`.
- **macOS:** WebKit của hệ thống.
- **Windows:** WebView2, có sẵn trên Windows 11.

## Dữ liệu nằm ở đâu

| Hệ điều hành | Thư mục |
| --- | --- |
| Linux | `~/.local/share/forgeline` |
| macOS | `~/Library/Application Support/forgeline` |
| Windows | `%APPDATA%\forgeline` |

Database là `forgeline.db` trong thư mục đó (một file cho tất cả dự án). Đặt `FORGELINE_DATA_DIR` để dùng chỗ khác. CLI còn nhận `--db <đường-dẫn-file>`.

## CLI

Từ gốc repo:

```bash
# (tuỳ chọn) build helper Roslyn
dotnet build tools/forgeline-roslyn -c Release

cargo test
cargo run -p forgeline-cli -- index examples/sample --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- stats --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- explain App --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- callers Find --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- overview sample --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- overview sample --focus backend --project sample --db .forgeline/forgeline.db --out /tmp/overview.json
```

`callers` chỉ trả cạnh exact. `overview` xuất JSON sơ đồ theo thư mục/tệp (chỉ cạnh exact).

Cùng một dự án, nhiều nguồn:

```bash
cargo run -p forgeline-cli -- index ./backend --project shop --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- index ./web --project shop --db .forgeline/forgeline.db
```

Bỏ `--db` thì CLI ghi vào thư mục dữ liệu của hệ điều hành, cùng chỗ với app desktop.

Lệnh kiểm tra Rust (không build vỏ desktop, vì vỏ cần thư viện WebView):

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`cargo test` và `cargo clippy` dùng default-members: ba crate Rust. Crate desktop nằm trong workspace nhưng không thuộc default-members.

### Roslyn (C#)

```bash
# Cần .NET SDK 8 trên PATH (hoặc DOTNET_ROOT)
dotnet build tools/forgeline-roslyn -c Release
# Tuỳ chọn: trỏ thẳng vào dll đã build
export FORGELINE_ROSLYN="$PWD/tools/forgeline-roslyn/bin/Release/net8.0/forgeline-roslyn.dll"
```

Indexer gọi helper bằng `Command` + argv (`--root`, `--timeout-ms`), timeout mặc định 120s. Không có SDK → thông báo tiếng Việt, giữ tree-sitter.

### JS/TS module resolution

Phân giải exact: import tương đối, `index.*`, phần mở rộng `.ts/.tsx/.js/.jsx/.mjs/.cjs/.d.ts`, `tsconfig`/`jsconfig` `baseUrl` + `paths` (theo `extends`), `package.json` `exports`/`main` của package workspace cục bộ. Re-export / `export * from` được nối khi đích duy nhất. npm ngoài → cạnh `external_module` hoặc unresolved, không đoán.

## Desktop

```bash
cd apps/desktop
npm install
npm run tauri dev
```

Build gói cài:

```bash
cd apps/desktop
npm install
npm run tauri build
```

Cửa sổ: tạo / đổi tên / xóa dự án; thêm nguồn; Phân tích; xem exact / unresolved; nút **Sơ đồ** (sigma.js + graphology, bundle local, không CDN) với tìm kiếm, lọc kind, khoan sâu thư mục → tệp → ký hiệu, callers/callees. Xuất JSON sơ đồ từ UI hoặc CLI.

Chỉ kiểm tra kiểu và bundle frontend:

```bash
cd apps/desktop
npm install
npm run build
cargo check -p forgeline-desktop
```

## Bố cục repo

```text
crates/forgeline-graph   tree-sitter + module resolution + Roslyn merge
crates/forgeline-core    dự án, nguồn, SQLite, overview
crates/forgeline-cli     bin forgeline
apps/desktop             React + Vite + Tauri 2
tools/forgeline-roslyn   .NET 8 console (Microsoft.CodeAnalysis)
examples/sample          mẫu C# ASP.NET-style + React
docs                     kiến trúc và lộ trình
```

## Giấy phép

MIT. Xem `LICENSE`.
