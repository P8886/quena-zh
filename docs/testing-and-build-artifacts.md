# 测试结果与构建产物记录

记录日期：2026-11-10（北京时间）。

测试完成后保留测试源码、命令、结果和版本记录。生成的测试可执行文件、
临时数据和废弃安装包中间文件在确认未被使用后清理；编译依赖按需复用。

## 本次磁盘清理

用户反馈项目目录约占 40.55 GB。排查时另一个任务仍在构建，实测逻辑大小
约 40.64 GB；清理后约 4.32 GB。磁盘实际可用空间增加约 29.5 GB。
以下 GB 使用十进制，GiB 使用二进制。

| 产物 | 本次处理 | 说明 |
| --- | --- | --- |
| `target/debug/` | 已清理，Cargo 汇总 27.1 GiB | 旧开发和测试产物，包含完整调试信息、增量缓存和多份依赖变体 |
| `target/local/incremental/` | 已清理，约 6.14 GB | 为加速局部重编译保存的状态；后续已关闭生成 |
| 7 个 `plugins/*/target/` | 已清理，约 1.07 GB | 独立插件项目的编译目录；保留源码和 `plugins/dist/` 中的打包资源 |
| `target/local/deps/` 中的 16 个测试可执行文件 | 已清理，约 168 MB | 保留测试源码和可复用的库，测试需要时可重新编译可执行文件 |
| 中断打包遗留的 `rw.*.dmg` | 已清理，约 100 MB | 已确认未挂载且没有进程占用 |
| `target/local/deps/` 和 `build/` | 保留，分别约 3.40 GB 和 0.55 GB | 可复用的 Rust 依赖、链接库、构建脚本和原生库编译结果 |
| 当前应用和安装包 | 保留 | `target/local/bundle/macos/Quena.app` 和 `target/local/bundle/dmg/Quena_0.2.1_aarch64.dmg` |
| 源码、锁文件、前端依赖和插件资源 | 保留 | 继续开发和打包所需；本次未清理安装应用和抓包数据 |

逻辑大小会重复计入硬链接文件。`du` 实际分配空间和磁盘可用空间的变化，
因此小于逻辑大小的减少量；不能将目录减少约 36 GB 直接当作磁盘释放量。

旧开发和插件目录使用 `cargo clean` 清理；剩余增量缓存、测试文件在持有
Cargo 构建锁后清理。处理临时 DMG 前还核对了挂载状态和文件占用。

## 已完成的功能测试

以下结果来自关联聊天“排查安装后无法打开”中的现有执行记录，
本次磁盘清理没有重新运行整套功能测试。

- 关联聊天 ID：`01a1b392-8f95-7083-b61a-d319c9bf9942`。
- 源码基准提交：`e81e6938b797710cb427c50e0e6d21256d5e1d11`，另含当时未提交的修复。
- 功能测试时工作区 diff 的 SHA-256：`c1436fe2076204e4cf7008b71affbea7226ecbe54e553586eab78ffe5c658d37`。
- 记录来源：该任务的命令输出及 r12 构建来源记录；其中标注运行时代码在测试后未变化。

| 验证项 | 命令或范围 | 已记录结果 |
| --- | --- | --- |
| 正文转发单元测试 | `cargo test -p quena-proxy --profile local --locked --offline --lib body::tests` | 8 项通过，0 失败；属于下方完整代理测试的子集 |
| 代理完整测试 | `cargo test -p quena-proxy --profile local --locked --offline` | 62 项通过 |
| 响应协议场景 | HTTP、HTTPS/HTTP/1.1、HTTPS/HTTP/2，共 21 个场景 | 已通过，包含空正文、HEAD、204、304 等完成状态判断 |
| 旧代码回归复现 | `cargo test -p quena-proxy --profile local --locked --offline --test e2e complete_and_bodyless_responses_are_not_marked_aborted -- --exact` | 旧代码按预期失败：正常 204 被标为 `Aborted`，预期为 `Done`；执行记录确认随后恢复修复代码 |

正文相关测试覆盖：空正文正常结束、已知长度收齐后没有再次轮询 EOF、
未知长度在真实 EOF 结束、提前断开仍标为中断、转发尾部字段，以及收齐正文后
尾部读取错误仍报错。保留这些测试源码用于后续回归。

