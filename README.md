# Forgeline

Forgeline là ứng dụng desktop local-first, ưu tiên bảo mật, cho công việc phần mềm nhiều agent. Mỗi cuộc hội thoại là một dự án. Một dự án gắn được nhiều nguồn mã, ví dụ backend, web và mobile.

Giai đoạn này là khung và lõi đồ thị, chưa điều phối agent. Bạn tạo dự án, thêm nguồn, phân tích mã bằng tree-sitter, rồi xem ký hiệu và lời gọi kèm vị trí `file:dòng`. Cạnh chính xác và cạnh suy đoán được tách riêng để truy vấn caller không bị nhiễu.

Nguyên tắc:

- Không telemetry, không tự cập nhật, không tự cài parser hay git.
- Parser chỉ đọc cú pháp, không chạy mã đang phân tích.
- Dữ liệu nằm trong một thư mục trên máy bạn.

Tài liệu: [kiến trúc](docs/architecture.md), [lộ trình](docs/roadmap.md).

## Yêu cầu

- Rust stable (rustup), kèm `rustfmt` và `clippy`. File `rust-toolchain.toml` chọn toolchain stable.
- Trình biên dịch C: MSVC trên Windows, Xcode Command Line Tools trên macOS, GCC trên Linux.
- Node.js 20 trở lên và npm, chỉ khi build app desktop.
- Git có sẵn trên `PATH` chỉ khi bạn phân tích một URL git. Forgeline không cài git giúp bạn.

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

Database là `forgeline.db` trong thư mục đó. Đặt `FORGELINE_DATA_DIR` để dùng chỗ khác. CLI còn nhận `--db <đường-dẫn-file>`.

## CLI

Từ gốc repo:

```bash
cargo test
cargo run -p forgeline-cli -- index examples/sample --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- stats --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- explain main --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- callers health --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- callers orphan --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- callers orphan --include-inferred --project sample --db .forgeline/forgeline.db
```

`callers` mặc định chỉ in cạnh exact. Cạnh inferred (khớp mỗi tên, không có import) bị ẩn và được đếm ở dòng gợi ý.

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

Cửa sổ cho phép tạo, đổi tên, xóa dự án; thêm nguồn cục bộ bằng hộp thoại chọn thư mục của hệ điều hành, hoặc dán URL git; bấm Phân tích; xem số ký hiệu và lời gọi exact / inferred. Giao diện tiếng Việt. Tóm tắt cho agent tiếng Anh.

Lần đầu biên dịch Rust của Tauri khá lâu. Trên Linux, nếu thiếu WebKitGTK, cài các gói ở mục yêu cầu rồi chạy lại.

Chỉ kiểm tra kiểu và bundle frontend, không mở cửa sổ:

```bash
cd apps/desktop
npm install
npm run build
cargo check -p forgeline-desktop
```

## Bố cục repo

```text
crates/forgeline-graph   tree-sitter, hash, cạnh, tóm tắt
crates/forgeline-core    dự án, nguồn, SQLite
crates/forgeline-cli     bin forgeline
apps/desktop             React + Vite + Tauri 2
examples/sample          Python và TypeScript mẫu
docs                     kiến trúc và lộ trình
```

## Giấy phép

MIT. Xem `LICENSE`.
