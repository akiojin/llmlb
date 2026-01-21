const getFirstLine = (message) =>
  message.split('\n')[0].replace(/^\uFEFF/, '').trim();

const legacyMessages = new Set([
  'Use config-based runtime hint for safetensors',
  'Update specs for Windows CUDA primary',
  'Fix Nemotron VRAM size compilation error',
  'Fix Windows test config reading and MSVC llama.cpp patch',
]);

module.exports = {
  extends: ['@commitlint/config-conventional'],
  ignores: [
    // Merge commits (handle BOM/leading whitespace)
    (message) => getFirstLine(message).startsWith('Merge '),
    // GitHub squash merge commits (e.g., "feature/branch-name (#123)")
    (message) => /\(#\d+\)$/.test(getFirstLine(message)),
    // Legacy non-conventional commits already in history
    (message) => legacyMessages.has(getFirstLine(message)),
  ],
  rules: {
    'header-max-length': [2, 'always', 72],
    'subject-full-stop': [2, 'never', '.'],
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
