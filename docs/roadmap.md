# Lộ trình

Phạm vi giai đoạn 1: kho dự án cục bộ và đồ thị cú pháp **exact-only**, trọng tâm C# + JavaScript/React. Điều phối agent nằm sau khi đồ thị đọc được từ trên xuống mà không kéo cạnh sai.

## Giai đoạn 1 — nền cục bộ

- [x] Workspace Cargo: `forgeline-core`, `forgeline-graph`, `forgeline-cli`, vỏ Tauri
- [x] Tạo / đổi tên / xóa dự án; **một** SQLite dùng chung; tên dự án duy nhất không phân biệt hoa thường
- [x] Thêm nguồn đường dẫn cục bộ (hộp thoại hệ điều hành) hoặc URL git; trạng thái từng nguồn
- [x] Tree-sitter **C#** (namespace, type, method, property, constructor, using, invocation, ASP.NET routes)
- [x] Tree-sitter **JS / JSX / TS / TSX** với React component, import, fetch/axios URL literal
- [x] Chỉ cạnh exact; unresolved được đếm; không lưu inferred
- [x] Phân tích lại theo hash; id ký hiệu namespaced theo nguồn
- [x] Tóm tắt project → source → module → symbol (một ngôn ngữ)
- [x] CLI: `index`, `stats`, `explain`, `callers` (không còn `--include-inferred`)
- [x] Desktop: dự án, nguồn, phân tích, thống kê exact / unresolved
- [ ] Truy vấn đường đi chỉ dùng cạnh exact
- [ ] Cắt đồ thị con exact theo ngân sách token
- [ ] Cạnh xuyên nguồn: API client → route, schema, config (kind đã dành chỗ)
- [ ] Giữ Python ở mức bảo trì; không mở rộng trừ khi cần

## Giai đoạn 2 — lớp chính xác

- [ ] **C#:** Roslyn và/hoặc SCIP để gắn kiểu, overload, lời gọi qua interface
- [ ] **JS/TS:** TypeScript compiler / scip-typescript cho import resolution và kiểu
- [ ] `export default`, re-export, path mapping
- [ ] Liên kết xuyên nguồn trên lớp chính xác (không đoán tên)
- [ ] API agent đọc từ tầng project xuống, dừng khi hết ngân sách

## Giai đoạn 3 — công việc nhiều agent

- [ ] Một cuộc hội thoại gắn một dự án và nhiều repo
- [ ] Agent nhận tóm tắt + cạnh exact, không nhận nguyên `graph.json`
- [ ] Gắn `file:dòng` và provenance trong câu trả lời agent

## Quyết định đã khóa

- Không inferred edges; chỉ exact + đếm unresolved.
- Một database dùng chung cho mọi dự án.
- Tên dự án unique case-insensitive.
- Tóm tắt agent một ngôn ngữ (không song ngữ).
- `git clone --depth 1` không submodule.
- Trọng tâm ngôn ngữ: C# + JS/React (TS/TSX đi kèm).