## 本次配置验证

| 检查 | 结果 |
| --- | --- |
| `cargo metadata --format-version 1 --no-deps --locked --offline` | 通过，解析 18 个工作区包，输出目录仍为本项目 `target/` |
| `git diff --check -- Cargo.toml CONTRIBUTING.md` | 通过 |
| 清理后的目录核对 | 旧 `target/debug/`、增量缓存和插件编译目录已移除，当前应用和安装包仍存在 |
| 生成目录体积 | 当前 `target/` 约 4.1 GB，低于本地 6 GiB 管理预算 |

`Cargo.toml` 已将 `local` 和 `dev` 的增量编译关闭，并将 `dev` 的调试信息设为
`0`；默认测试配置继承 `dev`。后续优先复用已有的 `local` 依赖运行针对性测试，
避免同时保留多份完整依赖图。清理旧缓存后，首次需要的测试产物会按需重建。

## 后续保留规则

1. 保留源码、测试源码、锁文件、依赖缓存、当前可复用的编译库和当前交付包。
2. 测试结束后更新本文件中的日期、源码版本、命令、结果、失败原因和必要证据。
3. 清理用完的测试可执行文件、临时测试数据、失效增量缓存和中断打包文件。
4. 构建完成后检查 `target/` 体积，以 6 GiB 为本地管理预算；超过时检查并清理过期生成物。
   这项规则记录在 `AGENTS.md`，属于开发工作流程，不是后台定时清理程序。
5. 清理时使用 Cargo 清理命令或构建锁，确认打包文件未被使用，保留必要编译依赖。
6. 仅在需要定位原生崩溃时临时启用调试信息，完成后清理该配置的输出。


## 宽布局与原始报文分隔条（2026-11-10）

新增宽布局：会话列表在上方，下方请求、响应并排。设置、查看菜单和命令面板
均可选择；列表高度、请求与响应宽度以及原始视图中各自的头部／正文高度会保存。
切换布局保留选中的会话和未发送的编辑内容。

- 基准提交：`e81e6938b797710cb427c50e0e6d21256d5e1d11`，另含当前工作区未提交内容。
- 工作区 diff SHA-256（不含本记录文件）：`317947f959f83c3821809acf9c8a52a93180e411112f1895226940099374d34c`。
- 相关文件的 SHA-256：`/tmp/quena-wide-layout-source-20261110.json`，包含新拆出的
  `app/ui/src/components/Splitter.tsx`；单独记录该新文件，避免仅对比 tracked diff 时遗漏。

| 检查 | 结果 |
| --- | --- |
| `npm test --prefix app/ui -- src/store.test.ts src/actions.test.ts src/i18n/i18n.test.ts` | 33 项通过，覆盖布局恢复、旧设置兼容、布局切换、编辑内容保留和多语言完整性 |
| `npm run typecheck --prefix app/ui` | 通过 |
| `npm run build --prefix app/ui` | 通过 |
| `cargo check -p quena-app --profile local --locked --offline` | 通过，原生菜单与翻译编译检查完成 |
| 浏览器中的实际前端组件，示例请求及模拟 Tauri IPC | 列表在上、请求在左、响应在右；设置切换、外层两个分隔条、左右各自的原始头部／正文分隔条均已操作验证 |
| 最小窗口 `900 × 560` | 页面无整体溢出，两个详情面板仍并排；原始视图头部约 71/68 px、正文约 119/121 px，均保留可用空间 |
| 完整重载后恢复 | 宽布局、列表高度及两侧独立的原始视图比例恢复成功；请求约 0.374706，响应约 0.360232 |
| 构建目录 | `target/` 约 4.2 GiB，低于 6 GiB 预算；继续保留可复用依赖 |

日志：`/tmp/quena-wide-layout-tests.log`、`/tmp/quena-wide-ui-build.log`、
`/tmp/quena-wide-native-check.log`。界面尺寸与拖动结果：
`/tmp/quena-wide-layout-ui-checks.json`；截图：
`/tmp/quena-wide-layout-20261110.png`。

开发页在 16:08:40 的热更新历史日志中曾出现 React 更新深度错误；随后完整重载，
后续切换、拖动和恢复检查期间没有新增同类日志。最终截图取自重载后的页面。

