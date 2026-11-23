import test from "node:test";
import assert from "node:assert/strict";
import { createReporter, describeError, installErrorReporting, MAX_LENGTH, MAX_REPORTS } from "./errorReport.ts";

test("an error is sent once, with its whitespace flattened and its length capped", () => {
  const sent: string[] = []; const report = createReporter(m => sent.push(m));
  assert.equal(report("  boom \n  at   x "), true);
  assert.equal(report("a".repeat(MAX_LENGTH + 500)), true);
  assert.deepEqual(sent[0], "boom at x");
  assert.equal(sent[1].length, MAX_LENGTH);
});
test("the same error twice in a row, an empty one and the browser's own harmless ones are not sent", () => {
  const sent: string[] = []; const report = createReporter(m => sent.push(m));
  assert.equal(report("same"), true);
  assert.equal(report("same"), false, "a loop repeating one error");
  assert.equal(report("   "), false);
  assert.equal(report("ResizeObserver loop completed with undelivered notifications."), false);
  assert.equal(report("different"), true);
  assert.equal(report("same"), true, "only an immediate repeat is dropped");
  assert.deepEqual(sent, ["same", "different", "same"]);
});
test("a session is limited to a fixed number of reports", () => {
  const sent: string[] = []; const report = createReporter(m => sent.push(m));
  for (let i = 0; i < MAX_REPORTS + 30; i++) report(`error ${i}`);
  assert.equal(sent.length, MAX_REPORTS);
  assert.equal(createReporter(() => {}, 0)("x"), false, "a limit of none sends none");
});
test("a failing send is swallowed: reporting an error must never cause another", () => {
  const report = createReporter(() => { throw new Error("log unavailable"); });
  assert.doesNotThrow(() => report("x"));
});
test("errors, strings and odd values are all described as text", () => {
  const e = new TypeError("bad thing"); e.stack = "TypeError: bad thing\n    at f (tauri://localhost/assets/index.js:1:2345)\n    at g (x.js:9:9)";
  assert.equal(describeError(e), "TypeError: bad thing at f (tauri://localhost/assets/index.js:1:2345)");
  assert.equal(describeError("plain"), "plain");
  assert.equal(describeError({ code: 5 }), '{"code":5}');
  assert.equal(describeError(undefined), "undefined");
  const loop: Record<string, unknown> = {}; loop.self = loop;
  assert.equal(describeError(loop), "[object Object]", "a value that cannot be written out still gives text");
  const bare = new Error("no stack"); bare.stack = undefined;
  assert.equal(describeError(bare), "Error: no stack");
});
test("installing listens for page errors and for promise rejections and sends both", () => {
  const listeners = new Map<string, (event: never) => void>();
  const sent: string[] = [];
  const fake = { addEventListener: (type: string, fn: (event: never) => void) => { listeners.set(type, fn); } } as unknown as Window;
  installErrorReporting(fake, m => sent.push(m));
  assert.deepEqual([...listeners.keys()].sort(), ["error", "unhandledrejection"]);
  (listeners.get("error") as (e: unknown) => void)({ error: new RangeError("too far"), message: "x", filename: "tauri://localhost/assets/app-1.js", lineno: 77 });
  (listeners.get("error") as (e: unknown) => void)({ message: "Script error.", filename: "", lineno: 0 });
  (listeners.get("unhandledrejection") as (e: unknown) => void)({ reason: new Error("nobody caught me") });
  assert.match(sent[0], /^RangeError: too far .*\(app-1\.js:77\)$/);
  assert.equal(sent[1], "Script error.");
  assert.match(sent[2], /^Unhandled promise rejection: Error: nobody caught me/);
});
