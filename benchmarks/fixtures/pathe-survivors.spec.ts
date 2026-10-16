// Copy into the prepared Pathe checkout's test/ directory. Production source stays unchanged.
import { expect, it, vi } from "vitest";
import { resolve } from "../src/index";
import { normalizeAliases, resolveAlias } from "../src/utils";

it("resolves empty inputs when cwd has no usable path", () => {
  // Pathe already tests this fallback with nonempty relative arguments.
  const cwd = vi.spyOn(process, "cwd").mockReturnValue("");
  try {
    expect(resolve()).toBe(".");
    expect(resolve("", "")).toBe(".");
  } finally {
    cwd.mockRestore();
  }
});

it("does not expand normalized aliases a second time", () => {
  const aliases = normalizeAliases({ pkg: "pkg/dist", feature: "pkg/feature" });
  expect(aliases).toEqual({ pkg: "pkg/dist", feature: "pkg/dist/feature" });
  expect(normalizeAliases(aliases)).toEqual(aliases);
});

it("resolves paths using already normalized aliases without repeating expansion", () => {
  const aliases = normalizeAliases({ pkg: "pkg/dist", feature: "pkg/feature" });
  expect(resolveAlias("feature/file.ts", aliases)).toBe("pkg/dist/feature/file.ts");
});
