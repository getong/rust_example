import { build } from 'esbuild';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
process.chdir(fileURLToPath(new URL('..', import.meta.url)));
const index = process.argv.indexOf('--outdir');
const outdir = index >= 0 ? process.argv[index + 1] : 'target/frontend';
if (!outdir) throw new Error('--outdir requires a directory');
await build({
  entryPoints: ['frontend/app.ts', 'frontend/demos.ts', 'frontend/studio.ts'],
  bundle: true, platform: 'browser', format: 'iife', target: 'es2022',
  outdir: resolve(outdir), sourcemap: 'inline',
  banner: { js: '// Generated from TypeScript. Do not edit.' },
});
