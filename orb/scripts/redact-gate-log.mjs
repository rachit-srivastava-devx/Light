#!/usr/bin/env node

let input = '';
for await (const chunk of process.stdin) input += chunk;

const patterns = [
  [/\b(sk-(?:proj-|live-|test-)?[A-Za-z0-9_-]{12,})\b/g, '[REDACTED_OPENAI_KEY]'],
  [/\b(AIza[0-9A-Za-z_-]{20,})\b/g, '[REDACTED_GOOGLE_KEY]'],
  [/\b(gh[pousr]_[A-Za-z0-9]{20,})\b/g, '[REDACTED_GITHUB_TOKEN]'],
  [/\b(AKIA[0-9A-Z]{16})\b/g, '[REDACTED_AWS_KEY]'],
  [/\b(xox[baprs]-[A-Za-z0-9-]{10,})\b/g, '[REDACTED_SLACK_TOKEN]'],
  [/\bBearer\s+[A-Za-z0-9._~+/=-]{12,}\b/gi, 'Bearer [REDACTED]'],
  [
    /((?:API[_-]?KEY|ACCESS[_-]?TOKEN|AUTH[_-]?TOKEN|CLIENT[_-]?SECRET|PASSWORD)\s*[=:]\s*["']?)[^\s,"'}]+/gi,
    '$1[REDACTED]',
  ],
];

for (const [pattern, replacement] of patterns) input = input.replace(pattern, replacement);
process.stdout.write(input);
