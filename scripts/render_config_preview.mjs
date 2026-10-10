import fs from 'node:fs';
import { execSync } from 'node:child_process';

const html = `<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8" />
  <title>Pivot - 配置服务商</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "PingFang SC", "Segoe UI", Roboto, sans-serif;
      background: #f8fafc;
      color: #0f172a;
      padding: 24px;
      display: flex;
      justify-content: center;
    }
    .container {
      width: 760px;
    }
    .top-nav {
      display: flex;
      align-items: center;
      justify-content: space-between;
      margin-bottom: 12px;
    }
    .back-btn {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 4px 10px;
      border-radius: 8px;
      font-size: 12px;
      color: #71717a;
      background: transparent;
      border: none;
      cursor: pointer;
    }
    .back-btn:hover {
      background: #f4f4f5;
      color: #18181b;
    }
    .confirm-pill {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 4px 10px;
      border-radius: 10px;
      background: #fff1f2;
      border: 1px solid #fecdd3;
      font-size: 12px;
      color: #be123c;
    }
    .btn-abandon {
      padding: 3px 8px;
      border-radius: 6px;
      font-size: 11px;
      font-weight: 600;
      color: white;
      background: #e11d48;
      border: none;
      cursor: pointer;
    }
    .btn-keep {
      padding: 3px 8px;
      border-radius: 6px;
      font-size: 11px;
      color: #71717a;
      background: white;
      border: 1px solid #e4e4e7;
      cursor: pointer;
    }
    .actions {
      display: flex;
      align-items: center;
      gap: 8px;
    }
    .btn-save {
      display: flex;
      align-items: center;
      gap: 6px;
      padding: 6px 14px;
      border-radius: 10px;
      font-size: 12px;
      font-weight: 600;
      color: white;
      background: #4f46e5;
      border: none;
      cursor: pointer;
    }
    .card {
      background: white;
      border: 1px solid #e4e4e7;
      border-radius: 16px;
      box-shadow: 0 1px 3px rgba(0,0,0,0.05);
      padding: 24px;
    }
    .header {
      padding-bottom: 16px;
      border-bottom: 1px solid #f4f4f5;
      margin-bottom: 20px;
    }
    .title {
      font-size: 14px;
      font-weight: 700;
      color: #09090b;
    }
    .subtitle {
      font-size: 12px;
      color: #71717a;
      margin-top: 2px;
    }
    .section-title {
      font-size: 12px;
      font-weight: 700;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      color: #18181b;
      margin-bottom: 14px;
    }
    .field {
      margin-bottom: 14px;
    }
    .label {
      display: block;
      font-size: 12px;
      font-weight: 500;
      color: #3f3f46;
      margin-bottom: 5px;
    }
    .input {
      width: 100%;
      padding: 8px 12px;
      font-size: 12px;
      font-family: ui-monospace, SFMono-Regular, monospace;
      border: 1px solid #e4e4e7;
      border-radius: 10px;
      outline: none;
    }
    .input:focus {
      border-color: #6366f1;
      box-shadow: 0 0 0 2px rgba(99, 102, 241, 0.15);
    }
    .row {
      display: flex;
      gap: 12px;
    }
    .row > div {
      flex: 1;
    }
    .model-box {
      margin-top: 20px;
      padding-top: 20px;
      border-top: 1px solid #f4f4f5;
    }
    .model-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 12px;
    }
    .btn-fetch {
      padding: 5px 10px;
      border-radius: 8px;
      font-size: 11px;
      color: #4f46e5;
      background: #eef2ff;
      border: none;
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 4px;
    }
    .footer {
      margin-top: 20px;
      padding-top: 14px;
      border-top: 1px solid #e4e4e7;
      display: flex;
      justify-content: flex-end;
      gap: 8px;
    }
    .btn-cancel {
      padding: 6px 14px;
      border-radius: 10px;
      font-size: 12px;
      color: #52525b;
      background: transparent;
      border: none;
      cursor: pointer;
    }
  </style>
</head>
<body>
  <div class="container">
    <div class="top-nav">
      <!-- 演示顶部智能内联确认状态（无论窗口高度多小均立即可见） -->
      <div class="confirm-pill">
        <span>存在未保存的修改，确认放弃？</span>
        <button class="btn-abandon">确认放弃</button>
        <button class="btn-keep">继续编辑</button>
      </div>
      <div class="actions">
        <button class="btn-save">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
            <polyline points="20 6 9 17 4 12"></polyline>
          </svg>
          添加并启用
        </button>
      </div>
    </div>

    <div class="card">
      <div class="header">
        <div class="title">配置 Anthropic</div>
        <div class="subtitle">配置 API Key、接入点及声明的模型池</div>
      </div>

      <div class="section-title">基础连接信息</div>
      <div class="field">
        <label class="label">服务商名称</label>
        <input class="input" value="Anthropic" />
      </div>

      <div class="row field">
        <div style="flex: 0.4;">
          <label class="label">协议格式</label>
          <input class="input" value="Anthropic" disabled style="background:#f4f4f5; color:#71717a;" />
        </div>
        <div style="flex: 0.6;">
          <label class="label">接口基地址 (Base URL)</label>
          <input class="input" value="https://api.anthropic.com/v1" />
        </div>
      </div>

      <div class="field">
        <div style="display:flex; justify-content:space-between; margin-bottom:5px;">
          <label class="label" style="margin:0;">API 密钥 (Key)</label>
          <span style="font-size:11px; color:#4f46e5; cursor:pointer;">+ 多密钥轮询</span>
        </div>
        <input class="input" type="password" placeholder="sk-ant-... / sk-...（可留空）" />
      </div>

      <div class="model-box">
        <div class="model-header">
          <div>
            <span style="font-size:12px; font-weight:700;">可用模型池</span>
            <span style="font-size:10px; padding:2px 6px; border-radius:4px; background:#f4f4f5; color:#71717a; margin-left:4px;">已选 0</span>
          </div>
          <button class="btn-fetch">从上游获取模型</button>
        </div>
        <div style="display:flex; gap:8px;">
          <input class="input" placeholder="手动输入模型 ID，如 claude-3-7-sonnet-20250219" style="flex:1;" />
          <button style="padding:6px 14px; border-radius:10px; font-size:11px; background:#f4f4f5; border:none; cursor:pointer; color:#71717a;">添加模型</button>
        </div>
      </div>

      <div class="footer">
        <button class="btn-cancel">取消</button>
        <button class="btn-save">添加并启用</button>
      </div>
    </div>
  </div>
</body>
</html>`;

fs.writeFileSync('/tmp/config_preview.html', html, 'utf8');

const outPath = '/Users/votan/.gemini/antigravity/brain/ccd1525c-a533-4780-990d-29e85ee0e9f6/clean_provider_config_view.png';
execSync(`"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --window-size=850,750 --screenshot="${outPath}" file:///tmp/config_preview.html`);
console.log('Saved:', outPath);
