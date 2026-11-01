# Kiến trúc Forgeline

Forgeline là ứng dụng desktop local-first cho công việc phần mềm nhiều agent. Mỗi cuộc hội thoại là một dự án. Một dự án gắn được nhiều nguồn mã (backend, web, mobile, …). Giai đoạn 1 dựng kho dữ liệu cục bộ và đồ thị cú pháp, chưa điều phối agent.

## Vì sao không đi theo Graphify

Ba điểm yếu đã đo trên Graphify (Graphify-Labs/graphify):

- Cạnh suy đoán làm bẩn truy vấn đường đi.
- Truy vấn ngôn ngữ tự nhiên bỏ sót caller thật.
- `graph.json` quá lớn để agent đọc.

Forgeline tách những việc đó ngay từ lược đồ:

- Mọi cạnh có `provenance` (`exact` hoặc `inferred`) và `confidence`. Lệnh `callers` mặc định chỉ trả cạnh `exact`. Cạnh inferred vẫn được lưu để soi, nhưng không lẫn vào kết quả mặc định.
- Giai đoạn 1 không có truy vấn ngôn ngữ tự nhiên. `explain` và `callers` tìm đúng tên ký hiệu và trả `file:dòng`.
- Không xuất một JSON đồ thị khổng lồ. Dữ liệu nằm trong SQLite. Agent đọc tóm tắt theo tầng, dừng khi đủ ngữ cảnh.

## Các lớp

```text
Giao diện desktop (Tauri)  và  CLI
        │
        ▼
forgeline-core          dự án, nguồn, SQLite, thư mục dữ liệu
        │
        ▼
forgeline-graph         tree-sitter, băm tệp, phân giải cạnh, tóm tắt
        │
        ▼
(sau này) SCIP / LSP    kiểu, overload, default export, re-export
        │
        ▼
(sau này) liên kết xuyên nguồn
```

Lớp tree-sitter đang chạy cho Python, TypeScript, TSX và JavaScript. Nó chỉ đọc cây cú pháp. Nó không import, không eval, không chạy mã đang được phân tích.

Lớp chính xác (SCIP hoặc LSP) sẽ gắn kiểu cho `client.get()` và lời gọi qua interface. Khi chưa có kiểu, những lời gọi dạng thuộc tính không được đoán bằng tên. Đó là cố ý: đoán tên là cách cạnh sai lọt vào truy vấn đường đi.

Liên kết xuyên nguồn (client API → route backend, model/schema dùng chung, config/env) đã có mặt trong enum `EdgeKind` (`cross_source_api`, `shares_schema`, `config_ref`) nhưng indexer chưa phát sinh các cạnh đó. Chúng không dùng lại kind `calls`, để sau này lọc được riêng.

## Thư mục dữ liệu

Một thư mục dữ liệu cho cả ứng dụng. Biến `FORGELINE_DATA_DIR` ghi đè vị trí này.

| Hệ điều hành | Mặc định |
| --- | --- |
| Linux | `~/.local/share/forgeline` |
| macOS | `~/Library/Application Support/forgeline` |
| Windows | `%APPDATA%\forgeline` |

```text
forgeline/
  forgeline.db
  projects/
    <project_id>/
      sources/
        <source_id>/
          checkout/          # chỉ khi nguồn là URL git
```

Nguồn đường dẫn cục bộ không bị sao chép. Nguồn git được `git clone --depth 1` vào `checkout/` bằng git có sẵn trên máy. Lần phân tích sau không `fetch` lại, nên không có mạng ngầm. Forgeline không tự cài git.

## Mô hình dự án

- `projects`: id, tên duy nhất trong một database, thời điểm tạo và sửa.
- `sources`: thuộc một dự án. `kind` là `local_path` hoặc `git_url`. `status` là `pending`, `indexing`, `ready`, hoặc `error`.
- Bộ đếm trên từng nguồn (tệp, ký hiệu, cạnh exact/inferred, lời gọi chưa phân giải, tệp tái dùng) được ghi lại sau mỗi lần phân tích.

## Lược đồ đồ thị

Ký hiệu thống nhất, có không gian tên theo nguồn:

```text
<source_id>::<đường dẫn>::<kind>::<tên đầy đủ>@<dòng>
```

