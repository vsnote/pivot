import assert from 'node:assert/strict';
import { readFile, access } from 'node:fs/promises';
import { test } from 'node:test';
import { execFileSync } from 'node:child_process';

test('Brand Kit 桌面图标资源文件完整且符合 1024x1024 高清规范', async () => {
  await assert.doesNotReject(() => access(new URL('../public/app-icon-light.png', import.meta.url)));
  await assert.doesNotReject(() => access(new URL('../public/app-icon-dark.png', import.meta.url)));
  await assert.doesNotReject(() => access(new URL('../public/app-icon.png', import.meta.url)));
});

test('浅色模式桌面图标为白底黑图案，深色模式桌面图标为黑底白图案', () => {
  const pyCode = `
from PIL import Image
light = Image.open('public/app-icon-light.png')
dark = Image.open('public/app-icon-dark.png')

assert light.size == (1024, 1024), f"Light size mismatch: {light.size}"
assert dark.size == (1024, 1024), f"Dark size mismatch: {dark.size}"

# Check squircle interior background (at 250, 250)
light_bg = light.getpixel((250, 250))
dark_bg = dark.getpixel((250, 250))

assert light_bg[0] > 240 and light_bg[1] > 240 and light_bg[2] > 240, f"Light bg not white: {light_bg}"
assert dark_bg[0] < 35 and dark_bg[1] < 35 and dark_bg[2] < 35, f"Dark bg not black: {dark_bg}"

# Check motif pixels
# Motif at (534, 464)
light_motif = light.getpixel((534, 464))
dark_motif = dark.getpixel((534, 464))

assert light_motif[0] < 50 and light_motif[1] < 50 and light_motif[2] < 50, f"Light motif not black: {light_motif}"
assert dark_motif[0] > 240 and dark_motif[1] > 240 and dark_motif[2] > 240, f"Dark motif not white: {dark_motif}"
print("OK")
`;
  const result = execFileSync('python3', ['-c', pyCode], { encoding: 'utf8' });
  assert.equal(result.trim(), 'OK');
});

test('PivotLogo 组件在 variant="icon" 下适配浅色/深色模式桌面图标', async () => {
  const code = await readFile(
    new URL('../src/components/common/PivotLogo.tsx', import.meta.url),
    'utf8',
  );
  assert.ok(code.includes('/app-icon-light.png'), '必须包含浅色模式图标 /app-icon-light.png');
  assert.ok(code.includes('/app-icon-dark.png'), '必须包含深色模式图标 /app-icon-dark.png');
  assert.ok(code.includes('dark:hidden'), '浅色模式图标需带有 dark:hidden');
  assert.ok(code.includes('hidden dark:block'), '深色模式图标需带有 hidden dark:block');
});

test('index.html 支持系统深浅色自适应 Favicon', async () => {
  const html = await readFile(
    new URL('../index.html', import.meta.url),
    'utf8',
  );
  assert.ok(html.includes('href="/app-icon-light.png"'), '应声明浅色模式 favicon');
  assert.ok(html.includes('href="/app-icon-dark.png"'), '应声明深色模式 favicon');
});
