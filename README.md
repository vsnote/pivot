# OmniSwitch

OmniSwitch 是一个面向 Claude Code、Codex CLI 和 Gemini CLI 的本地 AI 路由网关。它在本机启动一个 OpenAI / Anthropic 兼容接口，将请求转发到已配置的服务商，并提供模型路由、fallback、协议转换、请求日志和 CLI 配置拦截。

## 当前能力

- 本地 Gateway：`http://127.0.0.1:16888`
- Anthropic API：`POST /v1/messages`
- OpenAI 兼容 API：`POST /v1/chat/completions`
- 模型发现：`GET /v1/models`
- 健康检查：`GET /health`
- OpenAI、Anthropic、Gemini、OpenRouter、Ollama 服务商
- 按模型、前缀、通配符或正则路由
- fallback 服务商和 API Key 会话亲和
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

1. 启动 OmniSwitch。
2. 在“服务商”页面添加服务商、API 地址、API Key 和模型。
3. 在“路由”页面配置模型匹配规则和 fallback。
4. 在“客户端”页面开启对应 CLI 的拦截。
5. CLI 的请求会通过 `127.0.0.1:16888` 转发到目标服务商。

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
