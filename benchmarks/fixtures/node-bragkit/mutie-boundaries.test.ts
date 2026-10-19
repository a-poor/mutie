import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import cp from "node:child_process";
import { syncBuiltinESMExports } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { main } from "../src/cli.ts";
import { DEFAULT_CONFIG_PATH } from "../src/config.ts";
import { register } from "../src/collectors/registry.ts";
import { github } from "../src/collectors/github.ts";
import { jira } from "../src/collectors/jira.ts";
import { confluence } from "../src/collectors/confluence.ts";
import { slack } from "../src/collectors/slack.ts";
import { writePdf } from "../src/reports/pdf.ts";

const range = { since: "2025-01-01T00:00:00Z", until: "2025-12-31T00:00:00Z" };

// Restore synchronized builtin bindings inside each body, including assertion failures.
async function isolated(t: any, body: (dir: string) => Promise<void>) {
  const dir = fs.mkdtempSync(join(tmpdir(), "bragkit-boundary-"));
  const saved = { ...process.env };
  const read = fs.readFileSync;
  t.mock.method(fs, "readFileSync", (path: any, ...args: any[]) =>
    String(path) === DEFAULT_CONFIG_PATH ? "{}" : (read as any)(path, ...args));
  syncBuiltinESMExports();
  try { await body(dir); }
  finally {
    t.mock.restoreAll();
    syncBuiltinESMExports();
    for (const key of Object.keys(process.env)) if (!(key in saved)) delete process.env[key];
    Object.assign(process.env, saved);
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

const collectArgs = ["collect", "--source", "mutie-local", "--db", ":memory:",
  "--since", "2025-01-01", "--until", "2025-12-31"];

 test("collection defaults are opt-in and JSON mode is quiet", async t => isolated(t, async () => {
  let options: any;
  register({ name: "mutie-local", async collect(opts) { options = opts; return { achievements: [], errors: [] }; } });
  const lines: string[] = [], errors: string[] = [];
  t.mock.method(console, "log", (line: string) => { lines.push(line); });
  t.mock.method(process.stderr, "write", (line: string) => { errors.push(String(line)); return true; });
  assert.equal(await main([...collectArgs, "--json"]), 0);
  assert.equal(options.enrich, false);
  assert.equal(options.includeCreated, false);
  assert.equal(options.includeUpdated, false);
  assert.equal(errors.join(""), "");
  assert.equal(JSON.parse(lines[0]).collected, 0);
  lines.length = 0;
  assert.equal(await main(collectArgs), 0);
  assert.deepEqual(lines, []);
  assert.match(errors.join(""), /Collecting/);
}));

for (const command of ["collect", "export", "list", "runs"]) {
  test(`${command} rejects unexpected positional arguments`, async t => isolated(t, async dir => {
    register({ name: "mutie-local", async collect() { return { achievements: [], errors: [] }; } });
    t.mock.method(console, "log", () => {});
    t.mock.method(process.stderr, "write", () => true);
    const args = command === "collect" ? collectArgs : [command, "--db", ":memory:"];
    const extra = command === "export" ? ["--out", join(dir, "export.json")] : [];
    await assert.rejects(main([...args, ...extra, "unexpected"]), { code: "ERR_PARSE_ARGS_UNEXPECTED_POSITIONAL" });
  }));
}

test("report accepts a positional year and creates nested output directories", async t => isolated(t, async dir => {
  const out = join(dir, "reports", "nested", "report.json");
  assert.equal(await main(["report", "2025", "--db", ":memory:", "--format", "json", "--out", out]), 0);
  assert.doesNotThrow(() => JSON.parse(fs.readFileSync(out, "utf8")));
}));

test("database initialization creates missing ancestor directories", async t => isolated(t, async dir => {
  t.mock.method(console, "log", () => {});
  const db = join(dir, "data", "nested", "store.db");
  assert.equal(await main(["list", "--db", db]), 0);
  assert.ok(fs.statSync(db).isFile());
}));

test("GitHub enrichment is absent by default and present when requested", async t => isolated(t, async () => {
  t.mock.method(cp, "spawnSync", (command: string, args: string[]) => {
    assert.equal(command, "gh");
    const data = args[0] === "api" ? { data: { repository: { pullRequest: {
      additions: 9, deletions: 2, changedFiles: 1 } } } } : args[1] === "prs" ? [{
      number: 1, title: "Change", url: "https://example.invalid/pr/1", closedAt: range.until,
      repository: { nameWithOwner: "example/repo" } }] : [];
    return { status: 0, stdout: JSON.stringify(data), stderr: "" };
  });
  syncBuiltinESMExports();
  const normal = await github.collect(range);
  assert.deepEqual(normal.errors, []);
  assert.equal(normal.achievements.length, 1);
  assert.equal(normal.achievements[0].description, "");
  assert.equal(normal.achievements[0].metadata.additions, undefined);
  const enriched = await github.collect({ ...range, enrich: true });
  assert.equal(enriched.achievements[0].metadata.additions, 9);
}));

for (const kind of ["jira", "confluence"]) {
  test(`${kind} optional contributions require explicit opt-in`, async t => isolated(t, async () => {
    process.env.ATLASSIAN_SITE = "example.invalid";
    process.env.ATLASSIAN_EMAIL = "test@example.invalid";
    process.env.ATLASSIAN_API_TOKEN = "synthetic-test-token";
    t.mock.method(globalThis, "fetch", async (input: any, init: any) => {
      const url = new URL(input);
      assert.equal(url.hostname, "example.invalid");
      if (kind === "jira") return Response.json({ isLast: true, issues: [{
        key: "EX-1", fields: { summary: "Example", created: range.since, resolutiondate: range.until } }] });
      return Response.json({ results: [{ id: "1", title: "Example", type: "page", history: { createdDate: range.since } }] });
    });
    const collector = kind === "jira" ? jira : confluence;
    const normal = await collector.collect(range);
    assert.deepEqual(normal.errors, []);
    assert.deepEqual(normal.achievements.map(a => a.type), [kind === "jira" ? "issue_resolved" : "page_created"]);
    const expanded = await collector.collect({ ...range, includeCreated: true, includeUpdated: true });
    assert.deepEqual(expanded.achievements.map(a => a.type), kind === "jira"
      ? ["issue_resolved", "issue_created"] : ["page_created", "page_updated"]);
  }));
}

for (const failure of ["response", "transport"]) {
  test(`Slack surfaces ${failure} authentication errors`, async t => isolated(t, async () => {
    process.env.SLACK_USER_TOKEN = "synthetic-test-token";
    const calls: string[] = [];
    t.mock.method(globalThis, "fetch", async (input: any) => {
      calls.push(new URL(input).pathname);
      if (failure === "transport") throw new Error("offline fixture");
      return Response.json({ ok: false, error: "invalid_auth" });
    });
    const result = await slack.collect(range);
    assert.deepEqual(result.achievements, []);
    assert.deepEqual(result.errors, [`slack auth failed: ${failure === "transport" ? "offline fixture" : "invalid_auth"}`]);
    assert.deepEqual(calls, ["/api/auth.test"]);
  }));
}

test("missing browser reports failure and retains printable HTML", async t => isolated(t, async dir => {
  t.mock.method(fs, "existsSync", () => false);
  t.mock.method(cp, "spawnSync", (command: string) => {
    assert.equal(command, "which");
    return { status: 1, stdout: "", stderr: "" };
  });
  syncBuiltinESMExports();
  const result = writePdf("# Local report", join(dir, "report.pdf"));
  assert.equal(result.ok, false);
  assert.equal(result.chrome, null);
  assert.match(fs.readFileSync(result.html, "utf8"), /Local report/);
  assert.equal(fs.readdirSync(dir).includes("report.pdf"), false);
}));
