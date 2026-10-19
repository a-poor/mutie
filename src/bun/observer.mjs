// Experimental Bun 1.3.11 observer. Not a production runner or a public preload API.
import * as native from "bun:test";
import { appendFileSync } from "node:fs";

const path = process.env.MUTIE_BUN_OBSERVER;
if (!path) throw new Error("Missing observer output path");
const append = appendFileSync;
const stringify = JSON.stringify;
const apply = Reflect.apply;
const then = Promise.prototype.then;
const NativePromise = Promise;
const schedule = setTimeout;
const checkpoint = () => new NativePromise(resolve => schedule(resolve, 0));
const hasInstance = Function.prototype[Symbol.hasInstance];
let sequence = 0;
let invocation = 0;
let registration = 0;
const emit = (event) => append(path, stringify({ sequence: ++sequence, ...event }) + "\n");
const unsupported = (reason) => emit({ event: "unsupported", reason });
const original = { ...native }; // Copy before mock.module updates ESM live bindings.
const wrapped = { ...original };
emit({ event: "init", revision: Bun.revision, version: Bun.version });

function callback(fn, phase, name, testId) {
  // Callback-style tests have runner-specific completion semantics. Preserve them,
  // but never classify the resulting run with this observer.
  if (typeof fn !== "function" || fn.length !== 0) {
    unsupported("callback arity or non-function");
    return fn;
  }
  return function () {
    const id = ++invocation;
    emit({ event: "enter", id, phase, name, ...(testId ? { testId } : {}) });
    // A synchronous throw can make Bun exit before pending rejections are
    // reported. Failure evidence is provisional until an event-loop checkpoint.
    function finish(event, value) {
      if (event === "return" || phase !== "body") {
        emit({ event, id, phase, name });
        if (event !== "return") throw value;
        return value;
      }
      emit({ event: "caught", id, phase, name, kind: event });
      return apply(then, checkpoint(), [() => {
        emit({ event, id, phase, name });
        if (event !== "return") throw value;
        return value;
      }]);
    }
    let result;
    try { result = apply(fn, this, arguments); }
    catch (error) { return finish("throw", error); }
    if (apply(hasInstance, NativePromise, [result])) {
      return apply(then, result, [
        value => finish("return", value),
        error => finish("reject", error),
      ]);
    }
    if (result !== null && (typeof result === "object" || typeof result === "function")) {
      unsupported("non-native object return");
      // Preserve native thenable behavior without probing its properties.
      return result;
    }
    return finish("return", result);
  };
}

for (const key of ["test", "it"]) {
  const target = original[key];
  wrapped[key] = new Proxy(target, {
    apply(fn, self, args) {
      if (args.length !== 2 || typeof args[0] !== "string") {
        unsupported("test registration overload");
        return apply(fn, self, args);
      }
      const testId = ++registration;
      emit({ event: "declare", testId, name: args[0] });
      return apply(fn, self, [args[0], callback(args[1], "body", args[0], testId)]);
    },
    get(fn, key, receiver) {
      unsupported("test modifier: " + String(key));
      const value = Reflect.get(fn, key, fn);
      return typeof value === "function" ? value.bind(fn) : value;
    },
  });
}
for (const key of ["beforeEach", "afterEach", "beforeAll", "afterAll"]) {
  wrapped[key] = function (...args) {
    if (args.length !== 1) unsupported("hook registration overload");
    return apply(original[key], this, [callback(args[0], key, key), ...args.slice(1)]);
  };
}
// Mocking can bypass the module observer. Keep native behavior, but disqualify it.
for (const key of ["mock", "jest", "vi", "spyOn", "setDefaultTimeout"]) {
  if (original[key] !== undefined) {
    wrapped[key] = new Proxy(original[key], {
      apply(fn, self, args) { unsupported("mocking/configuration API: " + key); return apply(fn, self, args); },
      get(fn, prop, receiver) { unsupported("mocking/configuration API: " + key); return Reflect.get(fn, prop, receiver); },
    });
  }
}
original.mock.module("bun:test", () => wrapped);
process.on("exit", (code) => emit({ event: "exit", code }));
