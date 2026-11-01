# Lộ trình

Phạm vi giai đoạn 1 là kho dự án cục bộ và đồ thị cú pháp. Điều phối agent nằm sau, khi đồ thị đọc được từ trên xuống mà không kéo theo cạnh sai.

## Giai đoạn 1 — nền cục bộ

- [x] Workspace Cargo: `forgeline-core`, `forgeline-graph`, `forgeline-cli`, vỏ Tauri
- [x] Tạo, đổi tên, xóa dự án; một database SQLite trong thư mục dữ liệu ứng dụng
- [x] Thêm nguồn đường dẫn cục bộ hoặc URL git; trạng thái phân tích riêng từng nguồn
- [x] Trích xuất tree-sitter cho Python, TypeScript, TSX, JavaScript: ký hiệu, import, lời gọi, kèm `file:dòng`
- [x] `provenance` exact / inferred và `confidence`
- [x] Phân tích lại theo hash nội dung; tệp không đổi không parse lại
- [x] Id ký hiệu có không gian tên theo nguồn
- [x] Tóm tắt project → source → module → symbol
- [x] CLI: `index`, `stats`, `explain`, `callers` (mặc định chỉ exact)
- [x] Desktop: danh sách dự án, tạo / đổi tên / xóa, thêm nguồn, phân tích, thống kê
- [ ] Truy vấn đường đi chỉ dùng cạnh exact
- [ ] Cắt một đồ thị con exact vừa một cửa sổ ngữ cảnh của agent
- [ ] Tham số ngân sách token khi đọc tóm tắt
- [ ] Cạnh xuyên nguồn: API client → route, schema dùng chung, config/env (kind đã dành chỗ)
- [ ] Ngôn ngữ tiếp theo sau Python và JS/TS (Rust, Go, …)

## Giai đoạn 2 — lớp chính xác

- [ ] SCIP hoặc LSP cho kiểu, overload, và lời gọi qua interface
- [ ] `export default` và re-export
- [ ] Liên kết xuyên nguồn trên lớp chính xác, không trên đoán tên
- [ ] API để agent đọc từ tầng project xuống, dừng khi hết ngân sách

## Giai đoạn 3 — công việc nhiều agent

- [ ] Một cuộc hội thoại gắn một dự án và nhiều repo
- [ ] Agent nhận tóm tắt và cạnh exact, không nhận nguyên `graph.json`
- [ ] Ghi lại nguồn chứng cứ (`file:dòng`, provenance) trong câu trả lời của agent

## Câu hỏi còn mở

- Ngưỡng `0.4` của cạnh inferred có nên để người dùng chỉnh không, hay cố định để kết quả so sánh được?
- Có nên ngừng lưu cạnh inferred và chỉ tính khi được yêu cầu? Hiện chúng được lưu, còn truy vấn mặc định thì bỏ qua.
- Tên dự án đang là duy nhất trong một database. Có cần cho phép trùng tên và chọn bằng id không?
- Một file SQLite cho cả ứng dụng tiện sao lưu, nhưng sẽ lớn. Có tách một database mỗi dự án không?
- Tóm tắt cho agent đang là tiếng Anh; giao diện và tài liệu là tiếng Việt. Có cần tóm tắt song ngữ không?
- `git clone` dùng git của máy, `--depth 1`, không submodule, không tự cấu hình credential. Cách này có đủ cho repo riêng không?
