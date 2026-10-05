# Homeport —— 远程访问控制台

本目录是 [Sageport](https://github.com/joygqz/sageport) 的 fork，在原有 SSH 工作台之上加入了一个
「远程访问」面板：控制本机 frpc 的启停、查看中转服务器上的代理状态、查询阿里云 ECS 资源与流量。
程序常驻系统托盘，关闭窗口只是隐藏。

上游 Sageport 的能力（终端、SFTP、任务、监控、端口转发、AI 助手等）全部保留。

## 与原版的差异

| 位置                                               | 改动                                                                                                    |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/commands/remote_console.rs`         | 新增：frpc 启停/状态/看门狗、frps 服务器状态与端口映射查询（结构化）、阿里云 ECS 与计费查询、配置持久化 |
| `src-tauri/src/commands/mod.rs`                    | 注册新模块                                                                                              |
| `src-tauri/src/lib.rs`                             | 注册 10 个 IPC 命令，启动看门狗任务                                                                     |
| `src-tauri/src/tray.rs`                            | 托盘菜单加入「启动 frpc / 停止 frpc」，并广播 `rc://status`                                             |
| `src-tauri/Cargo.toml`                             | 新增依赖 `hmac`、`hex`、`windows-sys`（进程枚举）；bin 名改为 `Homeport`                                |
| `src-tauri/tauri.conf.json`                        | 产品名 `Homeport`，标识符 `com.nick0x01.homeport`                                                       |
| `src/features/remote-console/`                     | 新增视图与数据层（`RemoteConsoleView.tsx`、`api.ts`）                                                   |
| `src/lib/ipc.ts`                                   | 新增 `ipc.remoteConsole` 命名空间                                                                       |
| `src/types/models.ts`                              | 新增 `RcStatus`、`RcCloud`、`RcRustDesk`                                                                |
| `src/workbench/{ActivityBar,SideBar,layout-state}` | 新增 `remote` 活动视图入口                                                                              |
| `src/i18n/locales/{en,zh-CN}.ts`                   | 新增 `remote.*` 与 `activityBar.remote` 文案（双语同步）                                                |
| `scripts/check-conventions.mjs`                    | 顺带修复 Windows 路径 bug（`URL.pathname` → `fileURLToPath`），原版在 Windows 上必失败                  |
| `src/i18n/locales/parity.test.ts`                  | 允许清单补入 `IP`、`RustDesk`                                                                           |

## 面板内容

- **运行状态**：frpc 进程、PID、运行时长（从日志推算）、启动/停止按钮、掉线自动重启（看门狗）
- **服务器侧**（标签页卡片，两个页签）：
  - **现有配置**：frps 版本、在线客户端数、中转地址与端口、累计流量，以及结构化代理列表（名称 / 类型 / 在线状态），
    每个代理显示**端口映射**；每 60 秒自动刷新，也可手动刷新
  - **日志**：frpc 日志尾部，可用编辑器打开
- **云资源**：ECS 实例状态/规格/可用区/公网 IP、实时出/入带宽、本月出/入流量、估算流量费、账户余额、本月账单
- **RustDesk**：自建服务器地址与 Key，一键复制

停止 frpc 会弹出确认对话框，并说明后果——本机是服务提供方，停止后外部连接会全部断开。

所有子进程（`ssh`、`frpc`、`taskkill`）都以 `CREATE_NO_WINDOW` 启动，操作时不会闪出控制台窗口。

界面风格参考 [Clash Verge Rev](https://github.com/clash-verge-rev/clash-verge-rev) 的信息层级：
卡片带**色块图标标题**与右上角动作、指标以**瓷砖**呈现（色块图标 + 大数值 + 单位）、状态用**胶囊徽章**。
颜色全部取自 Sageport 主题令牌（`--success`/`--warning`/`--info`/`--danger`/`--primary`、`surface-raised`、`border-subtle` 等），
不写死色值，因此在浅色/深色及其他主题下均能正确适配。

> 附带修正：早期版本的状态圆点误用了并不存在的 `var(--status-success)` / `var(--status-danger)`，
> 现改用仓库既有写法（`bg-success` / `bg-destructive`，见 `workbench/tab-styles.ts`），错误文字改用对比度更好的 `text-danger`。

## 配置

**源码中不保留任何配置值**（除首次运行的默认值外）。全部 15 项设置存放在本地数据库的 `settings` 表，
键前缀 `remote.`：

| 键                                                          | 含义                                         |
| ----------------------------------------------------------- | -------------------------------------------- |
| `remote.frpcPath` / `remote.frpcConfigPath`                 | frpc 可执行文件与（提供方）配置文件          |
| `remote.visitorConfigPath`                                  | 访客配置文件，用于读取访问端口               |
| `remote.logPath`                                            | frpc 日志路径（用于运行时长与日志面板）      |
| `remote.relayHost` / `remote.sshUser` / `remote.sshKeyPath` | 中转服务器的 SSH 连接参数                    |
| `remote.dashboardUrl` / `remote.dashboardAuth`              | frps 面板地址与认证（`user:password`）       |
| `remote.portMap`                                            | 代理名到访问端口的映射（每行 `名称 = 端口`） |
| `remote.rustdeskDomain` / `remote.rustdeskKey`              | RustDesk 自建服务器参数                      |
| `remote.instanceId` / `remote.region` / `remote.pricePerGb` | 阿里云实例、地域与流量单价                   |

行为：

- 首次启动自动补齐**缺失的**键（`remote_console::ensure_config`），已有值一律不覆盖，因此版本升级新增设置项时无需手动配置
- **配置界面在「设置 → 远程访问」**（`features/settings/RemoteSection.tsx`），按 frpc / 中转服务器 / frps 面板 / RustDesk / 阿里云分成 5 组；
  字段**失焦即保存**（与设置页其它分区一致），保存时校验（必填项、frpc 路径须为 `.exe`、面板认证须含 `:`、单价范围、端口映射格式）
- 远程访问面板底部保留一个卡片，点「打开设置」可直接跳到该分区（`openSettings("remote")`）
- 面板认证密码、RustDesk Key 等敏感值同样存放于该数据库——与 Sageport 自身对主机凭据的处理方式一致
- 阿里云 AccessKey **不在**这些设置里，仍从环境变量或 `~\.aliyun\credentials.json` 读取

端口映射的取值来源（frps 面板只提供一半信息，另一半需本地补齐）：

| 显示项   | 来源                                                                     | 说明                                                                                 |
| -------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------ |
| 本机端口 | 本机 `frpcConfigPath` 的 `[[proxies]]` 段                                | frps 面板对 `stcp` 不返回 `localPort`，只能读本机配置                                |
| 中转端口 | frps `serverinfo` 的 `bindPort`                                          | 所有代理共用，显示在卡片上部                                                         |
| 访问端口 | `remote.portMap` 优先，其次 `remote.visitorConfigPath` 的 `[[visitors]]` | `stcp` 的访问端口属于访问方机器的 visitor，frps 完全不记录（0.69.1 无 visitor 端点） |

访问端口有两个来源，覆盖两个相反方向：

- **本机是访问方**（如本机连 server-4090）：端口来自本机访客配置的 `bindPort`
- **本机是提供方**（如 server-4090 连本机）：端口写在对面机器的访客配置里，本机看不到，只能用 `remote.portMap` 手填

> 两个设置项可以指向**同一个文件**。frp 客户端配置允许 `[[proxies]]` 与 `[[visitors]]` 并存，
> 因此一个 frpc 进程就能同时「对外提供服务」和「反向访问别人」——本机当前就是这么部署的
> （`frpcConfigPath` 与 `visitorConfigPath` 都是 `D:\Programs\frp\frpc.toml`）。
> 此时面板从同一份文件里读两侧端口，且「停止 frpc」只影响这一个进程。

解析访客配置时按 **`serverName`** 匹配，而不是 `[[visitors]]` 里自己的 `name`——frps 上报的是提供方的代理名
（例如配置里 `name = "s4090-ssh"` 而 `serverName = "server-4090-ssh"`，必须用后者）。这一点有单元测试守着。

因此由其它机器提供的代理（如 `server-4090-*`）只显示访问端口——本机配置里没有它的服务端口，不臆测。

辅助脚本：

```powershell
py -3 scripts/check-rc-settings.py   # 打印库中现有的 remote.* 配置
py -3 scripts/seed-rc-settings.py    # 一次性写入实际值（按需修改脚本内常量）
```

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

产物：`src-tauri\target\release\Homeport.exe`

若要复用其他 Sageport 检出的依赖缓存，可设置：

```powershell
$env:CARGO_TARGET_DIR = "<其他检出>\src-tauri\target"
```

## 运行

直接运行 exe 即可。数据目录按以下优先级解析（见 `src-tauri/src/paths.rs`）：

1. 环境变量 `HOMEPORT_DATA_DIR`
2. exe 同级的 `data` 目录（存在即为**便携模式**）
3. 系统应用数据目录（由标识符 `com.nick0x01.homeport` 决定）

因此本 fork 与原版 Sageport 的数据互不干扰。

## 依赖的外部条件

| 依赖                                     | 用途                         | 缺失时表现           |
| ---------------------------------------- | ---------------------------- | -------------------- |
| `D:\Programs\frp\frpc.exe` + `frpc.toml` | 启停对象                     | 启动报「未找到」     |
| `~\.aliyun\credentials.json`             | 云资源查询的 AccessKey       | 云资源区显示错误原因 |
| `~\.ssh\id_ed25519`                      | SSH 到中转服务器查 frps 面板 | 代理检查失败         |
| WebView2                                 | 渲染界面                     | Win11 自带           |

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

| 检查                                           | 结果                             |
| ---------------------------------------------- | -------------------------------- |
| `pnpm typecheck`                               | 通过                             |
| `pnpm format:check`                            | 通过                             |
| `pnpm check:conventions`                       | 通过                             |
| `pnpm lint`                                    | 通过                             |
| `pnpm test`                                    | 74 文件 / 514 用例全通过         |
| `cargo test --lib`                             | 185 通过 / **13 失败**           |
| `cargo test --lib remote_console`              | 10 个单元测试通过                |
| `cargo test --lib remote_console -- --ignored` | 真实调用阿里云接口验证签名，通过 |

那 13 个失败是**上游在 Windows 上的既有问题**，与本 fork 无关：原始检出跑同一套用例，失败数量与名单完全一致。
集中在 `sshkey`、`legacy`、`sync`（依赖 POSIX 文件权限）、`db`（含特殊字符的路径）、`tasks`、`ssh_config`。

## 许可

沿用上游 [GPL-3.0-only](LICENSE)。
