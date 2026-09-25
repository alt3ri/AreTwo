const assert = require('node:assert/strict');
const fs = require('node:fs');

const app = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8'));
const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
const cargo = fs.readFileSync('src-tauri/Cargo.toml', 'utf8');
const crateVersion = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];

assert.equal(pkg.version, app.version, 'package.json and Tauri versions must match');
assert.equal(crateVersion, app.version, 'Cargo.toml and Tauri versions must match');
assert.equal(process.env.RELEASE_TAG, `v${app.version}`, 'Tag must be v followed by the app version');
console.log(`Release version checked: v${app.version}`);
