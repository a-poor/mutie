import { expect, test } from "bun:test";
import { createComposer } from "../src/factory.ts";
import { buildFromOptions } from "../src/macros.ts";

function customComposer() {
	return createComposer({
		discriminator: (_: object) => "message",
		methods: { greeting() { return "original"; } },
	}).Composer;
}

test("custom methods can be overridden on an individual instance", () => {
	const Composer = customComposer();
	const first = new Composer();
	const second = new Composer();
	first.greeting = () => "instance";
	expect(first.greeting()).toBe("instance");
	expect(second.greeting()).toBe("original");
});

test("custom methods can be redefined on the generated prototype", () => {
	const Composer = customComposer();
	Object.defineProperty(Composer.prototype, "greeting", {
		get() { return () => "replacement"; },
	});
	expect(new Composer().greeting()).toBe("replacement");
});

test("custom methods do not leak into context property enumeration", () => {
	const Composer = customComposer();
	const instance = new Composer();
	const keys: string[] = [];
	for (const key in instance) keys.push(key);
	expect(instance.greeting()).toBe("original");
	expect(keys).not.toContain("greeting");
});

test("true macro shorthand uses the factory default options", async () => {
	const calls: string[] = [];
	const handler = buildFromOptions(
		{
			greeting: (options: { name: string } = { name: "default" }) => ({
				preHandler: (_: object, next: () => Promise<void>) => {
					calls.push(options.name);
					return next();
				},
			}),
		},
		{ greeting: true },
		() => { calls.push("handler"); },
	);
	await handler({}, () => Promise.resolve());
	expect(calls).toEqual(["default", "handler"]);
});
