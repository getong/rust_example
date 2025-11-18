import { copyFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';

const TARGET_DIR = join(__dirname, '..', '..', '..', 'target');

const CANDIDATE_FILES = [
  'napi_example.node',
  'napi_example.dll',
  'napi_example.so',
  'napi_example.dylib',
  'libnapi_example.so',
  'libnapi_example.dylib',
];

function ensureNodeExtension(originalPath: string): string {
  if (originalPath.endsWith('.node')) {
    return originalPath;
  }

  const dir = dirname(originalPath);
  const nodePath = join(dir, 'napi_example.node');
  if (!existsSync(nodePath)) {
    copyFileSync(originalPath, nodePath);
  }
  return nodePath;
}

function findNativeBinary(): string {
  const buildDirs = ['debug', 'release'];
  for (const build of buildDirs) {
    const dir = join(TARGET_DIR, build);
    for (const file of CANDIDATE_FILES) {
      const candidate = join(dir, file);
      if (existsSync(candidate)) {
        return ensureNodeExtension(candidate);
      }
    }
  }

  throw new Error(
    'Unable to locate the napi_example native binary. Please run `cargo build --release` first.',
  );
}

export function loadNativeBinding<T = unknown>(): T {
  const binary = findNativeBinary();
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  return require(binary) as T;
}

export type NativeBinding = ReturnType<typeof loadNativeBinding>;