本次未生成新的桌面安装包。保留正在供用户查看的预览实例与其临时脚本
`/tmp/quena-wide-preview-20261110.mjs`；本次 Rust 检查没有生成独立测试可执行文件。


## 会话类型图标与 Fiddler 分类对照（2026-11-10）

会话列表固定标记栏增加类型／状态图标，复用 Canvas 绘制，最多缓存 36 个
`Path2D`，不为每条抓包记录建立 DOM 节点或发起资源请求。标记栏由 22 px
加宽为 30 px，为图标右侧与会话编号留出间距。错误状态条、用户标记条保留。
断点暂停时仅显示红色（请求）或黄色（响应）圆点；放行后恢复图标，历史断点
标志不影响显示。F2 解锁后的开锁图标会随编辑状态更新。

以用户提供的 `gabrielxvx/zh-fiddler` 仓库内 Fiddler 程序反编译结果核对，
分类入口为 `frmViewer.method_2`、`updateSession`、F2 解锁处理与隧道握手解析。
本次只参考分类逻辑，图形使用与现有 Lucide 风格一致的矢量线条。

| Fiddler 分类／原图标编号 | 当前处理 |
| --- | --- |
| 普通请求 0/4，POST 32 | 普通 HTTP 图标；未知响应类型的 POST 使用提交图标，已知 JSON 等类型仍保留其类型图标 |
| 请求／响应断点 1/3 | 按用户要求以红／黄圆点代替类型图标 |
| 接收／发送响应 2 | 接收响应图标；等待响应使用时钟图标 |
| 图片 5、脚本 6、XML 7、HTML 8、CSS 9 | 均已覆盖；SVG 优先归为图片，XHTML 归为 HTML |
| 重定向 10 | 301/302/303/307/308 均已覆盖 |
| 304 缓存 11、206 分段 37 | 均已覆盖，并优先于 MIME 类型 |
| HTTP 错误 12、中断 14、401/403/407 认证 15 | 均已覆盖；中断与已收到的 HTTP 状态码分别显示 |
| 204/HEAD/OPTIONS 16 | 分别显示无正文、HEAD、OPTIONS 图标；补充 205 无正文 |
| CONNECT 13、WebSocket 34、SSE/RPC 39 | 均有对应图标；RPC 按 RPC_IN_DATA/RPC_OUT_DATA 方法识别 |
| 音频 28、视频 27、字体 29、JSON 33 | 均已覆盖；JSON 支持 +json、NDJSON，字体支持 WOFF/WOFF2 等 |
| Flash 30、Silverlight 31 | 仅提供类型图标识别，不增加执行运行时 |
| F2 解锁编辑 36 | 开锁图标；断点暂停的圆点仍优先 |
| 未解密隧道握手中识别 HTTP/2 40、SPDY 38 | 暂未细分；Quena 当前会话摘要没有对应握手识别数据，统一显示 CONNECT 隧道，不按 URL 或请求版本猜测 |
| Quena 补充 | gRPC、Protobuf、表单、PDF、压缩归档、纯文本、二进制类型 |

只在缺少 Content-Type 时由 URL 路径后缀推测；查询参数、实际已声明的 MIME、
错误与断点状态均优先，不因 URL 含 `image.png` 等参数而误分类。悬停提示提供
中、英、德文本，并标明 URL 推测。

- 基准提交：`e81e6938b797710cb427c50e0e6d21256d5e1d11`，另含当前未提交更改。
- 工作区 diff SHA-256（不含本记录）：`0343fda0c9bd1e71dc89a4778b20e3aa8ef36299a56eadd815994293257dfee4`。
- 相关文件／反编译来源指纹：`/tmp/quena-session-icons-source-20261110.json`。

| 检查 | 结果 |
| --- | --- |
| `npm test --prefix app/ui -- src/grid/sessionIcon.test.ts src/grid/style.test.ts src/i18n/i18n.test.ts` | 82 项通过，包含 MIME 与扩展名优先级、错误/断点/解锁优先级、恢复、未知类型和翻译完整性 |
| `npm run typecheck --prefix app/ui` | 通过 |
| `npm run build --prefix app/ui` | 通过；最终编译包含加宽后的标记栏及补齐的状态图标 |
| 浏览器预览 | 37 条模拟会话，已检查深色／浅色、类型与状态图标、独立断点圆点、30 px 标记栏以及中文悬停提示 |
| 构建目录 | `target/` 约 4.2 GiB，低于 6 GiB；此次前端检查未生成独立 Rust 测试可执行文件 |

