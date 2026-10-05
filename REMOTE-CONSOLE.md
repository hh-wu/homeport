# Sageport RC —— 远程访问控制台

本目录是 [Sageport](https://github.com/joygqz/sageport) 的 fork，在原有 SSH 工作台之上加入了一个
「远程访问」面板：控制本机 frpc 的启停、查看中转服务器上的代理状态、查询阿里云 ECS 资源与流量。
程序常驻系统托盘，关闭窗口只是隐藏。

上游 Sageport 的能力（终端、SFTP、任务、监控、端口转发、AI 助手等）全部保留。

## 与原版的差异

| 位置 | 改动 |
|---|---|
| `src-tauri/src/commands/remote_console.rs` | 新增：frpc 启停/状态/看门狗、frps 面板查询、阿里云 ECS 与计费查询 |
| `src-tauri/src/commands/mod.rs` | 注册新模块 |
| `src-tauri/src/lib.rs` | 注册 10 个 IPC 命令，启动看门狗任务 |
| `src-tauri/src/tray.rs` | 托盘菜单加入「启动 frpc / 停止 frpc」，并广播 `rc://status` |
| `src-tauri/Cargo.toml` | 新增依赖 `hmac`、`hex`、`windows-sys`（进程枚举）；bin 名改为 `SageportRC` |
| `src-tauri/tauri.conf.json` | 产品名 `Sageport RC`，标识符 `com.nick0x01.sageportrc` |
| `src/features/remote-console/` | 新增视图与数据层（`RemoteConsoleView.tsx`、`api.ts`） |
| `src/lib/ipc.ts` | 新增 `ipc.remoteConsole` 命名空间 |
| `src/types/models.ts` | 新增 `RcStatus`、`RcCloud`、`RcRustDesk` |
| `src/workbench/{ActivityBar,SideBar,layout-state}` | 新增 `remote` 活动视图入口 |
| `src/i18n/locales/{en,zh-CN}.ts` | 新增 `remote.*` 与 `activityBar.remote` 文案（双语同步） |
| `scripts/check-conventions.mjs` | 顺带修复 Windows 路径 bug（`URL.pathname` → `fileURLToPath`），原版在 Windows 上必失败 |
| `src/i18n/locales/parity.test.ts` | 允许清单补入 `IP`、`RustDesk` |

## 面板内容

- **运行状态**：frpc 进程、PID、运行时长（从日志推算）、启动/停止按钮、掉线自动重启（看门狗）
- **服务器侧代理**：按需查询 frps 面板，确认 `home-rdp` 与 `home-ssh` 在线
- **云资源**：ECS 实例状态/规格/可用区/公网 IP、实时出/入带宽、本月出/入流量、估算流量费、账户余额、本月账单
- **RustDesk**：自建服务器地址与 Key，一键复制
- **日志**：frpc 日志尾部，可用编辑器打开

停止 frpc 会弹出确认对话框，并说明后果——本机是服务提供方，停止后外部连接会全部断开。

## 托盘行为

- 关闭窗口 = 隐藏到托盘，程序继续运行，看门狗继续工作
- 左键点击托盘图标 = 显示主窗口
- 右键菜单：显示主窗口 / 定时任务 / 端口转发 / **启动 frpc** / **停止 frpc** / 退出

## 构建

```powershell
pnpm install
pnpm build
cd src-tauri
cargo build --release --features custom-protocol
```

产物：`src-tauri\target\release\SageportRC.exe`

若要复用其他 Sageport 检出的依赖缓存，可设置：

```powershell
$env:CARGO_TARGET_DIR = "<其他检出>\src-tauri\target"
```

## 运行

直接运行 exe 即可。数据目录按以下优先级解析（见 `src-tauri/src/paths.rs`）：

1. 环境变量 `SAGEPORT_DATA_DIR`
2. exe 同级的 `data` 目录（存在即为**便携模式**）
3. 系统应用数据目录（由标识符 `com.nick0x01.sageportrc` 决定）

因此本 fork 与原版 Sageport 的数据互不干扰。

## 依赖的外部条件

| 依赖 | 用途 | 缺失时表现 |
|---|---|---|
| `D:\Programs\frp\frpc.exe` + `frpc.toml` | 启停对象 | 启动报「未找到」 |
| `~\.aliyun\credentials.json` | 云资源查询的 AccessKey | 云资源区显示错误原因 |
| `~\.ssh\id_ed25519` | SSH 到中转服务器查 frps 面板 | 代理检查失败 |
| WebView2 | 渲染界面 | Win11 自带 |

阿里云凭据优先读环境变量 `ALIBABA_CLOUD_ACCESS_KEY_ID` / `ALIBABA_CLOUD_ACCESS_KEY_SECRET`，
源码中不硬编码密钥。

## 校验

```powershell
pnpm format:check
pnpm check:conventions
pnpm typecheck
pnpm test
pnpm lint
cd src-tauri; cargo test --lib
```

当前基线（Windows，2026-10-05）：

| 检查 | 结果 |
|---|---|
| `pnpm typecheck` | 通过 |
| `pnpm format:check` | 通过 |
| `pnpm check:conventions` | 通过 |
| `pnpm lint` | 通过 |
| `pnpm test` | 74 文件 / 514 用例全通过 |
| `cargo test --lib` | 185 通过 / **13 失败** |

那 13 个失败是**上游在 Windows 上的既有问题**，与本 fork 无关：原始检出跑同一套用例，失败数量与名单完全一致。
集中在 `sshkey`、`legacy`、`sync`（依赖 POSIX 文件权限）、`db`（含特殊字符的路径）、`tasks`、`ssh_config`。

## 许可

沿用上游 [GPL-3.0-only](LICENSE)。
