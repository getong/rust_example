import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
process.chdir(fileURLToPath(new URL('..', import.meta.url)));
const index = process.argv.indexOf('--outdir');
const outdir = index >= 0 ? process.argv[index + 1] : 'target/frontend';
if (!outdir) throw new Error('--outdir requires a directory');
const result = spawnSync('bun', [
  'build', 'frontend/app.ts', 'frontend/demos.ts', 'frontend/studio.ts',
  '--target=browser', '--format=iife', '--sourcemap=inline',
  '--entry-naming=[name].js', '--outdir', resolve(outdir),
  '--banner', '// Generated from TypeScript. Do not edit.',
], { stdio: 'inherit' });
if (result.error) throw result.error;
if (result.status !== 0) throw new Error(`Bun bundling failed (${result.signal ?? result.status})`);