日志：`/tmp/quena-session-icons-tests.log`、`/tmp/quena-session-icons-build.log`。
截图：`/tmp/quena-session-icons-20261110.png`、
`/tmp/quena-session-icons-light-20261110.png`。运行中的临时预览使用
`/tmp/quena-session-icons-preview-20261110.mjs`，保留供用户查看；示例通过模拟 IPC
提供，不连接真实抓包数据。该预览只用于会话列表／详情布局，其他功能的模拟
返回并不完整（例如智能体工具报告曾因模拟数据不全触发预览错误）。
本次未打包或替换已安装应用。

## 按功能拆分提交前的回归验证（2026-11-10）

本轮在拆分提交前重新运行前端测试及核心、存储、代理、TLS 测试。
运行时代码最终归档至 `b4e87c9f6fd4e38cb0d6a59f12836e6d674c983b`；
测试时已使用本次提交中的紧凑构建配置。拆分暂存区未修改工作区源文件。

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| 前端完整测试 | `npm test --prefix app/ui` | 30 个测试文件、225 项通过，0 失败 |
| Rust 相关工作区测试 | `cargo test -p quena-app-core -p quena-store -p quena-proxy -p quena-tls --profile local --locked --offline` | 374 项通过，0 失败，4 项按原配置忽略 |

Rust 命令使用现有工具链和依赖缓存：`CARGO_HOME=/tmp/quena-cargo`、
`RUSTUP_HOME=/tmp/quena-rustup`，并将 `/tmp/quena-cargo/bin` 加入 `PATH`。
测试包含真实断点阻塞与规则、TLS CBC 兼容、清空会话隔离及正文完成状态回归。
忽略项分别需要真实 AI 智能体运行、公开旧 TLS 域名、真实 WireMock 环境，
以及显式执行的性能计时测试；这些结果不表示对应外部场景已在本轮验证。

日志：`/tmp/quena-precommit-ui-tests.log`、
`/tmp/quena-precommit-rust-tests.log`。

测试后持有 `target/local/.cargo-lock` 的排他锁，仅清理本次 Rust 日志列出的
46 个独立测试可执行文件；逐个用 `lsof` 确认未被使用，共删除
734,953,152 字节（约 701 MiB），没有因占用而跳过的文件。
清理记录：`/tmp/quena-precommit-test-cleanup.json`。
`target/` 清理后约 4.3 GiB，低于 6 GiB 预算；保留可复用依赖和交付包。

## 中文修订 13 安装包（2026-11-10）

- 源码提交：`de019dd1859c6e11a982a2dc3dcec3f851124b76`；开始构建时工作区干净，
  包含本轮按功能拆分的全部 10 个提交。
- 版本：`0.2.1`，构建编号 `2026111013`，macOS Apple Silicon（arm64）。
- 构建复用现有工具链与 `target/local/`，关闭增量编译；前端类型检查、生产构建
  及原生编译、APP/DMG 打包均通过。本轮没有重复运行此前已通过的功能测试。

```sh
CARGO_HOME=/tmp/quena-cargo RUSTUP_HOME=/tmp/quena-rustup CARGO_INCREMENTAL=0 \
PATH=/tmp/quena-cargo/bin:$PATH npm exec --prefix app/ui -- tauri build \
  --bundles app,dmg --config /tmp/quena-zh-r13-package.json --ci \
  -- --profile local --locked --offline
```

交付文件：`/Users/jun/Downloads/Quena_0.2.1_zh-CN_r13_arm64.dmg`，
26,277,368 字节（约 25.06 MiB），SHA-256：
`3d8f5dca8850232f5c89fdea51a671a97b19f26d3c969273f1e7e0cbff999c1c`。

已通过磁盘映像校验、挂载后本地签名完整性检查、arm64 架构检查、
内嵌主程序与构建结果的哈希比对，以及 5 个随包插件资源检查。
使用临时应用副本和独立数据目录启动 10 秒，WebView 初始化成功并以状态码 0 退出；
关闭测试实例后清理临时副本并卸载校验映像。现有应用和抓包数据保留。
构建后 `target/` 约 4.3 GiB，低于 6 GiB 预算。

