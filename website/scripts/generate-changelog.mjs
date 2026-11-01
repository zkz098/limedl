#!/usr/bin/env node
// Generates src/data/changelog.json from stable git tags and Conventional
// Commits. `pnpm build` runs this via the `prebuild` hook; when git metadata is
// unavailable (a shallow export, a tarball build) the checked-in snapshot is
// kept and the build continues.

import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(scriptDir, '..', '..');
const outDir = join(scriptDir, '..', 'src', 'data');
const outFile = join(outDir, 'changelog.json');

const GROUPS = [
  ['feat', 'features'],
  ['fix', 'fixes'],
  ['perf', 'performance'],
  ['refactor', 'refactoring'],
];

// Internal work that happens to use a user-facing type (fix(ci): …) is noise in
// a product changelog.
const SKIP_SCOPES = new Set(['ci', 'wiki', 'website', 'agents']);

function git(args) {
  return execFileSync('git', args, {
    cwd: repoRoot,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
  }).trim();
}

function stableTags() {
  const tags = git([
    'for-each-ref',
    '--sort=creatordate',
    '--format=%(refname:short)|%(creatordate:short)',
    'refs/tags',
  ])
    .split('\n')
    .map((line) => line.split('|'))
    .filter(([tag]) => /^v\d+\.\d+\.\d+$/.test(tag))
    .map(([tag, date]) => ({ tag, date }));

  return tags
    .map((entry) => ({ ...entry, parts: entry.tag.slice(1).split('.').map(Number) }))
    .sort((a, b) => a.parts[0] - b.parts[0] || a.parts[1] - b.parts[1] || a.parts[2] - b.parts[2]);
}

function parseCommits(range) {
  const raw = git(['log', '--no-merges', '--pretty=format:%H%x1f%s', range]);
  if (!raw) return [];

  const commits = [];
  for (const line of raw.split('\n')) {
    const [hash, subject] = line.split('\x1f');
    const match = /^(feat|fix|perf|refactor)(?:\(([^)]+)\))?(!)?:\s+(.+)$/.exec(subject ?? '');
    if (!match) continue;

    const [, type, scope, breaking, message] = match;
    if (scope && SKIP_SCOPES.has(scope)) continue;
    commits.push({
      hash,
      shortHash: hash.slice(0, 8),
      type,
      scope: scope ?? null,
      subject: message,
      breaking: Boolean(breaking),
    });
  }
  return commits;
}

function groupCommits(commits) {
  return GROUPS.map(([type, id]) => ({
    id,
    commits: commits.filter((commit) => commit.type === type),
  })).filter((group) => group.commits.length > 0);
}

function release(tag, date, range) {
  return {
    tag,
    version: tag.slice(1),
    date,
    groups: groupCommits(parseCommits(range)),
  };
}

function writeEmpty() {
  if (existsSync(outFile)) return;
  mkdirSync(outDir, { recursive: true });
  writeFileSync(
    outFile,
    `${JSON.stringify({ generatedAt: null, latestTag: null, unreleased: [], releases: [] }, null, 2)}\n`,
  );
}

try {
  if (git(['rev-parse', '--is-inside-work-tree']) !== 'true') throw new Error('not a git worktree');

  const tags = stableTags();
  if (tags.length === 0) throw new Error('no stable vX.Y.Z tags');

  const releases = [];
  for (let i = tags.length - 1; i >= 0; i -= 1) {
    const current = tags[i];
    const previous = i > 0 ? tags[i - 1].tag : null;
    const entry = release(
      current.tag,
      current.date,
      previous ? `${previous}..${current.tag}` : current.tag,
    );
    if (entry.groups.length > 0) releases.push(entry);
  }

  const latest = tags[tags.length - 1];
  const unreleasedCommits = parseCommits(`${latest.tag}..HEAD`);
  const data = {
    generatedAt: git(['log', '-1', '--format=%cs', 'HEAD']),
    latestTag: latest.tag,
    unreleased: groupCommits(unreleasedCommits),
    releases,
  };

  mkdirSync(outDir, { recursive: true });
  writeFileSync(outFile, `${JSON.stringify(data, null, 2)}\n`);
  console.log(
    `changelog: ${releases.length} stable releases, ${unreleasedCommits.length} unreleased commits`,
  );
} catch (error) {
  writeEmpty();
  console.warn(`changelog: keeping the existing snapshot (${error.message})`);
}
