import assert from 'node:assert/strict';
import { test } from 'node:test';
import { ESLint } from 'eslint';
import { selfTest } from './normalize-crap-baseline.mjs';

test('CRAP normalizer inline acceptance fixtures', selfTest);

const eslint = new ESLint();
// Sixteen independent decisions exceed classic max 15. ESLint supplies the
// metric; these tests only verify the real config and test-only exception.
const highComplexity = `export function probe(value: number) {
  ${Array.from({ length: 16 }, (_, i) => `if (value === ${i}) return ${i};`).join('\n')}
  return -1;
}
const unusedProbe = 1;
`;

test('production complexity is a warning and other lint errors still apply', async () => {
  const [result] = await eslint.lintText(highComplexity, {
    filePath: 'src/crap-complexity-probe.ts',
  });
  const complexity = result.messages.filter((m) => m.ruleId === 'complexity');
  assert.equal(complexity.length, 1);
  assert.equal(complexity[0].severity, 1);
  assert.ok(
    result.messages.some(
      (m) => m.ruleId === '@typescript-eslint/no-unused-vars' && m.severity === 2,
    ),
  );
});

for (const suffix of ['test.ts', 'test.tsx', 'spec.ts', 'spec.tsx']) {
  test(`${suffix} exempts only complexity`, async () => {
    const [result] = await eslint.lintText(highComplexity, {
      filePath: `src/crap-complexity-probe.${suffix}`,
    });
    assert.ok(!result.messages.some((m) => m.ruleId === 'complexity'));
    assert.ok(
      result.messages.some(
        (m) => m.ruleId === '@typescript-eslint/no-unused-vars' && m.severity === 2,
      ),
    );
  });
}