日志和来源记录：`/tmp/quena-zh-r13-package.log`、
`/tmp/quena-zh-r13-source.json`、`/tmp/quena-zh-r13-verify.json`、
`/tmp/quena-zh-r13-delivery.json`、`/tmp/quena-zh-r13-startup.log`。

## 查找高亮被失焦选中色遮挡（2026-11-10）

查找会自动选中所有匹配行；原绘制逻辑在列表失焦时仍用选中底色覆盖标记色，
因此用户需再点其他行取消批量选中，才能看到设置的高亮颜色。
现在失焦的已标记行保留标记配色；列表有焦点时继续使用选中配色，
未标记行的失焦选中色、匹配结果批量选择、错误状态条和断点圆点保持原逻辑。

- 基准提交：`de019dd1859c6e11a982a2dc3dcec3f851124b76`，另含本次未提交修复。
- 相关源码指纹：`/tmp/quena-find-highlight-source-20261110.json`。

| 检查 | 结果 |
| --- | --- |
| 修复前运行新增回归断言 | 7 项按预期失败，复现设置的标记色被 `selected-inactive` 覆盖；日志 `/tmp/quena-find-highlight-before-fix.log` |
| `npm test --prefix app/ui -- src/grid/style.test.ts src/grid/sessionIcon.test.ts src/actions.test.ts` | 3 个文件、98 项通过；覆盖六种标记色、活动／失焦选择、错误状态和请求／响应断点 |
| `npm run typecheck --prefix app/ui` | 通过 |
| 真实前端组件与模拟查找 IPC | 修复前在列表中复现浅蓝色覆盖金色；修复后重新加载并查找 `/api/`，弹窗关闭即显示金色，状态栏仍显示已选择 11 个，红／黄断点圆点仍可见，全程无需额外点击 |

截图：`/tmp/quena-find-highlight-fixed-20261110.jpg`。
模拟预览脚本：`/tmp/quena-find-highlight-preview-20261110.mjs`；验证后已停止本轮
临时预览服务，保留脚本用于复现。此次没有运行 Rust 构建或生成新的安装包，
没有新增独立测试可执行文件；当前 r13 交付包及已安装应用保留。

## 查找持续匹配后续流量（2026-11-10）

原查找只处理启动任务时的会话快照。现在颜色高亮默认开启“持续匹配新会话”，
初次查找结束后，由索引订阅合并发生变化的会话编号，后台按批检查，
不在代理转发线程或界面线程读取正文，也不反复扫描整个历史列表。
后续匹配沿用范围、大小写、正则、正文大小上限、压缩解码及字符集设置。
新匹配只高亮，不改变用户选中或正在编辑的会话；状态栏提供计数和停止入口。
停止保留已有标记，新查找替换旧条件；仅选择、仅选中会话和 MCP 查找保持单次任务。
清空或切换抓包后保留条件并重建编号绑定，旧任务不能给复用编号的新会话着色。

- 基准提交：`de019dd1859c6e11a982a2dc3dcec3f851124b76`，另含本次及上一轮未提交修复。
- 源码指纹：`/tmp/quena-continuous-find-source-20261110.json`，包含两个新增测试文件。
- Rust 检查复用 `/tmp/quena-cargo`、`/tmp/quena-rustup` 和当前 `target/local/`。

| 检查 | 结果 |
| --- | --- |
| `cargo test -p quena-index -p quena-store --profile local --locked --offline --lib` | 21 项通过，含订阅合并、分批读取和视图更新相互独立的回归 |
| `cargo test -p quena-app-core --profile local --locked --offline --lib --test live_find` | 核心 191 项、新增集成 7 项通过；包含真实 HTTP 代理的分块响应在初次查找后到达、大小写／正则、gzip／字符集／正文上限、600 条后续会话分批处理、筛选变更、MCP 隔离、停止／替换及清空／切换抓包 |
| `npm test --prefix app/ui -- src/i18n/i18n.test.ts src/grid/style.test.ts src/actions.test.ts` | 44 项通过 |
| `npm test --prefix app/ui -- src/components/FindDialog.test.tsx` | 5 项通过，验证默认开启、记住关闭、仅选择／仅选中会话，以及停止时保留选择和编辑内容 |
| `npm run typecheck --prefix app/ui` | 通过 |
| `cargo check -p quena-app --profile local --locked --offline` | 通过，包含新增原生命令注册和 MCP 编译检查 |
| 浏览器真实组件、模拟新捕获事件 | 初次匹配 11 条；追加 #38 匹配请求后立即高亮且计数变为 12，#39 普通请求无高亮，详情仍查看原选中的 #10。点击状态栏停止后，再追加同样匹配的 #40 请求不再高亮，#38 的原标记保留 |

