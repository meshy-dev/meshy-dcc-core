import { describe, expect, it } from 'vitest';

import { waitForVersion } from './api';

describe('waitForVersion', () => {
  it('keeps polling through a restart until the new version answers', async () => {
    const answers: Array<string | Error> = ['0.2.1', new Error('connection refused'), '0.2.2'];
    let reads = 0;
    const read = async () => {
      const answer = answers[Math.min(reads++, answers.length - 1)];
      if (answer instanceof Error) throw answer;
      return answer;
    };
    await expect(waitForVersion(read, '0.2.2', { timeoutMs: 1_000, intervalMs: 1 })).resolves.toBe(true);
    expect(reads).toBe(3);
  });

  it('gives up when the new version never answers', async () => {
    await expect(waitForVersion(async () => '0.2.1', '0.2.2', { timeoutMs: 20, intervalMs: 5 })).resolves.toBe(false);
  });
});
