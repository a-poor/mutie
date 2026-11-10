import { expect, test } from "vitest";
import { encodeHost } from "../src";

test("encodes a pair beginning with the first high surrogate", () => {
  expect(encodeHost("\u{10000}.example")).toBe("xn--2n7c.example");
});

test("encodes a pair beginning with the last high surrogate", () => {
  expect(encodeHost("\u{10FC00}.example")).toBe("xn--4s22g.example");
});
