import { test, expect } from "bun:test";
import { enabled, fallback } from "../src/flags.js";
test("enabled", () => expect(enabled()).toBe(true));
test("nonempty input", () => expect(fallback(" value ")).toBe("value"));
