---
title: MCP 配置与权限
description: 在本机 MCP 客户端中注册 Plotium，以及每次工具调用都遵守的规则。
---

本组页面面向编写自定义 MCP 客户端、或需要核对单个工具调用行为的读者。
要连接 Codex、Claude Code 或 Claude Desktop 并用日常语言使用 Plotium，请看
[连接 AI 客户端](/zh-cn/ai/connect/)。

| 页面 | 内容 |
| --- | --- |
| 本页 | 手动注册、权限、所有计划共同遵守的规则、工具 annotations |
| [状态与发现](/zh-cn/reference/mcp/discovery/) | `plotium_get_state`、能力发现、文档结构、属性目录 |
| [数据导入与表格](/zh-cn/reference/mcp/tables/) | 原生与 CSV/TSV/XLSX 导入、表格读取、派生、单元格补丁、刷新 |
| [建图与曲线](/zh-cn/reference/mcp/plots/) | `plot.create`、追加和重绑曲线 |
| [样式修改](/zh-cn/reference/mcp/styling/) | 显示属性、批量写入、手动设置的值、图例、主题 |
| [页面布局与删除](/zh-cn/reference/mcp/layout/) | `figure.edit`：页面、面板、文字、图片、标注和删除 |
| [预览、诊断与恢复](/zh-cn/reference/mcp/review/) | 画布与候选预览、图面诊断、恢复动作、错误码 |
| [保存与导出](/zh-cn/reference/mcp/save-and-export/) | `project.save`、`figure.export` |
| [科学运算](/zh-cn/reference/mcp/scientific/) | 处理、扩散、DOSY、任务以及结果建图 |

## 手动配置与诊断

Windows 上，Plotium 从 `PATH` 查找独立的客户端可执行文件。对于通过 npm 安装的
Codex，会在 `PATH` 中的 npm 前缀和默认的 `%APPDATA%\npm` 前缀内查找包中的原生
可执行文件，支持 x64 和 ARM64 安装。使用自定义 npm 前缀时，请将它加入 `PATH`
并重启 Plotium，或通过 **Choose executable…** 选择包内的 `codex.exe`。
添加 Codex 客户端需要先安装 Codex CLI。

在 **Preferences → AI connections (MCP)** 中展开 **Manual configuration and diagnostics**，
点击 **Copy stable stdio configuration**。复制的 JSON 含实际连接器绝对路径；
下例中的路径仅为占位符：

```json
{"mcpServers":{"plotium":{"command":"/ABSOLUTE/USER/DATA/bin/plotium-mcp","args":[]}}}
```

请使用页面复制的真实路径，Windows 文件名为 `plotium-mcp.exe`。启用时连接器会
复制到用户数据目录，应用升级时更新。发行安装目录中请将 `plotium-mcp` 保留在主
程序旁。官方 CLI 注册形式为：

```sh
codex mcp add plotium -- /ABSOLUTE/USER/DATA/bin/plotium-mcp
claude mcp add --transport stdio --scope user plotium -- /ABSOLUTE/USER/DATA/bin/plotium-mcp
```

