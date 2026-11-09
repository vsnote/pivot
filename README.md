<div align="center">
  <img src="public/app-icon.png" width="96" height="96" alt="Pivot Logo" />
  <h1>Pivot</h1>
  <p><strong>面向 AI Coding 终端工具的本地轻量路由网关与客户端接管中心</strong></p>

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

Pivot 是面向现代主流 AI Coding Agent 与终端命令行工具打造的本地轻量路由网关与客户端接管中心。它在本地启动一个兼容 OpenAI 与 Anthropic 标准的高性能网关服务，提供多 Agent 配置无感接管、启动模型独立设定、请求精确透明分发、协议双向无损转换以及官方订阅账号池负载均衡。

---

## ✨ 核心特性

- **⚡ 统一本地网关**：在本机启动高性能接口服务（默认 `http://127.0.0.1:16888`），同时提供 Anthropic (`POST /v1/messages`) 与 OpenAI (`POST /v1/chat/completions`) 双标准端点。
- **🔄 协议双向无损转换**：支持 Anthropic ⇋ OpenAI ⇋ Gemini 多协议即时无损互转，完整保留流式传输 (SSE)、工具调用 (Tool Call) 与思维链 (Reasoning/Thinking)。
- **🎯 精确透明路由，绝无静默篡改**：网关严格按照请求模型精确分发、归一化智能匹配（自动兼容版本日期与上下文标记）与前缀路由（`provider/model`）；若无任何服务商支持该模型，直接明确报错，绝不进行静默跨模型降级或篡改。
- **🔌 多 Agent 一键无感接管**：一键接管主流 AI Coding 终端工具的本地配置；支持在界面中为各 Agent 独立指定启动模型，关闭接管时自动无损恢复原始配置。
- **👥 官方订阅账号池 (免 API Key)**：原生支持主流 AI 官方订阅账号接入与多账号池管理，具备并发轮换、Token 自动刷新、故障自愈与官方配额看板。
- **🔑 多 Key 负载均衡与会话亲和**：单服务商支持配置多个 API Key，采用 Round-Robin 轮询或 Session Affinity（按会话亲和锁定 Key）进行分发。
- **📊 高密度使用统计看板**：提供全宽比例图与聚合分析，支持按客户端、服务商、模型等多维度统计请求次数、Token 消耗、响应时延与多维排序。
- **🌐 国际化与纯本地优先**：原生支持中英文界面无缝切换；所有密钥凭证、配置与请求日志均保存在本机 SQLite 数据库，无任何数据上传云端，零网络遥测。

---

## 📥 下载与安装

