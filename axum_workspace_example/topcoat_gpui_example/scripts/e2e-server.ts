import { spawn, execFileSync } from 'node:child_process';
import { join } from 'node:path';
let binary = process.env.TOPCOAT_E2E_SERVER;
if (!binary) {
  execFileSync('cargo', ['build', '-p', 'topcoat_gpui_example'], {stdio: 'inherit'});
  const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--no-deps', '--format-version', '1'], {encoding: 'utf8'})) as {target_directory: string};
  binary = join(metadata.target_directory, 'debug', 'topcoat_gpui_example');
}
const child = spawn(binary, [], {stdio: 'inherit', env: process.env});
for (const signal of ['SIGINT', 'SIGTERM'] as const) process.on(signal, () => child.kill(signal));
child.on('exit', code => process.exit(code ?? 0));
