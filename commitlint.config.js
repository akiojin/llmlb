module.exports = {
  extends: ['@commitlint/config-conventional'],
  ignores: [
    // Merge commits
    (message) => message.startsWith('Merge '),
    // GitHub squash merge commits (e.g., "feature/branch-name (#123)")
    (message) => /\(#\d+\)$/.test(message.split('\n')[0]),
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
