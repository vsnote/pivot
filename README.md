# OmniSwitch

OmniSwitch 是一个面向 Claude Code、Codex CLI 和 Gemini CLI 的本地 AI 路由网关。它在本机启动一个 OpenAI / Anthropic 兼容接口，将请求转发到已配置的服务商，并提供模型路由、fallback、协议转换、请求日志和 CLI 配置拦截。

## 当前能力

- 本地 Gateway：`http://127.0.0.1:16888`
- Anthropic API：`POST /v1/messages`
- OpenAI 兼容 API：`POST /v1/chat/completions`
- 模型发现：`GET /v1/models`
- 健康检查：`GET /health`
- OpenAI、Anthropic、Gemini、OpenRouter、Ollama 服务商
- 自动模型路由：前缀、精确模型名、默认服务商与默认模型，无需手写规则
- 备用链自动跳过不支持该模型的服务商，日志保留每次尝试
- 规则作为高级覆盖项保留，API Key 会话亲和
- Anthropic / OpenAI / Gemini 协议转换
- 非流式请求 Token 统计和请求日志
- Claude Code、Codex CLI、Gemini CLI 配置备份、拦截和恢复

## 开发环境

推荐使用：

- Node.js 20+
- pnpm
- Rust stable
- macOS 开发需要 Xcode Command Line Tools 或完整 Xcode
- VS Code + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

安装依赖：

```bash
pnpm install
```

启动前端开发服务器：

```bash
pnpm dev
```

启动 Tauri 桌面应用：

```bash
pnpm tauri dev
```

## 测试与构建

前端测试和构建：

```bash
pnpm test
pnpm run build
```

Rust 测试：

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

如果 macOS 上 Cargo 错误地使用了损坏的 Command Line Tools SDK，可以显式指定 Xcode 工具链：

```bash
CC="$(xcrun --find clang)" \\
CXX="$(xcrun --find clang++)" \\
SDKROOT="$(xcrun --sdk macosx --show-sdk-path)" \\
cargo test --manifest-path src-tauri/Cargo.toml
```

模拟 Claude Code 请求（需要先启动桌面应用）：

```bash
pnpm run test:cli
```

## 使用流程

1. 启动 OmniSwitch。首次启动没有任何内置服务商，全部由你添加。
2. 在“服务商”页面从预设或自定义添加服务商、API 地址、API Key 和模型。
3. 指定路由行为：给某家服务商填写「默认模型」并打开「设为默认服务商」，
   请求里没有服务商声明的模型名（例如 Claude Code 的模型名）就会改写成该模型。
4. 在“客户端”页面开启对应 CLI 的拦截。
5. CLI 的请求会通过 `127.0.0.1:16888` 转发到目标服务商。

模型名解析顺序（全部由服务商配置推导，不需要规则）：

1. `provider/model` 前缀：直接指定服务商。
2. “高级设置”里启用的规则：需要强制改写模型名或固定服务商时才添加。
3. 精确模型名：匹配到声明该模型名的服务商。
4. 默认服务商：请求里的模型名没有服务商声明时，改写成它的「默认模型」。

备用链会跳过无法提供该模型名的服务商：先按优先级尝试声明了同名模型的服务商，
再尝试用自己「默认模型」接手的服务商，每个候选都携带自己的模型名。
请求日志里可以看到完整的尝试链（服务商、模型、状态、错误摘要），失败时不再只留最后一条错误。

删除服务商会同时移除以它为目标的规则，并从其他规则的备用列表里去掉它，
避免留下永远不会生效的配置。

所有配置和请求日志保存在本机：

```text
~/.omniswitch/omniswitch.db
```

开启 CLI 拦截前，OmniSwitch 会在对应配置目录写入备份文件；关闭拦截会优先恢复首次开启时的原始配置。

## 注意事项

- API Key 保存在本机 SQLite 数据库中，请勿将数据库文件提交到代码仓库。
- 默认 Gateway 端口是 `16888`。如果端口被占用，桌面端会显示 Gateway 未运行，需要关闭占用进程后重启。
- 当前流式协议转换已经覆盖 OpenAI → Anthropic 和 Gemini → Anthropic 的事件格式；流式 Token 统计仍待进一步补齐。
- 真实服务商请求需要用户自行提供有效 API Key，项目不会内置任何密钥。