界面预览截图：`/tmp/quena-continuous-find-options-20261110.jpg`、
`/tmp/quena-continuous-find-active-20261110.jpg`、
`/tmp/quena-continuous-find-stopped-20261110.jpg`。
模拟脚本 `/tmp/quena-continuous-find-preview-20261110.mjs` 仅提供测试数据，
验证后已停止服务。真实代理功能另由上述集成测试验证；本轮没有替换正在运行的应用。

日志：`/tmp/quena-continuous-find-index-store.log`、
`/tmp/quena-continuous-find-core-tests.log`、`/tmp/quena-continuous-find-ui-tests.log`、
`/tmp/quena-continuous-find-dialog-tests.log`、`/tmp/quena-continuous-find-ui-typecheck.log`、
`/tmp/quena-continuous-find-native-check.log`。

测试后持有 Cargo 构建锁并通过 `lsof` 确认未被使用，清理本次日志列出的
4 个独立测试可执行文件，共 53,446,336 字节。记录：
`/tmp/quena-continuous-find-test-cleanup.json`。清理后 `target/` 约 4.4 GiB，
低于 6 GiB 预算；保留依赖库、测试源码和 r13 交付包，本轮未生成新安装包。

## 非全屏窗口底部留白（2026-11-10）

宽布局将保存的 0～1 比例直接作为 `fr` 权重；详情区触发 240 px 最小高度后，
剩下的权重小于 1，CSS Grid 只分配部分剩余空间，导致窗口底部多出空白。
用当前保存的列表比例 `0.6956313775510204` 在 `1440 × 850` 渲染区域复现，
详情底边与状态栏之间相距约 159.49 px。

现在把两个权重按比例归一化，使每个非零权重至少为 1，最小高度触发后仍能用满
剩余空间。宽布局和原始报文头部／正文分隔复用同一计算，保留原有分隔比例和拖动保存逻辑。

- 基准提交：`de019dd1859c6e11a982a2dc3dcec3f851124b76`，另含前述未提交修复。
- 源码指纹：`/tmp/quena-window-layout-source-20261110.json`。

| 检查 | 结果 |
| --- | --- |
| `npm test --prefix app/ui -- src/store.test.ts src/actions.test.ts` | 24 项通过，布局恢复与切换相关回归通过 |
| `npm run typecheck --prefix app/ui` | 通过 |
| 真实前端组件与模拟 IPC，原保存比例 | `1440 × 850`、`1440 × 1000`、`1000 × 700`、`900 × 560` 下底部空白均为 0 px，详情区保持至少 240 px |
| 拖动列表／详情分隔条 | 底部继续填满；完整重载后分隔位置恢复一致 |
| 拖动请求原始报文分隔条后缩小至 `900 × 560` | 请求正文保留 60 px 最小高度，头部／分隔条／正文用满区域；两侧正文底边与原始视图底边之差小于 0.01 px |
| 完整重载 | 列表比例、请求与响应各自的原始报文比例均恢复一致，正文实际加载显示正常 |

尺寸、轨道和拖动记录：`/tmp/quena-window-layout-checks-20261110.json`。
截图：`/tmp/quena-window-layout-before-20261110.jpg`、
`/tmp/quena-window-layout-fixed-20261110.jpg`、
`/tmp/quena-window-layout-small-20261110.jpg`、
`/tmp/quena-window-layout-final-20261110.jpg`。
日志：`/tmp/quena-window-layout-tests.log`、`/tmp/quena-window-layout-typecheck.log`。