Hai nguồn trong cùng dự án không đụng id. `tên đầy đủ` của hàm ở mức module là tên hàm (`main`). Phương thức là `Box.ping`. Hàm lồng nhau là `Box.run.inner`.

Bảng:

| Bảng | Việc |
| --- | --- |
| `symbols` | hàm, lớp, phương thức, module (một module mỗi tệp) |
| `edges` | `contains`, `imports`, `calls`, và ba kind xuyên nguồn dành sẵn |
| `file_extractions` | JSON cây cú pháp thô kèm hash SHA-256, để lần sau khỏi parse lại |
| `summaries` | tóm tắt `project`, `source`, `module`, `symbol` |

Cạnh:

| Trường | Ý nghĩa |
| --- | --- |
| `provenance` | `exact` hoặc `inferred` |
| `confidence` | `1.0` nếu exact, `0.4` nếu inferred trong scaffold này |
| `evidence_path`, `evidence_line` | chỗ trong mã chứng minh cạnh |
| `detail` | `lexical`, `import`, `namespace`, `method`, `name`, `syntax` |

### Cách phân giải lời gọi

Thứ tự, dừng ở bước đầu tiên khớp:

1. **Exact / lexical.** Cùng tệp, trong phạm vi: hàm lồng trong hàm đang xét, rồi hàm hoặc lớp mức module. Hàm che tên ở ngoài.
2. **Exact / import.** Tên cục bộ được `import` hoặc `from ... import` và specifier trỏ tới đúng một tệp, đúng một ký hiệu mức module. Import không phân giải được thì lời gọi là *chưa phân giải*, không rơi xuống bước đoán tên.
3. **Exact / method.** `self`, `cls`, hoặc `this` gọi một phương thức của lớp bao quanh.
4. **Exact / namespace.** `import utils` rồi `utils.add()`, khi `add` là ký hiệu mức module của tệp đó.
5. **Inferred / name.** Lời gọi trần (không có receiver) không có binding, và đúng một ký hiệu trong nguồn mang tên đó.
6. **Bỏ qua.** Nhiều ứng viên, hoặc lời gọi thuộc tính không biết kiểu (`client.get()`). Không tạo cạnh.

Import bên ngoài (`react`, `express`) và `console.log` không sinh nút giả.

Phân tích lại theo hash: tệp không đổi và cùng phiên bản extractor được giữ nguyên phần trích xuất. Cạnh được tính lại trên toàn nguồn, vì một tệp đổi có thể làm đổi phân giải import. Tệp đã xóa biến mất khỏi đồ thị. Parser có timeout 1 giây, tệp lớn hơn 1 MiB bị bỏ, symlink không được đi theo, và indexer từ chối gốc hệ thống cùng các thư mục phụ thuộc (`node_modules`, `target`, `.git`, …).

## Tóm tắt nhiều tầng

Tóm tắt viết bằng tiếng Anh vì chúng là ngữ cảnh cho agent, đo token sơ bộ bằng số ký tự chia 4.

Thứ tự đọc: **project → source → module → symbol**. Tầng trên đủ để biết nguồn nào đáng mở. Tầng ký hiệu liệt kê lời gọi kèm exact/inferred, không nhét cả đồ thị.

Ví dụ tầng project:

```text
project sample
sources 1 | symbols 11 | calls exact 3 inferred 1
read order: project, source, module, symbol
source sample ready symbols 11 calls exact 3 inferred 1
```

## Bảo mật

- Không telemetry, không tự cập nhật, không plugin mạng trong vỏ desktop.
- Parser không thực thi mã. Git chỉ chạy khi người dùng phân tích một URL đã kiểm tra, qua `Command` với danh sách đối số, không qua shell.
- URL `https` có userinfo (`user:token@host`) bị từ chối để token không nằm trong SQLite.
- SQL dùng tham số. Khóa ngoại bật. Một file SQLite, chế độ WAL.
- CSP của cửa sổ không mở script từ mạng ngoài.

## Việc scaffold cố ý chưa làm

- Cạnh xuyên nguồn.
- Lớp SCIP/LSP, `export default`, re-export, `require()`.
- Truy vấn đường đi chỉ-exact cho agent.
- API cắt ngữ cảnh theo ngân sách token (tóm tắt đã có, chưa có tham số ngân sách).
- Điều phối nhiều agent trong cuộc hội thoại.
