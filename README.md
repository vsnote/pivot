<div align="center">
  <img src="public/app-icon.png" width="96" height="96" alt="Pivot Logo" />
  <h1>Pivot</h1>
  <p><strong>面向 AI 智能体的本地轻量路由网关</strong></p>

  <p>
    <a href="https://github.com/vsnote/pivot/releases/latest">
      <img src="https://img.shields.io/github/v/release/vsnote/pivot?color=blue&label=Release" alt="Latest Release" />
    </a>
    <img src="https://img.shields.io/badge/Platform-macOS-lightgrey.svg" alt="Platform macOS" />
    <a href="LICENSE">
      <img src="https://img.shields.io/badge/License-MIT-green.svg" alt="License MIT" />
    </a>
  </p>
</div>

---

Pivot 是面向现代主流 AI Coding Agent 打造的本地轻量路由网关。在本机提供兼具 OpenAI 与 Anthropic 兼容的统一网关端点，支持多 Agent 配置无感接管、独立启动模型设定、协议即时转换与官方订阅池接入。

---

## ✨ 核心特性

- **⚡ 统一本地网关**：监听 `127.0.0.1:16888`，双向兼容 Anthropic 与 OpenAI 标准端点。
- **🔄 协议无损转换**：Anthropic ⇋ OpenAI ⇋ Gemini 实时互转，完整支持流式、Tool Call 与思维链。
- **🎯 精确透明路由**：按前缀与模型池严格路由，未声明模型直接报错，绝无跨模型静默篡改。
- **🔌 Agent 一键接管**：一键接管与无损还原 Agent 本地配置，支持独立设定启动模型。
- **👥 订阅账号池**：原生支持 Antigravity 与 ChatGPT 等官方订阅，免 Key 轮换与故障自愈。
- **🔑 负载均衡**：多 API Key 支持 Round-Robin 轮询与会话亲和 (Session Affinity)。
- **📊 统计监控**：高密度可视化看板，实时统计用量、Token 消耗、响应时延与日志链路。
- **🔒 本地与隐私**：配置与密钥全量存放在本机 SQLite，零网络遥测，支持中英文双语。

---

## 📥 下载与安装

前往 [GitHub Releases](https://github.com/vsnote/pivot/releases/latest) 下载 macOS 安装包：

| 芯片架构 | 适用机型 | 安装包 |
| :--- | :--- | :--- |
| **Apple Silicon** | M1 / M2 / M3 / M4 及后续芯片 Mac | `Pivot_*_aarch64.dmg` |
| **Intel** | Intel 处理器传统 Mac | `Pivot_*_x64.dmg` |

> 若遇到 macOS 拦截提示，可在终端执行解除隔离：  
> `sudo xattr -r -d com.apple.quarantine /Applications/Pivot.app`

---

## 🚀 快速上手

1. **添加服务商**：填入 API 密钥，或直接接入官方订阅账号池。
2. **选择启动模型**：为对应 Agent 独立指定偏好模型。
3. **开启接管**：一键开启代理，配置文件自动备份并重定向至本地网关。
4. **开始编码**：在终端正常运行开发命令即可。

---

## 🧭 路由机制

网关按以下优先级精确分发请求（未声明模型直接返回报错）：

1. **前缀路由**：显式指定前缀（如 `deepseek/deepseek-chat`）时直达目标服务商。
2. **精确匹配**：命中已启用服务商声明的模型池。
3. **智能归一化**：自动剥离日期后缀（`-20250219`）、`-latest` 或上下文标记（`[1M]`、`[thinking]`）。
4. **厂商家族启发**：同家族特定前缀（如 `deepseek-*`）由对应服务商承接。

---

## 🌐 支持的 Agent 与生态兼容

### 主流 AI Coding Agent

| Agent | 接管配置文件 | 原生协议 | 独立模型配置 |
| :--- | :--- | :---: | :---: |
| **Claude Code** | `~/.claude/settings.json` | Anthropic | ✅ |
| **Codex CLI** | `~/.codex/config.toml` | OpenAI | ✅ |
| **Gemini CLI** | 环境变量与端点代理 | Gemini | ✅ |
| **Pi Agent** | 本地开发代理 | OpenAI | ✅ |
| **OpenCode** | `opencode.json` | OpenAI | ✅ |

### 模型服务商与订阅池

- **官方订阅账号池 (免 API Key)**：Antigravity (Claude 3.7 / 3.5, Sonnet 4.6, Gemini 2.5)、ChatGPT Plus / Pro (Codex OAuth)、OpenCode Go。
- **主流商业模型**：Anthropic、OpenAI、Google Gemini、DeepSeek、xAI (Grok)、Mistral、MiniMax、智谱 AI、月之暗面等。
- **聚合与推理平台**：OpenRouter、Groq、SiliconFlow (硅基流动)、Cerebras、Together AI、Fireworks AI、GitHub Models 等。
- **本地部署与自建端点**：Ollama、LM Studio、vLLM 及任意兼容 OpenAI / Anthropic 标准的端点。

---

## 🛠️ 本地开发

```bash
# 启动前端与 Tauri 桌面端调试
pnpm install
pnpm tauri dev

# 运行测试与打包
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

---

## 💾 数据与存储

- 本地配置与日志存储于：`~/.pivot/pivot.db`
- 本地网关默认监听：`127.0.0.1:16888`
- 开启接管时会自动备份原始配置，随时可一键无损还原。

---

## 📄 开源协议

本项目采用 [MIT License](LICENSE) 开源协议。
