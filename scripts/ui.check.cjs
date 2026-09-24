// Run with Vite on :1420 and Playwright available via NODE_PATH.
// IPC is mocked in the test browser only; no R2 credentials or remote writes.
// Add --readme to refresh the public/assets feature screenshots with demo data.
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');

(async () => {
  const browser = await chromium.launch({ channel: 'msedge', headless: true });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  await page.addInitScript(() => {
    const files = ['README.md', 'manifest.json', 'index.html', 'styles.css', 'release-notes.txt', 'backup.zip'].map((name, i) => ({
      name, key: name, size: (i + 1) * 2048, lastModified: '2026-09-24T08:30:00Z', etag: 'test', storageClass: 'STANDARD',
    }));
    const images = Array.from({ length: 4 }, (_, i) => ({
      name: `example-${i + 1}.png`, key: `images/example-${i + 1}.png`, size: 2048,
      lastModified: '2026-09-24T08:30:00Z', etag: 'example', storageClass: 'STANDARD',
    }));
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (cmd, args = {}) => {
        if (cmd.startsWith('plugin:event|')) return 1;
        if (cmd === 'list_profiles') return [{ id: 'demo', name: 'Development', accountId: 'a'.repeat(32), accessKeyId: 'test', defaultBucket: 'project-assets', readOnly: false, createdAt: '' }];
        if (cmd === 'list_buckets') return ['project-assets', 'archive'];
        if (cmd === 'list_objects' && args.prefix === 'images/') return {
          folders: [], files: images, folderCount: 0, fileCount: images.length,
          totalSize: images.length * 2048, truncated: false, nextToken: null,
        };
        if (cmd === 'list_objects') return {
          folders: args.prefix ? [] : ['documents', 'images', 'releases'].map(name => ({ name, prefix: name + '/' })),
          files: args.prefix ? [] : files, folderCount: args.prefix ? 0 : 3, fileCount: args.prefix ? 0 : files.length,
          totalSize: 43008, truncated: false, nextToken: null,
        };
        if (cmd === 'folder_stats') return { folders: 2, files: 12, size: 1048576, scannedAt: '2026-09-24T08:30:00Z' };
        if (cmd === 'head_object') return { size: 2048, contentType: args.key.endsWith('.png') ? 'image/png' : 'text/plain', lastModified: '2026-09-24T08:30:00Z', metadata: {} };
        if (cmd === 'read_image') {
          const canvas = document.createElement('canvas');
          canvas.width = 640;
          canvas.height = 480;
          const ctx = canvas.getContext('2d');
          ctx.fillStyle = '#deded8';
          ctx.fillRect(0, 0, 640, 480);
          ctx.fillStyle = '#303236';
          ctx.textAlign = 'center';
          ctx.font = '28px sans-serif';
          ctx.fillText('Example image', 320, 220);
          ctx.font = '18px sans-serif';
          ctx.fillText(args.key.split('/').pop(), 320, 262);
          return canvas.toDataURL('image/png');
        }
        if (cmd === 'read_text') return { text: '# Project assets\n\nShared files for the development environment.\n\n## Contents\n\n- documents/\n- images/\n- releases/\n', size: 2048, truncated: false };
        if (cmd === 'search_objects') return { files: files.filter(f => f.name.includes(args.query)), scanned: files.length, truncated: false };
        throw new Error('Unexpected test IPC: ' + cmd);
      },
    };
  });
  const shots = '.harness-ui-screenshots';
  fs.mkdirSync(shots, { recursive: true });
  try {
    await page.goto('http://localhost:1420');
    await page.locator('.row').first().waitFor();
    assert.equal(await page.locator('.row').count(), 9);
    await page.screenshot({ path: `${shots}/desktop.png` });
    await page.getByLabel('Filter this folder').fill('manifest');
    assert.equal(await page.locator('.row').count(), 1);
    await page.getByLabel('Filter this folder').fill('');
    await page.locator('.row').filter({ hasText: 'README.md' }).click();
    assert.equal(await page.locator('.row.sel').count(), 1);
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    await page.locator('.source').waitFor();
    await page.screenshot({ path: `${shots}/preview.png` });
    await page.getByRole('button', { name: 'Close preview' }).click();
    await page.getByLabel('Gallery view').click();
    assert.equal(await page.locator('.tile').count(), 9);
    await page.screenshot({ path: `${shots}/gallery.png` });
    await page.getByLabel('Details view').click();
    await page.getByRole('button', { name: 'Search bucket', exact: true }).click();
    await page.locator('#bucket-search').fill('manifest');
    await page.locator('.bucket-search').getByRole('button', { name: 'Search', exact: true }).click();
    await page.waitForFunction(() => document.querySelectorAll('.row').length === 1);
    await page.locator('.bucket-search').getByRole('button', { name: 'Close', exact: true }).click();
    await page.waitForFunction(() => document.querySelectorAll('.row').length === 9);
    await page.getByLabel('New tab', { exact: true }).click();
    assert.equal(await page.locator('.tab').count(), 2);
    await page.getByLabel('Close tab', { exact: true }).last().click();
    assert.equal(await page.locator('.tab').count(), 1);
    await page.locator('summary').filter({ hasText: 'New' }).focus();
    await page.keyboard.press('Enter');
    await page.getByRole('button', { name: 'New folder', exact: true }).click();
    assert.equal(await page.locator('dialog.prompt[open]').count(), 1);
    await page.getByRole('button', { name: 'Cancel', exact: true }).click();
    await page.locator('.row').filter({ hasText: 'documents' }).dblclick();
    await page.waitForFunction(() => document.querySelectorAll('.row').length === 0);
    await page.getByLabel('Back', { exact: true }).click();
    await page.waitForFunction(() => document.querySelectorAll('.row').length === 9);
    for (const width of [320, 375, 414, 768, 1280]) {
      await page.setViewportSize({ width, height: 800 });
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `Page overflow at ${width}`);
      await page.screenshot({ path: `${shots}/${width}.png` });
      await page.getByRole('button', { name: 'Connections', exact: true }).click();
      await page.locator('dialog.sheet[open]').waitFor();
      const box = await page.locator('dialog.sheet[open]').boundingBox();
      assert(box.x >= 0 && box.x + box.width <= width, `Dialog overflow at ${width}`);
      await page.screenshot({ path: `${shots}/connections-${width}.png` });
      await page.locator('dialog.sheet[open]').getByRole('button', { name: 'Close', exact: true }).click();
    }
    await page.locator('.row').filter({ hasText: 'images' }).dblclick();
    await page.waitForFunction(() => document.querySelectorAll('.row').length === 4);
    await page.getByLabel('Gallery view').click();
    await page.locator('.tile').first().dblclick();
    await page.locator('.shot img').waitFor();
    await page.waitForFunction(() => [...document.querySelectorAll('.art img, .shot img')].every(img => img.complete && img.naturalWidth > 0));
    await page.screenshot({ path: `${shots}/gallery_preview.png` });
    assert.deepEqual(errors, []);
    if (process.argv.includes('--readme')) {
      fs.mkdirSync('public/assets', { recursive: true });
      for (const [source, dest] of [['preview', 'preview'], ['gallery', 'gallery'], ['gallery_preview', 'gallery_preview'], ['connections-1280', '1280']]) {
        fs.copyFileSync(`${shots}/${source}.png`, `public/assets/${dest}.png`);
      }
    }
    console.log('UI check passed: navigation, tabs, filter, search, gallery, preview, prompt, connections, 5 viewport widths; no page errors.');
  } finally {
    await browser.close();
  }
})().catch(e => { console.error(e); process.exitCode = 1; });
