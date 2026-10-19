import { test, expect } from "bun:test";
import { nameMiddleware } from "../src/utils.ts";

test("middleware can be named again when reused in a different context", () => {
  const handler = () => {};
  nameMiddleware(handler, "first", "handler");
  expect(handler.name).toBe("first:handler");
  nameMiddleware(handler, "second", "handler");
  expect(handler.name).toBe("second:handler");
});