验证使用独立模拟数据，已恢复浏览器默认尺寸并停止本轮临时预览服务。
仅修改前端源码，没有生成 Rust 测试可执行文件或新的安装包，保留现有应用和交付包。

## 本轮拆分提交前验证（2026-11-11）

在提交前重新运行前端完整测试和生产构建：

| 命令 | 结果 |
| --- | --- |
| `npm test --prefix app/ui` | 31 个测试文件、236 项通过，0 失败 |
| `npm run build --prefix app/ui` | TypeScript 类型检查和 Vite 生产构建通过 |

核对查找高亮、持续匹配及窗口布局的源码指纹，运行时代码与前述已验证版本一致。
持续匹配涉及的 219 项 Rust 测试及原生命令编译检查沿用前述结果，本轮没有重复编译。
日志：`/tmp/quena-precommit-20261111-ui-tests.log`、
`/tmp/quena-precommit-20261111-ui-build.log`。本轮未生成独立 Rust 测试可执行文件或安装包。

## 插件 JSON 解析器与暗色样式去重（2026-11-11）

仅合并 GraphQL、JWT、WebDiag 的重复 JSON 解析器，以及系统／手动暗色的重复配色。
三个插件通过 `#[path]` 引用 `plugins/shared/json.rs`，保留各自的公开 `json` 模块；
共用文件与原三份 349 行源码的 SHA-256 完全一致，没有调整解析或格式化行为。
暗色的 76 项声明集中保留一份，主题应用逻辑将系统偏好解析为亮／暗色，
会话列表观察实际主题属性变化后刷新 Canvas。入口在语言和设置加载前应用系统主题。
实现代码净减少 775 行，不含本验证记录。

- 基准提交：`e22c12f09a31a3a208a018f4b864816148f8ad04`，另含本次未提交改动。
- 源码指纹：`/tmp/quena-dedup-20261111-source.json`，包含新共用文件及原文件删除状态。
- 改动前的配色与解析器指纹：`/tmp/quena-dedup-before-20261111.json`。

| 检查 | 结果 |
| --- | --- |
| 三个插件的 `cargo test` | 220 项通过、0 失败；沿用原有两个忽略项，分别是 50 万会话的耗时及 OAuth 压力测试 |
| 三个插件的 `cargo check --target wasm32-wasip2` | 全部通过，验证独立插件和 WIT 接口仍能引用共用源码 |
| `npm test --prefix app/ui` | 31 个文件、236 项通过，0 失败 |
| `npm run build --prefix app/ui` | TypeScript 类型检查和 Vite 生产构建通过 |
| 浏览器真实组件、模拟 IPC | 系统亮／暗与手动亮／暗／跟随系统共六种组合、八次状态切换；逐项对比原 CSS 的 80 个根变量及 `color-scheme`，结果一致，Canvas 背景同步更新 |
| `git diff --check` | 通过 |

插件是独立 Cargo 项目；本轮通过 `/tmp/quena-dedup-20261111-profiles.toml` 提供
与根项目完全相同的 `local` 配置，复用当前 `target/`、`/tmp/quena-cargo` 和
`/tmp/quena-rustup`，关闭增量编译，使用 `--locked --offline`。
测试与 WASM 检查的完整命令、结果及日志路径保存在
`/tmp/quena-dedup-20261111-rust-results.json`。
前端日志：`/tmp/quena-dedup-20261111-ui-tests.log`、
`/tmp/quena-dedup-20261111-ui-build.log`；界面结果：
`/tmp/quena-dedup-20261111-browser-results.json`，亮／暗截图使用同名前缀。
开发预览中出现过一次 `ResizeObserver loop` 通知；使用基准提交的四个前端源文件
替换预览输入后同样复现，记录在 `quena-dedup-20261111-original-*` 临时文件中，
本次未调整尺寸观察逻辑。亮暗配色及列表重绘检查均通过。

测试后持有 `target/local/.cargo-lock`，逐个通过 `lsof` 确认未被使用，清理
12 个独立测试可执行文件，共 27,093,504 字节；清理记录：
`/tmp/quena-dedup-20261111-cleanup.json`。`target/` 约 4.5 GiB，低于 6 GiB。
验证后的临时预览服务已停止；本轮未生成安装包，保留现有应用、插件资源及交付包。
