#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');

function run(cmd, options = {}) {
  console.log(`> ${cmd}`);
  return execSync(cmd, { cwd: rootDir, stdio: 'inherit', ...options });
}

function parseSemver(ver) {
  const match = ver.match(/^(\d+)\.(\d+)\.(\d+)(?:-([\w.-]+))?$/);
  if (!match) {
    throw new Error(`无法解析语义化版本: ${ver}`);
  }
  return {
    major: parseInt(match[1], 10),
    minor: parseInt(match[2], 10),
    patch: parseInt(match[3], 10),
    prerelease: match[4] || '',
  };
}

function bumpVersion(currentVersion, bumpType) {
  const parsed = parseSemver(currentVersion);
  if (bumpType === 'major') {
    return `${parsed.major + 1}.0.0`;
  }
  if (bumpType === 'minor') {
    return `${parsed.major}.${parsed.minor + 1}.0`;
  }
  if (bumpType === 'patch') {
    return `${parsed.major}.${parsed.minor}.${parsed.patch + 1}`;
  }
  // 如果直接传入了特定版本号（如 0.2.0 或 v0.2.0）
  const cleanVer = bumpType.replace(/^v/, '');
  parseSemver(cleanVer); // 校验格式
  return cleanVer;
}

function main() {
  const rawArgs = process.argv.slice(2);
  const flags = new Set(rawArgs.filter((arg) => arg.startsWith('--')));
  const nonFlagArgs = rawArgs.filter((arg) => !arg.startsWith('--'));

  const bumpType = nonFlagArgs[0] || 'patch';
  const noPush = flags.has('--no-push');
  const dryRun = flags.has('--dry-run');

  const pkgPath = path.join(rootDir, 'package.json');
  const tauriConfPath = path.join(rootDir, 'src-tauri', 'tauri.conf.json');
  const cargoTomlPath = path.join(rootDir, 'src-tauri', 'Cargo.toml');

  const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
  const oldVersion = pkg.version;
  const newVersion = bumpVersion(oldVersion, bumpType);
  const tagName = `v${newVersion}`;

  console.log(`\n🚀 准备发布版本: ${oldVersion} -> ${newVersion} (${tagName})`);

  if (dryRun) {
    console.log('[dry-run] 将同步更新以下文件并创建 tag:');
    console.log(`- ${pkgPath}`);
    console.log(`- ${tauriConfPath}`);
    console.log(`- ${cargoTomlPath}`);
    return;
  }

  // 1. 更新 package.json
  pkg.version = newVersion;
  fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
  console.log(`✔ 更新 package.json -> ${newVersion}`);

  // 2. 更新 tauri.conf.json
  const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));
  tauriConf.version = newVersion;
  fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n');
  console.log(`✔ 更新 tauri.conf.json -> ${newVersion}`);

  // 3. 更新 Cargo.toml
  let cargoToml = fs.readFileSync(cargoTomlPath, 'utf8');
  cargoToml = cargoToml.replace(/^version = ".*?"/m, `version = "${newVersion}"`);
  fs.writeFileSync(cargoTomlPath, cargoToml);
  console.log(`✔ 更新 Cargo.toml -> ${newVersion}`);

  // 4. 刷新 Cargo.lock
  try {
    console.log('✔ 正在刷新 Cargo.lock...');
    run('cargo check --manifest-path src-tauri/Cargo.toml', { stdio: 'ignore' });
  } catch (e) {
    // 忽略非关键错误
  }

  // 5. Git 提交与打 Tag
  try {
    run(`git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock`);
    run(`git commit -m "chore(release): ${tagName}"`);
    run(`git tag ${tagName}`);
    console.log(`✔ 已创建 Git 提交与标签: ${tagName}`);

    if (!noPush) {
      console.log(`\n📤 正在推送至 GitHub 触发自动发布流水线...`);
      run(`git push origin HEAD --follow-tags`);
      console.log(`\n🎉 推送成功！GitHub Actions 正在打包 macOS 安装包并生成发布说明。`);
    } else {
      console.log(`\n💡 已打标签（未推送）。你可以稍后执行: git push origin HEAD --follow-tags`);
    }
  } catch (e) {
    console.error('Git 操作失败:', e.message);
  }
}

main();
