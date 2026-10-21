import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["tests/js/**/*.test.ts"],
    // Native (.node) addons load more reliably in a forked process than in
    // worker threads, so pin the pool to forks for the napi binding tests.
    pool: "forks",
  },
});
