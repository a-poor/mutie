import { expectTypeOf, test } from "vitest";
import { enabled } from "../src";

test("return type", () => {
  expectTypeOf(enabled()).toEqualTypeOf<boolean>();
});
