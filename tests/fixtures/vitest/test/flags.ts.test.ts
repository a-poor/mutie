import { expect, test } from "vitest";
import { enabled, fallback } from "../src/flags.ts";

test("ts: enabled", () => {
  expect(enabled()).toBe(true);
});

test("ts: fallback accepts true", () => {
  expect(fallback(true)).toBe(true);
});
