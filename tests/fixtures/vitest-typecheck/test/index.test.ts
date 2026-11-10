import { expect, test } from "vitest";
import { enabled, optional } from "../src";

test("enabled", () => {
  expect(enabled()).toBe(true);
});

test("optional", () => {
  expect(optional).toBeTypeOf("function");
});
