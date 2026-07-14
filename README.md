# Grok Switch

面向 **Grok CLI** 的本地 Provider / 官方账号切换器（对标 CC Switch）。

- 管理中转站：`base_url` + `api_key` + 模型，一键写入 `~/.grok/config.toml`
- 管理官方多账号：捕获 / 切换 `~/.grok/auth.json`
- 从 CC Switch 导入 Claude 中转配置
- 连通性检测、切换前自动备份、系统托盘

凭证只存本机，不上传。

---

## 环境要求

- Windows 10/11（当前优先）
- Node.js 18+
- Rust（[rustup](https://rustup.rs/)）+ Visual Studio C++ Build Tools（Tauri 需要）
- 已安装 Grok CLI（默认 `~/.grok/bin/grok.exe`）

---

## 启动

```powershell
cd grok-switch
npm install
npm run tauri:dev
```

仅预览前端（Mock API，不写真实配置）：

```powershell
npm run dev
# http://localhost:1420
```

---

## 构建安装包

```powershell
npm run tauri:build
```

产物在 `src-tauri/target/release/` 与 `src-tauri/target/release/bundle/`。

---

## 数据目录

| 路径 | 用途 |
|------|------|
| `~/.grok-switch/` | 本应用数据 |
| `~/.grok-switch/providers.json` | Provider 列表 |
| `~/.grok-switch/settings.json` | 应用设置 |
| `~/.grok-switch/accounts/<id>/` | 官方账号 auth 快照 |
| `~/.grok-switch/backups/` | 切换前备份（保留约 30 份） |
| `~/.grok-switch/activity.jsonl` | 操作日志 |
| `~/.grok/config.toml` | **写入目标**：托管 `[model.gs-*]` + `[models].default` |
| `~/.grok/auth.json` | **写入目标**：官方登录态 |
| `~/.cc-switch/cc-switch.db` | 导入源（只读） |

`~` = `C:\Users\<你>`

---

## 使用流程

### 1. 添加 / 启用中转 Provider

1. 打开 **Providers** → **添加**
2. 填写名称、Base URL（可勾选自动补 `/v1`）、API Key、协议（OpenAI / Responses / Anthropic）、默认模型
3. **测通** → **启用**
4. 终端验证：

```powershell
grok models
grok -p "只回复 ok" -m gs-<你的模型条目id>
```

托管模型段前缀固定为 `gs-`，例如 `[model.gs-myallapi-grok45]`。

### 2. 从 CC Switch 导入

1. 打开 **Import**
2. 预览 `~/.cc-switch` 中的 Claude providers
3. 勾选后导入（默认 Anthropic `messages` 协议，可再编辑）
4. 到 Providers 里启用

### 3. 官方多账号

1. 先在终端执行 `grok login`（或 device auth）
2. 打开 **Accounts** → **捕获当前登录**
3. 之后可在多个账号间切换；官方模式会把 `[models].default` 设为设置里的官方默认模型（默认 `grok-build`）

### 4. 备份恢复

切换 Provider / 账号前若开启自动备份，会写入 `~/.grok-switch/backups/<时间戳>/`。  
可在界面恢复（Activity/相关入口视版本），或手动拷回 `config.toml` / `auth.json`。

---

## 协议说明

| `api_backend` | 用途 |
|---------------|------|
| `chat_completions` | OpenAI 兼容 `/v1/chat/completions`（默认） |
| `responses` | OpenAI Responses API |
| `messages` | Anthropic Messages（与 Claude Code 中转同类） |

`messages` 使用 `extra_headers` 中的 `x-api-key`，不用 Bearer `api_key`。

---

## 功能清单（MVP）

- [x] Provider 增删改 + 一键启用
- [x] 官方账号捕获 / 切换
- [x] 连通性检测（OpenAI / Messages / Responses）
- [x] CC Switch 导入
- [x] 自动备份
- [x] Overview / Providers / Accounts / Import / Activity / Settings UI
- [x] 系统托盘（打开 / 退出）
- [x] 单实例（本机端口 47821 占用检测，二次启动直接退出）

---

## 已知限制

- 单实例不会把焦点 IPC 到首个窗口（无插件时 best-effort：二次启动退出并提示看托盘）
- 主题设置目前主要持久化，浅色主题样式可后续完善
- 不做 failover 队列、用量计费、技能同步
- 不修改系统全局 `ANTHROPIC_*` 环境变量（只写 Grok 自己的 config/auth）
- macOS/Linux 未优先打包

---

## 开发

```powershell
# 前端
npm run dev

# Rust 测试
cd src-tauri
cargo test

# 桌面调试
cd ..
npm run tauri:dev
```

设计文档：`docs/superpowers/specs/2026-07-13-grok-switch-design.md`  
实现计划：`docs/superpowers/plans/2026-07-13-grok-switch.md`

---

## 安全提示

- API Key 明文保存在 `~/.grok-switch/providers.json`（与 CC Switch 同类本机信任模型）
- 界面与日志会对密钥脱敏
- 请勿把 `providers.json`、`auth.json`、备份目录提交到公开仓库
