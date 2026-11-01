import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// The workspace version lives in the root Cargo.toml (`cargo xtask bump-version`
// rewrites it), so the website must not carry a second copy. At build time this
// module is bundled into dist/.prerender, so resolve the workspace by walking
// up from the build working directory instead of trusting import.meta.url.
function findWorkspaceCargo(): string {
  let dir = process.cwd();
  for (;;) {
    const candidate = join(dir, 'Cargo.toml');
    if (existsSync(candidate) && /^\[workspace\]/m.test(readFileSync(candidate, 'utf8'))) {
      return candidate;
    }
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }

  const fallback = fileURLToPath(new URL('../../../Cargo.toml', import.meta.url));
  if (existsSync(fallback)) return fallback;

  throw new Error('could not locate the workspace Cargo.toml');
}

const cargoToml = readFileSync(findWorkspaceCargo(), 'utf8');
const match = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
if (!match) {
  throw new Error('could not read the workspace version from Cargo.toml');
}

export const APP_VERSION = match[1];
export const APP_TAG = `v${APP_VERSION}`;
export const RELEASE_BASE_URL =
  'https://github.com/zkz098/limedl/releases/latest/download';
