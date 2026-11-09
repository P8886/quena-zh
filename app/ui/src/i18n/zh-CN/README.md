# 简体中文用语约定

有歧义时先查看调用位置、数据来源和实际行为，再确定译文。协议、字段名、命令、
文件路径及可执行示例保留原文；不能只按单词字面含义翻译。

| 原文 | 译法与含义 | 核对位置 |
| --- | --- | --- |
| `{size} free` | 可用空间，指磁盘剩余容量 | `components/StatusBar.tsx` |
| `Ask` | 每次询问，指导入已有会话列表时的处理方式 | `components/Dialogs.tsx` 的 `OptionsDialog` / `ImportExistingDialog` |
| `Run to Completion` | 继续执行，放行当前断点会话 | `inspectors/Tamper.tsx` 的 `TamperBar` |
| `Break on Response` | 在响应时暂停，先发送请求，再拦截响应 | `TamperBar` 的 `breakOnResponse` 动作 |
| `Respond {status}` | 直接返回指定状态码 | `TamperBar` 的 `respond` 动作 |
| `Times each` / parallel | 每条请求的重发次数 / 并发上限 | `components/Dialogs.tsx` 的 `ReplayDialog` |
| `as sent` | 跟随客户端，沿用请求的 HTTP/HTTPS 协议 | `components/HostRemapDialog.tsx` |
| `keep host` | 保留原主机信息，包括 Host 头与 TLS 服务器名称 | `HostRemapDialog.tsx` |
| `hit` / cache miss | 缓存命中 / 缓存未命中 | `components/Dialogs.tsx` 的 `AgentCacheOptions`、`panels/Agents.tsx` |
| `drop` | 导入提示中指拖放；流量规则中指丢弃请求或消息 | 对应文件导入、规则脚本的调用位置 |
| `Profile` | 诊断页指分析方案；网络估算项指网络参数 | `panels/Diagnostics.tsx`、`plugins/webdiag/src/profiles.rs` |
| `Sequence (clock) duration` | 整体耗时，首个请求开始到最后响应结束 | `panels/Statistics.tsx` 的 `elapsed` |
| `Aggregate Session time` | 各会话耗时总和，包含并发会话的重叠时间 | `panels/Statistics.tsx` 的 `aggregateMs` |
| `Sanitize` | 脱敏，按规则替换或移除敏感内容 | `components/SanitizeDialog.tsx` |

统一保留 Cookie、SOAP、JWT、Markdown、Content-Length、Authorization 等技术名称。
认证 token 使用“令牌”；大模型计量单位使用 token；AI agent 使用“智能体”，
网络 proxy 使用“代理”。常规 HTTP 请求 replay 使用“重发”，冻结对话的缓存复现使用“重放”。

`i18n.test.ts` 检查覆盖率、占位符、重复词条和可执行示例；`format.test.ts`
保留磁盘空间、导入选项、Cookie 等歧义词的回归检查。后端断点描述在
`lib/breakpointText.ts` 中转换，命令条件保持原文。
