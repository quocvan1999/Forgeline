# Kiến trúc Forgeline

Forgeline là ứng dụng desktop local-first cho công việc phần mềm nhiều agent. Mỗi cuộc hội thoại là một dự án. Một dự án gắn được nhiều nguồn mã (backend, web, mobile, …). Giai đoạn 1 dựng kho dữ liệu cục bộ và đồ thị cú pháp, chưa điều phối agent.

## Trọng tâm ngôn ngữ (giai đoạn 1)

- **C#** (backend): namespace, class, interface, record, struct, method, property, constructor, `using`, lời gọi, thuộc tính ASP.NET (`[HttpGet]`, `[HttpPost]`, `[Route]`, …).
- **JavaScript / React**: JS, JSX; kèm TypeScript / TSX vì web React thường dùng. Component (PascalCase), import, hooks (ghi nhận lời gọi), `fetch` / `axios` kèm URL literal.
- **Python** vẫn chạy được nhưng không còn là ưu tiên.

## Vì sao không đi theo Graphify

Ba điểm yếu đã đo trên Graphify (Graphify-Labs/graphify):

- Cạnh suy đoán làm bẩn truy vấn đường đi.
- Truy vấn ngôn ngữ tự nhiên bỏ sót caller thật.
- `graph.json` quá lớn để agent đọc.

Forgeline tách những việc đó ngay từ lược đồ:

- **Chỉ lưu cạnh exact.** Không có cạnh inferred, không có ngưỡng confidence cố định kiểu `0.4`.
- Lời gọi không phân giải được được **đếm / giữ như unresolved reference** (không thành cạnh), để đo độ phủ.
- Giai đoạn 1 không có truy vấn ngôn ngữ tự nhiên. `explain` và `callers` tìm đúng tên ký hiệu và trả `file:dòng`.
- Không xuất một JSON đồ thị khổng lồ. Dữ liệu nằm trong **một database SQLite dùng chung** cho mọi dự án. Agent đọc tóm tắt theo tầng (một ngôn ngữ duy nhất).

## Các lớp

```text
Giao diện desktop (Tauri)  và  CLI
        │
        ▼
forgeline-core          dự án, nguồn, SQLite, thư mục dữ liệu
        │
        ▼
forgeline-graph         tree-sitter (C#, JS/TS/JSX/TSX; Python phụ), hash, cạnh exact, tóm tắt
        │
        ▼
(sau này) lớp chính xác
        Roslyn / SCIP cho C#
        TypeScript compiler / scip-typescript cho JS/TS
        │
        ▼
(sau này) liên kết xuyên nguồn (API client → route, schema, config)
```

Parser chỉ đọc cây cú pháp. Không import, không eval, không chạy mã đang phân tích.

Lớp tree-sitter cố ý từ chối đoán: `_service.Find()`, `client.get()` không có kiểu → unresolved, không thành cạnh. Precision sẽ tăng khi gắn Roslyn/SCIP và scip-typescript.

Liên kết xuyên nguồn đã có kind dành sẵn (`cross_source_api`, `shares_schema`, `config_ref`) nhưng chưa phát sinh.

## Thư mục dữ liệu

Một thư mục dữ liệu và **một file `forgeline.db` dùng chung** cho mọi dự án. `FORGELINE_DATA_DIR` ghi đè vị trí.

| Hệ điều hành | Mặc định |
| --- | --- |
| Linux | `~/.local/share/forgeline` |
| macOS | `~/Library/Application Support/forgeline` |
| Windows | `%APPDATA%\forgeline` |

```text
forgeline/
  forgeline.db                 # một database cho tất cả dự án
  projects/
    <project_id>/
      sources/
        <source_id>/
          checkout/            # chỉ khi nguồn là URL git
```

Nguồn đường dẫn cục bộ không bị sao chép. Nguồn git: `git clone --depth 1` (không submodule) bằng git có sẵn trên máy. Lần phân tích sau không `fetch`. Forgeline không tự cài git.

## Mô hình dự án

- `projects`: id, tên **duy nhất không phân biệt hoa thường** (index `COLLATE NOCASE`), thời điểm tạo và sửa. Trùng tên → lỗi tiếng Việt rõ ràng.
- `sources`: thuộc một dự án. `kind` là `local_path` hoặc `git_url`. Desktop chọn thư mục bằng hộp thoại hệ điều hành.
- Bộ đếm: tệp, ký hiệu, cạnh exact, unresolved, ambiguous, tái dùng hash.

## Lược đồ đồ thị

Id ký hiệu có không gian tên theo nguồn:

```text
<source_id>::<đường dẫn>::<kind>::<tên đầy đủ>@<dòng>
```

| Bảng | Việc |
| --- | --- |
| `symbols` | module, namespace, class/interface/record/struct, function/method/constructor/property, component |
| `edges` | chỉ exact: `contains`, `imports`, `calls` (+ kind xuyên nguồn dành sẵn) |
| `file_extractions` | JSON trích xuất + hash SHA-256 |
| `summaries` | tóm tắt project → source → module → symbol (một ngôn ngữ) |

Mỗi cạnh lưu: `provenance=exact`, `confidence=1.0`, `evidence_path` / `evidence_line`, `detail` (`lexical`, `import`, `namespace`, `method`, `type`, `fqn`, `syntax`).

### Cách phân giải lời gọi (chỉ exact)

1. Lexical / cùng phạm vi trong tệp.
2. Import / using binding phân giải được tới đúng một ký hiệu.
3. Receiver `this` / `base` / `self` → method cùng class/type.
4. Namespace import hoặc tên type cùng nguồn → member duy nhất.
5. Fully-qualified name (có dấu `.`) khớp đúng một `qualified_name`.

Mọi trường hợp khác → **unresolved** (đếm, không lưu cạnh). Không có bước “tên trùng duy nhất trên toàn nguồn”.

### ASP.NET và HTTP client

- `[Route]`, `[HttpGet]` / `[HttpPost]` / … trên method (ghép với Route của class nếu có) → `RouteEndpoint` trong payload + tóm tắt.
- `fetch("...")`, `axios.get/post("...")`, một số lời gọi `HttpClient` kèm string URL → `HttpCall` (chưa nối xuyên nguồn).

## Tóm tắt nhiều tầng

Một ngôn ngữ duy nhất (không song ngữ). Thứ tự đọc: **project → source → module → symbol**. Token ước lượng ≈ số ký tự / 4.

## Bảo mật

- Không telemetry, không tự cập nhật, không plugin mạng ngoài vỏ desktop.
- Parser không thực thi mã. Git chỉ khi người dùng phân tích URL đã kiểm tra, qua `Command` không shell; `--depth 1`, không submodule.
- URL `https` có userinfo bị từ chối.
- SQL tham số; một SQLite WAL; CSP cửa sổ không mở script mạng ngoài.

## Việc scaffold cố ý chưa làm

- Cạnh xuyên nguồn (sẽ dựa route/http literal + lớp chính xác).
- Roslyn / SCIP / scip-typescript.
- `export default` phức tạp, re-export, `require()`.
- Truy vấn đường đi chỉ-exact cho agent với ngân sách token tham số hóa.
- Điều phối nhiều agent.