请前往 [GitHub Releases](https://github.com/vsnote/pivot/releases/latest) 下载最新的 macOS 安装包：

| 芯片架构 | 适用机型 | 安装包下载 |
| :--- | :--- | :--- |
| **Apple Silicon** | M1 / M2 / M3 / M4 及后续芯片 Mac | `Pivot_*_aarch64.dmg` |
| **Intel** | 采用 Intel 处理器的传统 Mac | `Pivot_*_x64.dmg` |

> **💡 首次打开提示「已损坏」或「无法打开」？**  
> 由于开源分发未内置苹果商业付费证书，macOS Gatekeeper 可能会弹出拦截提示。可通过以下方式解除隔离：
> 1. 打开系统「设置」->「隐私与安全性」，在安全性部分找到提示并点击「仍要打开」。  
> 2. 或在终端中执行：
>    ```bash
>    sudo xattr -r -d com.apple.quarantine /Applications/Pivot.app
>    ```

---

## 🚀 快速上手

1. **添加服务商或订阅账号**：在「服务商」页面选择预设（如 Antigravity、ChatGPT、Anthropic、DeepSeek、OpenAI、Gemini、OpenRouter 等）填入 API 密钥，或直接接入官方订阅账号池。
2. **配置 Agent 启动模型**：在「客户端」页面为各个 Agent 独立选择专属的服务商和启动模型。
3. **开启 Agent 接管**：点击对应 Agent 的开关，Pivot 会自动备份原有配置并将流量无感代理至本地网关。
4. **尽情编码**：在终端正常使用开发命令（如 `claude`、`codex` 等），所有请求将在本机通过 Pivot 智能路由并实时记录。

---

## 🧭 模型路由逻辑

Pivot 遵循透明、明确、无静默篡改的路由准则，按以下优先级推导并路由每个请求：

1. **`provider/model` 前缀路由**：显式指定服务商名称或 ID 时（如 `deepseek/deepseek-chat` 或 `anthropic/claude-3-7-sonnet`），直接转发至目标服务商。
2. **精确模型匹配**：若请求的模型名称已在某个启用服务商的模型池列表中，自动路由至该服务商；多服务商具有同名模型时按服务商列表优先级排序承接。
3. **智能归一化匹配**：自动兼容剥离日期后缀（如 `-20250219`）、`-latest` 后缀或方括号上下文标记（如 `[1M]`、`[thinking]`）后的模型名称。
4. **厂商家族启发匹配**：针对厂商特定前缀模型（如 `deepseek-*`），自动由已启用的同家族服务商承接。
5. **明确报错（无静默降级）**：当请求的模型未在任何启用的服务商中声明时，网关直接返回清晰明确的错误信息，绝不静默改写或跨模型降级。

### 负载均衡与容灾自愈

- **多 API Key 调度**：同一服务商配置多个 API Key 时，支持 Round-Robin 轮询或基于 Session Affinity 的会话亲和绑定。
- **订阅账号池容灾**：官方订阅账号池支持健康检测与故障自愈，遇限流或认证异常时自动隔离并无缝重试可用账号。

---

## 🌐 支持的 Agent 与生态兼容

### 主流 AI Coding Agent 支持

| Agent / 客户端 | 接管机制 | 独立启动模型 | 协议与特性 |
| :--- | :--- | :---: | :--- |
| **Claude Code** | 全局与本地设置重定向 (`settings.json`) | ✅ | 原生 Anthropic 协议、流式输出与 Tool Call |
| **Codex CLI** | 配置重定向与模型映射 (`config.toml`) | ✅ | 原生 OpenAI 协议、Thinking 等级映射 |
| **Gemini CLI** | 环境变量与端点代理接管 | ✅ | 双向协议实时转换与流式 SSE |
| **Pi Agent** | 本地开发配置无感代理 | ✅ | 轻量代码助手无缝接管 |
| **OpenCode** | 运行配置与模型池同步 (`opencode.json`) | ✅ | 官方模型池与端点智能映射 |

### 官方订阅账号池 (免 API Key)

- **Antigravity**：支持多账号 Google 官方订阅池接入，聚合调用 Claude 3.7 / 3.5、Sonnet 4.6 与 Gemini 2.5。
- **ChatGPT Plus / Pro**：通过 Codex OAuth 接入，直接消耗个人订阅配额，支持多账号轮换与额度看板。
- **OpenCode Go / Plus**：官方订阅聚合模型池，支持 DeepSeek、Kimi、GLM、Qwen 等开源及前沿代码模型。

### 支持的模型服务商

- **主流商业模型**：Anthropic、OpenAI、Google Gemini、DeepSeek、xAI (Grok)、Mistral AI、Perplexity、Moonshot AI、智谱 AI、通义千问、MiniMax、StepFun 等。
- **聚合平台与推理加速**：OpenRouter、Groq、SiliconFlow (硅基流动)、Cerebras、Together AI、Fireworks AI、GitHub Models、Cloudflare Workers AI 等。
- **本地部署与自建端点**：Ollama、LM Studio、vLLM 及任意兼容 OpenAI / Anthropic 标准规范的反代端点。

---

## 🛠️ 本地开发

### 环境要求
- Node.js 20+
- pnpm 10+
- Rust stable
- macOS (含 Xcode Command Line Tools)

### 安装与启动

```bash
# 1. 安装前端依赖
pnpm install

# 2. 启动前端 Vite 开发服务器
pnpm dev

# 3. 启动桌面端开发调试 (Tauri)
pnpm tauri dev
```

### 测试与构建

```bash
# 前端测试与打包
pnpm test
pnpm build

# Rust 后端检查与单元测试
cargo test --manifest-path src-tauri/Cargo.toml

# 本地打包 macOS 安装包
pnpm tauri build
```

---

## 💾 数据与存储

- 本地配置与日志存储路径：
  ```text
  ~/.pivot/pivot.db
  ```
- 网关服务默认监听：`127.0.0.1:16888`
- 客户端接管配置时，原有配置文件会自动备份，关闭开关即可一键无损还原。

---

## 📄 开源协议

本项目采用 [MIT License](LICENSE) 开源协议。
