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

## 面板拖出为独立窗口

AI 助手（右侧 aux 面板）与文件（底部 SFTP 面板）都可以**拖出为独立窗口**，也可以随时**固定回主窗口**：

- 面板头部的「拖出到独立窗口」按钮（`PictureInPicture2` 图标）→ Rust 创建一个带自绘标题栏的独立窗口，只渲染该面板
- 拖出期间主窗口原位置显示占位（「该面板已在独立窗口中打开」+「固定回主窗口」按钮）
- 弹窗标题栏的 📌 按钮、占位上的按钮、或直接关闭弹窗，都会把面板装回主窗口

实现要点（`src-tauri/src/commands/window.rs` + `src/workbench/popout.ts` + `src/app/PopoutApp.tsx`）：

- **同一时刻每个面板只有一个活实例**（拖出后主窗口卸载该面板，装回时重新挂载自动拉新数据），因此**不需要跨窗口状态同步**
- 拖出状态以 **Rust 窗口管理器为准**（`get_webview_window(label)` 存在即已拖出），前端用 `popout://closed` 事件镜像，不落盘——重启应用后一切归位，没有残留状态
- 弹窗识别靠 `getCurrentWindow().label`，不依赖 URL 参数；主题、缩放、i18n、react-query 通过复用 `AppProviders` 自动生效
- AI 面板在有任务运行时禁用拖出（运行中的审批对话框不会藏到看不见的地方）

## 面板内容

面板顶部是一个三段式切换（`SegmentedControl`），按能力拆成三个 Tab，避免把不同生命周期的东西塞进一个滚动条：

| Tab            | 内容                                                                                                                                                                                                         |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **隧道与穿透** | frpc 运行状态（PID / 运行时长 / 启停 / 看门狗）+ 服务器侧卡片：frps 版本、在线客户端数、中转地址与端口、累计流量、结构化代理列表（名称 / 类型 / 在线状态 / 端口映射），卡片内再分「现有配置 / 日志」两个页签 |
| **RustDesk**   | 自建服务器地址、中继与 Key，一键复制完整配置块                                                                                                                                                               |
| **云资源**     | ECS 实例状态/规格/可用区/公网 IP、实时出/入带宽、本月出/入流量、估算流量费、账户余额、本月账单                                                                                                               |

底部固定一张「配置」卡片，点「打开设置」跳到设置页对应分区。

- 服务器侧数据每 60 秒自动刷新，也可手动刷新；面板右上角刷新按钮会一次性刷新**当前面板涉及的所有查询**
- 停止 frpc 会弹出确认对话框，并说明后果——本机是服务提供方，停止后外部连接会全部断开
- 所有子进程（`ssh`、`frpc`、`taskkill`）都以 `CREATE_NO_WINDOW` 启动，操作时不会闪出控制台窗口

