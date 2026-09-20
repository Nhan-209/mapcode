# MapCode 🗺️⚡

> **Blazing-fast in-memory code map MCP server for AI coding assistants (Claude, Cursor, Antigravity, Cline, Roo-Code).**

MapCode parses and indexes your codebase in RAM using **Tree-sitter**, maintaining a realtime bidirectional Call Graph and symbol index. Instead of forcing AI assistants to repeatedly search through dozens of files or grep blindly, MapCode gives AI instant access to:
- 📌 File outlines with exact line ranges and function signatures.
- 🔍 Symbol definitions across the entire workspace.
- 🕸️ Bidirectional call graphs (**who calls this function** and **what functions does it call**).
- ⚡ Fuzzy symbol search with intelligent ranking.
- 📊 Project-level code metrics and language statistics.

---

## 🚀 Triết lý "Zero Local Build" (Không tốn tài nguyên máy cá nhân)

Dự án được tối ưu hoá theo triết lý **100% Cloud Build**:
- **Không cần cài đặt Rust hay Cargo trên máy tính của bạn**: Máy cấu hình thấp vẫn sử dụng mượt mà không lo giật lag hay tốn RAM/CPU để biên dịch.
- **GitHub Actions tự động hoá hoàn toàn**: Mọi commit và tag push đều được kiểm tra cú pháp (`cargo check`), chạy unit test (`cargo test`), kiểm tra chất lượng code (`clippy`), và biên dịch siêu tối ưu (`lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`).
- **Tải binary chạy ngay từ Releases**: Bạn chỉ việc tải file chạy (`.exe` cho Windows hoặc binary cho Linux/macOS) từ tab [Releases](https://github.com/Nhan-209/mapcode/releases) về và thêm vào cấu hình MCP client.

---

## 📦 Cài đặt & Cấu hình MCP Client

### Bước 1: Tải binary từ GitHub Releases
Tải file nén tương ứng với hệ điều hành của bạn từ mục **Releases**:
- **Windows (x64)**: `mapcode-windows-x64.zip` (giải nén lấy `mapcode.exe`)
- **Linux (x64)**: `mapcode-linux-x64.tar.gz`
- **macOS Apple Silicon (M1/M2/M3/M4)**: `mapcode-macos-arm64.tar.gz`
- **macOS Intel (x64)**: `mapcode-macos-x64.tar.gz`

Lưu file binary vào một thư mục tiện lợi, ví dụ: `C:\tools\mapcode.exe` (Windows) hoặc `/usr/local/bin/mapcode` (Linux/macOS).

---

### Bước 2: Cấu hình vào AI Assistant

#### 1. Claude Desktop
Mở file cấu hình Claude Desktop:
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`

Thêm cấu hình sau:
```json
{
  "mcpServers": {
    "mapcode": {
      "command": "C:\\tools\\mapcode.exe",
      "args": ["D:\\path\\to\\your\\project"]
    }
  }
}
```
*(Nếu dùng macOS/Linux, thay `command` bằng đường dẫn tới binary `mapcode` và `args` bằng thư mục dự án).*

---

#### 2. Cursor IDE
Tạo hoặc mở file cấu hình `.cursor/mcp.json` trong thư mục dự án (hoặc trong Cursor Settings -> Features -> MCP):
```json
{
  "mcpServers": {
    "mapcode": {
      "command": "C:/tools/mapcode.exe",
      "args": ["."]
    }
  }
}
```

---

#### 3. Antigravity / Cline / Roo-Code
Thêm vào file cấu hình MCP (`mcpSettings.json`):
```json
{
  "mcpServers": {
    "mapcode": {
      "command": "C:/tools/mapcode.exe",
      "args": ["${workspaceFolder}"],
      "disabled": false,
      "autoApprove": [
        "get_file_outline",
        "find_definition",
        "get_call_graph",
        "fuzzy_search_symbols",
        "get_project_stats"
      ]
    }
  }
}
```

---

## 🛠️ Danh sách 5 Tools MCP Cung Cấp Cho AI

| Tên Tool | Tham số | Mô tả |
| :--- | :--- | :--- |
| `get_file_outline` | `path: string` | Trích xuất toàn bộ cấu trúc file: các hàm, class, struct, trait, enum kèm số dòng bắt đầu/kết thúc, chữ ký (signature) và docstring. Hỗ trợ cả đường dẫn tương đối và tuyệt đối. |
| `find_definition` | `name: string` | Tra cứu nhanh nơi định nghĩa của bất kỳ symbol nào trên toàn bộ dự án. Hỗ trợ cả tên đơn (ví dụ: `parse_file`) và qualified name (ví dụ: `Point::distance` hoặc `Calculator.calculate`). |
| `get_call_graph` | `name: string` | Trả về đồ thị cuộc gọi: ai gọi hàm này (**callers**) và hàm này gọi những hàm nào (**callees**). Hỗ trợ cả qualified name. |
| `fuzzy_search_symbols`| `query: string`, `kind?: string`, `limit?: int` | Tìm kiếm mờ symbol theo tên hoặc lọc theo loại (`function`, `struct`, `class`, `trait`,...). Chấp nhận limit dạng số hoặc chuỗi. |
| `get_project_stats` | *(không)* | Báo cáo tổng quan số lượng file, số symbol, phân bổ theo ngôn ngữ trong dự án (Rust, Python, TypeScript, JavaScript). |

---

## 🌐 Các Ngôn Ngữ Được Hỗ Trợ

- 🦀 **Rust**: `.rs` (functions, methods, structs, enums, traits, impl blocks, macros)
- 🐍 **Python**: `.py` (functions, methods, classes, docstrings `"""..."""`)
- 📘 **TypeScript / TSX**: `.ts`, `.tsx` (functions, arrow functions, classes, interfaces, type aliases)
- 💛 **JavaScript / JSX**: `.js`, `.jsx`, `.mjs`, `.cjs` (functions, classes, methods, arrow functions)

---

## ⚙️ Kiến Trúc Hệ Thống (Architecture)

```
                       +-----------------------------+
                       |       AI Assistant          |
                       |  (Claude / Cursor / Cline)  |
                       +--------------+--------------+
                                      | Stdio (JSON-RPC 2.0)
                                      v
                       +-----------------------------+
                       |      src/mcp_server.rs      |
                       |   (5 MCP Analysis Tools)    |
                       +--------------+--------------+
                                      |
                                      v
                       +-----------------------------+
                       |        src/store.rs         |
                       |     In-Memory Code Map      |
                       |   DashMap Realtime Cache    |
                       | - Definitions  - Outline    |
                       | - Call Graph   - Fuzzy Search
                       +-------+--------------+------+
                               ^              ^
                AST Parsing    |              | Incremental Updates
                               |              |
                    +----------+--+        +--+------------+
                    | src/parser  |        | src/watcher   |
                    | Tree-sitter |        | notify daemon |
                    +-------------+        +---------------+
```

1. **Initial Walk (`ignore` crate)**: Quét toàn bộ repo khi khởi động, tôn trọng `.gitignore` và bỏ qua các thư mục rác (`target/`, `node_modules/`, `.git/`).
2. **Incremental Realtime Watcher (`notify` crate)**: Theo dõi sự kiện thay đổi file trên hệ thống. Khi bạn lưu file, MapCode chỉ re-parse đúng file đó và cập nhật đồ thị liên kết trên RAM trong vài mili-giây mà không quét lại toàn bộ repo.
3. **Pure Stdio JSON-RPC**: Mọi log thông tin được đẩy ra `stderr` (`eprintln!`), đảm bảo luồng `stdout` thuần khiết 100% cho giao thức MCP.

---

## 🧪 CI/CD & Đóng Góp (Development Workflow)

Vì dự án tuân thủ triết lý không build local:
1. Bạn chỉ cần chỉnh sửa code trong thư mục `src/`.
2. Commit và push lên GitHub:
   ```bash
   git add .
   git commit -m "feat: enhance call graph resolution"
   git push origin main
   ```
3. GitHub Actions (`.github/workflows/ci.yml`) sẽ tự động chạy:
   - `cargo check`
   - `cargo test`
   - `cargo clippy -- -D warnings`
4. Khi muốn phát hành phiên bản mới, chỉ cần tạo tag (ví dụ `v0.1.0`):
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
   GitHub Actions (`.github/workflows/release.yml`) sẽ tự động build binary cho Windows, Linux, macOS và đính kèm trực tiếp vào mục Releases.

---

## 📄 Bản Quyền (License)

Dự án được phân phối dưới giấy phép [MIT License](LICENSE).