参见 [Codex MCP](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)、
[Claude Code MCP](https://code.claude.com/docs/en/mcp) 和
[MCP Bundle manifest](https://github.com/modelcontextprotocol/mcpb/blob/main/MANIFEST.md)。
临时 HTTP 诊断信息包含密钥，服务重启后失效，仅用于高级诊断，不是保存的客户端
配置。发现文件只面向当前用户和本机；不支持远程或 WSL 跨环境访问。

升级 Plotium 后，请重启 AI 客户端中的 Plotium MCP 连接，以加载更新后的工具。
如果连接器安装失败，请打开 **AI connections (MCP)** 查看错误并点击 **Retry**。

## 权限与支持范围

支持的修改使用 Plotium 的 **Undo** 与 **Redo**。只读立即阻止后续修改执行，包括
待执行计划；表格读取及属性预检仍可用，导入、原生曲线绑定、创建及处理／补丁／刷新预检要求读取和
修改权限。权限会记住，直到手动更改。

项目查询及计划创建须同时提交 `expected_revision` 和 `expected_project_context`，
后者取自 `plotium_get_state` 的 `project_context`；各页示例用 `PROJECT_CONTEXT`
占位。每次打开、新建、关闭或恢复项目都会更换上下文，即使 revision 相同也不可
混用。切换清空计划并拒绝旧队列工作；切换期间的 `busy` 表示需等待完成或取消。
收到 `project_changed` 后重新读取状态、发现目标并规划，不沿用旧目标或计划。
执行计划绑定创建时的上下文。

项目编辑、科学计算和数据变换保留持久运行溯源，使项目进入未保存状态。普通查询（状态、
资源／属性／表格／文档浏览、科学结果查看、预览及诊断）不追加项目审计记录，也不改变
dirty、edit generation 或 automation revision。纯查询工作流同样不改变项目；包含修改
操作的工作流仍保留完整运行记录，包括复现操作所需的查询输入。握手、工具发现、
已认证状态查询、每五秒心跳、静态属性目录以及保存协调（预检、提交与
`plotium_save_status`），以及导出预检／提交／状态查询均不生成审计记录。

连接支持的操作见上表各页。[`plotium_properties_list`](/zh-cn/reference/mcp/discovery/#不依赖名称发现结构) 列出可写的属性及完整 schema；
[设置 ID](/zh-cn/reference/settings-ids/#图形) 列出面板中各图形设置对应的 ID。`object.axes.y2_*` 属性只在图中有曲线的
`series.y_axis` 为 `right` 时适用。应用偏好、新格式解析、科学结果自动建图及内置
聊天不在此接口范围内。

## 所有计划共同遵守的规则

工具结果同时提供结构化内容及 JSON 文本。项目读取与预检要求当前
`expected_revision`；需要动态值、桌面编辑或撤销后重新获取状态。已知精确目标时可按
[状态与发现](/zh-cn/reference/mcp/discovery/#读取状态与按需发现能力)中的规则复用同步提交结果。执行时重新检查保存的 revision
和当前权限。计划仅可提交一次，失败后也需重新预检；最多保留 32 个待执行计划，
重启连接会清空计划。桌面开始执行前已取消的请求不会修改项目；已进入原子
提交的修改无法通过取消回滚，请使用 **Undo**。

## 工具 annotations 与客户端审批

每个工具都在 `tools/list` 中提供 MCP
`annotations`，不随当前 **Access** 设置变化。`readOnlyHint` 表示是否改变环境；
`destructiveHint` 区分可能破坏性写入与仅追加操作；`idempotentHint` 表示相同参数的
重复调用是否产生额外影响；`openWorldHint` 表示是否与外部实体交互，包括本地输入／
输出文件。破坏性和幂等提示仅对非只读工具有意义。参见
[MCP 注解定义](https://modelcontextprotocol.io/specification/2025-11-25/schema#toolannotations)。

查询、预览和回执查看均为只读，仅访问原生项目／应用状态。诊断也是只读，但续查游标
仅可使用一次。`plotium_plan` 不是只读：每次成功调用都会保留新的可执行计划，部分
操作会读取外部数据、schema、处理方案或图片文件，并准备计算；它不提交候选结果，
也不写出保存／导出目标文件。`plotium_execute` 覆盖编辑、删除、科学任务提交及保存／
导出请求，因此整个工具保守声明为可能破坏性、非幂等、开放交互域。任务取消会改变任务
状态，属于可能破坏性写操作；对同一保留任务重复取消不会产生额外影响。

客户端可以依据策略使用可信服务端的提示来决定是否询问审批，但仍可能询问；不可信
注解不能作为授权依据。客户端审批、Plotium **Access** 与桌面文件确认是独立决定。
注解不授予权限：revision／context 检查、撤权与一次性计划规则仍然生效。每次保存／导出
请求仍保留独立桌面确认及准确的文件／覆盖范围，包括 **Read only** 模式。断连或结果
未知后绝不重放调用，即使标为幂等；应查询状态或保留回执。升级 Plotium 及其连接器后，
重新连接客户端以刷新工具目录。