界面风格参考 [Clash Verge Rev](https://github.com/clash-verge-rev/clash-verge-rev) 的信息层级：
卡片带**色块图标标题**与右上角动作、指标以**瓷砖**呈现（色块图标 + 大数值 + 单位）、状态用**胶囊徽章**。
颜色全部取自 Sageport 主题令牌（`--success`/`--warning`/`--info`/`--danger`/`--primary`、`surface-raised`、`border-subtle` 等），
不写死色值，因此在浅色/深色及其他主题下均能正确适配。

> 附带修正：早期版本的状态圆点误用了并不存在的 `var(--status-success)` / `var(--status-danger)`，
> 现改用仓库既有写法（`bg-success` / `bg-destructive`，见 `workbench/tab-styles.ts`），错误文字改用对比度更好的 `text-danger`。

### 代码组织

```
features/remote-console/
  RemoteConsoleView.tsx   壳：三段式切换 + 配置卡片 + 统一刷新 + 托盘状态事件
  tabs/FrpTab.tsx         隧道与穿透（frpc 状态、服务器侧、日志）
  tabs/RustDeskTab.tsx    RustDesk
  tabs/CloudTab.tsx       云资源
  parts.tsx               纯展示组件（Card / CardTitle / StatTile / StatusDot / ProxyBadge / RefreshButton）
  helpers.ts              无组件依赖的工具函数与常量（端口映射、格式化、复制）
  api.ts                  react-query 封装
```

`parts.tsx` 只导出组件、`helpers.ts` 只导出函数与常量，是为了满足仓库的
`react-refresh/only-export-components` 规则（混合导出会产生 lint 警告，基线是 0 警告）。

### AI 助手集成

AI 助手原先"看不到"这个面板，现已补上两处：

1. **上下文感知**：`features/ai/runner.ts` 的 `buildContext()` 现在会注入当前侧栏视图
   （`Active sidebar view: remote.`），并提示助手该视图对应的能力；用户在远程访问面板提问时不会答非所问
2. **工具接入**：新增 `features/ai/tools/remote.ts`，在「设置 → AI → 工具」中作为独立的 **Remote access** 分组：

| 工具                  | 作用                                                                                                   | 是否需批准                       |
| --------------------- | ------------------------------------------------------------------------------------------------------ | -------------------------------- |
| `get_remote_status`   | frpc 进程状态（PID/运行时长/看门狗）+ 中转服务器（版本/客户端数/流量）+ 每条隧道（状态、端口、连接数） | 否                               |
| `get_rustdesk_config` | 自建 RustDesk 服务器地址、中继与公钥                                                                   | 否                               |
| `get_cloud_status`    | ECS 实例状态/规格/公网 IP/实时带宽/本月流量/估算费用/余额/账单                                         | 否                               |
| `control_frpc`        | 启动、停止 frpc，或切换看门狗                                                                          | **是**（停止会切断所有外部连接） |

只读工具直接返回 JSON，便于模型精确引用；`control_frpc` 需要用户批准，并在系统提示词中明确
"停止 frpc 会切断所有外部连接，属于风险操作"。

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

- **本机是访问方**（如本机连 server-4090）：端口来自本机访客配置的 `bindPort`，此时 `accessLocal` 为 `true`，
  面板才会提供「复制连接命令」与「连接远程桌面」（端口确实在本机监听，连得上）
- **本机是提供方**（如 server-4090 连本机）：端口写在对面机器的访客配置里，本机看不到，只能用 `remote.portMap` 手填，
  `accessLocal` 为 `false`，只提供复制、不提供直连

> `remote.portMap` **只应填「端口位于对方机器」的代理**。若把本机 visitor 的代理也写进去，虽然数值可能相同，
> 但以后改了 `bindPort` 就会出现数值漂移（面板显示旧端口）。判断依据：该名字是否出现在 `visitorConfigPath` 的 `[[visitors]]` 里。

> 两个设置项可以指向**同一个文件**。frp 客户端配置允许 `[[proxies]]` 与 `[[visitors]]` 并存，
> 因此一个 frpc 进程就能同时「对外提供服务」和「反向访问别人」——本机当前就是这么部署的
> （`frpcConfigPath` 与 `visitorConfigPath` 都是 `D:\Programs\frp\frpc.toml`）。
> 此时面板从同一份文件里读两侧端口，且「停止 frpc」只影响这一个进程。

解析访客配置时按 **`serverName`** 匹配，而不是 `[[visitors]]` 里自己的 `name`——frps 上报的是提供方的代理名
（例如配置里 `name = "s4090-ssh"` 而 `serverName = "server-4090-ssh"`，必须用后者）。这一点有单元测试守着。

因此由其它机器提供的代理（如 `server-4090-*`）只显示访问端口——本机配置里没有它的服务端口，不臆测。

### 右键菜单

面板各区域都有右键菜单，写法与仓库既有区域（主机、SFTP、终端等）一致：图标 + 文案、分隔线分组、
危险项用 `destructive`、复制统一弹 toast。

| 区域                                   | 菜单项                                                                                                                | 所属 Tab  |
| -------------------------------------- | --------------------------------------------------------------------------------------------------------------------- | --------- |
| 运行状态卡片                           | 启动/停止 frpc（**按状态互斥**，停止为危险色）、切换看门狗、复制 PID、打开日志、在文件夹中显示、打开设置              | 隧道      |
| 代理行                                 | 复制代理名、复制访问端口、复制端口映射、复制连接命令、连接远程桌面 / 加入主机列表（按类型互斥，仅本机可访问时）、刷新 | 隧道      |
| 中转行 / 流量行                        | 复制中转地址 / 复制流量数值                                                                                           | 隧道      |
| 指标瓷砖（服务器 2 个 + 云资源 10 个） | 复制值（自动由数值+单位派生，`—` 的瓷砖不提供）                                                                       | 隧道 / 云 |
| 公网 IP 行                             | 复制公网 IP                                                                                                           | 云        |
| RustDesk 卡片                          | 复制服务器地址、复制 Key、复制完整配置                                                                                | RustDesk  |
| 日志区域                               | 复制全部日志、刷新日志、用记事本打开、在文件夹中显示                                                                  | 隧道      |
| 配置卡片                               | 打开设置                                                                                                              | 常驻      |

两条设计取舍：

- **不做嵌套菜单**：Radix 的 `ContextMenu` 嵌套时内外层都会响应，因此菜单只挂在「条目」上
  （代理行、瓷砖、行）与**没有子菜单的卡片**上，不给服务器/云资源卡片整体加菜单
- **敏感值不进菜单**：面板认证口令不提供复制项，避免误复制外泄；RustDesk Key 保留（卡片上本就有整体复制按钮）

### 全局默认菜单的处理

WebView2 自带浏览器右键菜单（返回 / 刷新 / 另存为 / 打印 / 更多工具），在没有自定义菜单的区域
（列表空白、设置页、表单）会直接冒出来，与桌面应用不符。

`main.tsx` 里全局监听 `contextmenu` 并 `preventDefault()`，**但输入框/文本域例外**：

```ts
if (isEditableTarget(event.target)) return; // input / textarea / contenteditable 放行
event.preventDefault();
```

- 放行编辑区是为了保留系统菜单自带的**剪切 / 复制 / 粘贴 / 全选**（受控输入框自己实现粘贴要绕过
  React 的 value tracker，成本与风险都不低）
- Radix 触发器自身也会 `preventDefault`，与全局处理不冲突；仓库内没有依赖默认菜单行为的处理器

> 曾尝试给 `Input` / `Textarea` 加自带菜单，被仓库的打包预算检查拦下：
> `scripts/check-bundle.mjs` 禁止懒加载特性块（Radix 菜单）泄漏进首屏，而这两个原语被
> `components/ui/index.ts` 静态引入。因此改为「编辑区放行系统菜单」这一零体积方案。

「连接远程桌面」走的是窄接口后端命令 `rc_open_rdp(port)`（只接受端口号，内部拼 `mstsc /v:127.0.0.1:<port>`），
不暴露「执行任意命令」的能力。SSH 则只提供**复制命令**——SSH 需要用户名，而用户名不在任何本机配置里，
直接启动会因为默认用户名不对而失败，不如复制出来让用户自己补。

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
