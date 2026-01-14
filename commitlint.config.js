const getFirstLine = (message) =>
  message.split('\n')[0].replace(/^\uFEFF/, '').trimStart();

module.exports = {
  extends: ['@commitlint/config-conventional'],
  ignores: [
    // Merge commits (handle BOM/leading whitespace)
    (message) => getFirstLine(message).startsWith('Merge '),
    // GitHub squash merge commits (e.g., "feature/branch-name (#123)")
    (message) => /\(#\d+\)$/.test(getFirstLine(message)),
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
