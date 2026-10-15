import { expect, test } from "vitest";
import { enabled, fallback } from "../src/flags.js";

test("js: enabled", () => {
  expect(enabled()).toBe(true);
});

test("js: fallback accepts true", () => {
  expect(fallback(true)).toBe(true);
});
