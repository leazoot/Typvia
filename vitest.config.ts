import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['apps/**/src/**/*.test.{ts,tsx}', 'packages/**/src/**/*.test.{ts,tsx}'],
    // The workspace starts with no frontend tests; the suite must still be runnable
    // so the unified `test` entry works from STAGE-01 onward.
    passWithNoTests: true,
  },
});
