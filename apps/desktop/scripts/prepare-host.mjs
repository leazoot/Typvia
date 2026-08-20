// Builds the browser-extension native-messaging host and stages it where
// tauri's externalBin bundling expects it: src-tauri/binaries/
// typvia-browser-host-<target-triple>[.exe]. Runs from beforeBuildCommand,
// so every `tauri build` ships the host inside the app bundle (the desktop
// app resolves it as a sibling of its own executable).
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const workspaceRoot = join(here, '..', '..', '..');

function hostTriple() {
  // tauri v2 exports the build target; fall back to the rustc host triple.
  const fromEnv = process.env.TAURI_ENV_TARGET_TRIPLE;
  if (fromEnv !== undefined && fromEnv !== '') return fromEnv;
  const verbose = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
  const line = verbose.split('\n').find((entry) => entry.startsWith('host: '));
  if (line === undefined) throw new Error('cannot determine the target triple');
  return line.slice('host: '.length).trim();
}

execFileSync('cargo', ['build', '--release', '-p', 'typvia-browser-host'], {
  cwd: workspaceRoot,
  stdio: 'inherit',
});

const triple = hostTriple();
const extension = triple.includes('windows') ? '.exe' : '';
const built = join(workspaceRoot, 'target', 'release', `typvia-browser-host${extension}`);
const outDir = join(here, '..', 'src-tauri', 'binaries');
mkdirSync(outDir, { recursive: true });
copyFileSync(built, join(outDir, `typvia-browser-host-${triple}${extension}`));
console.log(`staged host for ${triple}`);
