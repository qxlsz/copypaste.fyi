#!/usr/bin/env node
const { spawnSync } = require('child_process');

if (process.env.VERCEL === '1') {
  console.log('[prepare] Detected Vercel build environment, skipping Playwright install');
  process.exit(0);
}

const aptCheck = spawnSync('apt-get', ['--version'], { stdio: 'ignore' });
const hasAptGet = !aptCheck.error && aptCheck.status === 0;
const args = ['playwright', 'install'];
if (hasAptGet) {
  args.push('--with-deps');
}

const command = process.platform === 'win32' ? 'npx.cmd' : 'npx';
const result = spawnSync(command, args, { stdio: 'inherit' });

if (result.status !== 0) {
  const code = result.status ?? 1;
  console.error(`[prepare] Playwright install failed with exit code ${code}`);
  process.exit(code);
}
