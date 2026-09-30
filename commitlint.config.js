const getFirstLine = (message) =>
  message.split('\n')[0].replace(/^\uFEFF/, '').trim();

const legacyMessages = new Set([
  'Use config-based runtime hint for safetensors',
  'Update specs for Windows CUDA primary',
  'Fix Nemotron VRAM size compilation error',
  'Fix Windows test config reading and MSVC llama.cpp patch',
  'merge: resolve develop conflicts for PR #402',
  'chore: remove gh-fix-ci skill files including LICENSE.txt, SKILL.md, and inspect_pr_checks.py as part of the cleanup process.',
]);

// Dependabot の件名は依存名・group 名・更新数で長さが決まり、header-max-length (72) を
// 設定側で保証できない（例: #720 の 75 文字、#666 の 81 文字）。
// .github/dependabot.yml の commit-message.prefix と一致する件名で、かつ Dependabot の
// Signed-off-by トレーラーで終わるコミットだけを除外し、人間のコミットの厳格さは維持する（Issue #751）。
// prefix を変える場合は dependabot.yml と揃え、make dependabot-subjects で整合を検証する。
const DEPENDABOT_SUBJECT_PREFIX = 'chore(deps): ';
const DEPENDABOT_SIGN_OFF = 'Signed-off-by: dependabot[bot] <support@github.com>';

const isDependabotCommit = (message) => {
  const lines = message.trim().split('\n');
  return (
    getFirstLine(message).startsWith(DEPENDABOT_SUBJECT_PREFIX) &&
    lines[lines.length - 1].trim() === DEPENDABOT_SIGN_OFF
  );
};

module.exports = {
  extends: ['@commitlint/config-conventional'],
  ignores: [
    // Merge commits (handle BOM/leading whitespace)
    (message) => getFirstLine(message).startsWith('Merge '),
    // GitHub squash merge commits (e.g., "feature/branch-name (#123)")
    (message) => /\(#\d+\)$/.test(getFirstLine(message)),
    // Legacy non-conventional commits already in history
    (message) => legacyMessages.has(getFirstLine(message)),
    // Dependabot が生成した依存更新コミット
    isDependabotCommit,
  ],
  rules: {
    'header-max-length': [2, 'always', 72],
    'subject-full-stop': [2, 'never', '.'],
    // Disable subject-case to allow acronyms (LLM, API, CLI, etc.) and non-Latin text
    'subject-case': [0],
    'scope-case': [2, 'always', ['kebab-case', 'lower-case', 'camel-case']],
    'type-enum': [
      2,
      'always',
      [
        'build',
        'chore',
        'ci',
        'docs',
        'feat',
        'fix',
        'perf',
        'refactor',
        'revert',
        'style',
        'test',
      ],
    ],
  },
};
