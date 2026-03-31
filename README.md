# TokenMeow Client - Compute Node Desktop Application

跨平台桌面算力客戶端，使用 Tauri 3 + Rust 後端 + React 前端。

## 項目結構

```
TokenMeowClient/
├── src/                          # React 前端
│   ├── App.tsx                   # 主應用組件
│   ├── main.tsx                  # 入口點
│   └── styles.css                # 樣式
├── src-tauri/                    # Rust 後端
│   ├── src/
│   │   ├── main.rs               # 主入口
│   │   ├── lib.rs                # 庫入口
│   │   ├── commands.rs           # Tauri 命令
│   │   ├── metrics.rs            # 指標收集
│   │   └── tray.rs               # 系統托盤
│   ├── Cargo.toml                # Rust 依賴
│   ├── build.rs                  # 構建腳本
│   ├── tauri.conf.json           # Tauri 配置
│   └── capabilities/
│       └── default.json          # 權限配置
├── scripts/                      # 啟動腳本
│   ├── start_vllm_mlx.sh         # macOS vLLM-MLX 啟動
│   ├── vllm-docker-compose.yml  # Docker Compose
│   └── setup_vllm_windows.bat    # Windows 設置
├── supabase/
│   └── schema.sql                # 數據庫 Schema
├── package.json                  # npm 依賴
├── vite.config.ts                # Vite 配置
├── tsconfig.json                 # TypeScript 配置
└── tauri.conf.json               # Tauri 配置
```

## 平台路徑配置

### 模型持久化路徑

| 平台    | 路徑                                           |
|---------|-----------------------------------------------|
| macOS   | `~/Library/Application Support/ComputeNode/models` |
| Windows | `C:\ProgramData\ComputeNode\models`            |
| Linux   | `/opt/compute/models`                         |

### 平的平台判斷 (Rust cfg! 宏)

```rust
#[cfg(target_os = "macos")]
// macOS 特定代碼 (vLLM-MLX)

#[cfg(any(target_os = "windows", target_os = "linux"))]
// Windows/Linux 特定代碼 (Docker vLLM)
```

## 開發環境設置

### 前置需求

- Node.js 18+
- Rust 1.70+
- Tauri CLI 3.x
- Xcode Command Line Tools (macOS)
- Visual Studio Build Tools (Windows)
- Docker (Windows/Linux)

### 安裝依賴

```bash
cd TokenMeowClient

# 安裝 npm 依賴
npm install

# 安裝 Rust 依賴
cd src-tauri && cargo fetch && cd ..
```

### 開發模式

```bash
npm run tauri dev
```

### 生產構建

```bash
npm run tauri build
```

## GitHub Actions 自動構建（推薦）

每次 push 或發布 tag 都會自動生成三平台安裝檔：

1. 將代碼推送到 GitHub
2. 前往 **Actions** 頁面查看構建進度
3. 構建完成後，在 **Release** 或 **Artifacts** 下載各平台安裝檔

### 手動本地構建

#### 前置需求

- Node.js 18+
- Rust 1.70+
- Xcode Command Line Tools (macOS)
- Visual Studio Build Tools (Windows)
- Docker (Windows/Linux)

#### macOS (.dmg)

```bash
npm run tauri build -- --target universal-apple-darwin

# 或指定 target
npm run tauri build -- --target aarch64-apple-darwin
npm run tauri build -- --target x86_64-apple-darwin
```

### Windows (.exe / .msi)

**注意：Windows 安裝檔需要在 Windows 環境中構建。**

使用 GitHub Actions（推薦）：
1. 推送代碼到 GitHub
2. 在 Releases 頁面下載 `*-x64-setup.exe`

本地 Windows 構建：
```powershell
npm run tauri build
```

### Linux (.AppImage / .deb)

```bash
# AppImage
npm run tauri build -- --target x86_64-unknown-linux-gnu

# Debian
npm run tauri build -- --target x86_64-unknown-linux-gnu --bundles deb
```

## GitHub Updater 配置

### 1. 生成更新密鑰對

```bash
cd src-tauri
openssl rand -base64 32 > public-key.pem
```

### 2. 在 GitHub Releases 添加 assets

發布格式：`v{version}-{target}.{ext}`

例如：
- `TokenMeowClient_1.0.0_x64.dmg`
- `TokenMeowClient_1.0.0_x64-setup.exe`
- `TokenMeowClient_1.0.0_amd64.AppImage`

### 3. 創建 latest.json

```bash
curl -s https://api.github.com/repos/rickyrvl/TokenMeowClient/releases/latest \
  | jq '{version: .tag_name, notes: .body, pub_date: .published_at, platforms: {}}'
```

## Supabase 數據庫設置

### 執行 Schema

在 Supabase SQL Editor 中執行 `supabase/schema.sql`。

### 驗證表結構

```sql
-- 檢查 machines 表
SELECT * FROM machines LIMIT 5;

-- 檢查 metrics_raw 表
SELECT * FROM metrics_raw ORDER BY recorded_at DESC LIMIT 10;
```

## 功能驗證清單

### 核心功能

- [x] QR Code SSO 登入 (Google/Facebook/Apple/Email)
- [x] 機器名稱註冊
- [x] GPU 檢測 (NVIDIA / Apple Silicon)
- [x] macOS: vLLM-MLX 自動安裝與啟動
- [x] Windows/Linux: Docker + vLLM 自動安裝與啟動
- [x] Tailscale headless 模式啟動
- [x] 模型持久化到固定 Volume

### 後台任務

- [x] 每 10 秒抓取 tokens/s + GPU 使用率 → 上報 Supabase metrics_raw
- [x] 每 30 秒上報 Tailscale IP + port 到 machines 表
- [x] Tauri Updater 每 24 小時檢查 GitHub Releases
- [x] 靜默升級 (silent: true)

### UI/UX

- [x] 系統托盤 (Tray Icon) 顯示狀態
- [x] 點擊托盤圖標開啟主視窗
- [x] React 管理頁顯示在線機器列表
- [x] 一鍵測試 API 按鈕

## API 端點

### vLLM-MLX / vLLM OpenAI 兼容 API

```
POST http://{tailscale_ip}:8000/v1/chat/completions
GET  http://{tailscale_ip}:8000/v1/models
GET  http://{tailscale_ip}:8000/v1/metrics
```

### Supabase REST API

```
GET  /rest/v1/machines
PATCH /rest/v1/machines?machine_id=eq.{id}
POST /rest/v1/metrics_raw
```

## 故障排除

### macOS

```bash
# 檢查 vLLM-MLX 是否運行
curl http://127.0.0.1:8000/v1/models

# 查看進程
ps aux | grep vllm

# 查看日誌
log show --predicate 'process == "python"' --last 1h
```

### Windows

```powershell
# 檢查 Docker
docker ps

# 查看 vLLM 日誌
docker logs -f vllm-server

# 重啟服務
docker restart vllm-server
```

### Linux

```bash
# 檢查 Docker
sudo systemctl status docker
docker ps

# 查看 vLLM 日誌
docker logs -f vllm-server
```

## 證書

MIT License
