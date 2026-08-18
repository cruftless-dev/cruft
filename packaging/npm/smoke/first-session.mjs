
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

let step = "";
const fail = (m) => { console.error(`FIRST_SESSION_FAIL[${step}]: ${m}`); process.exit(1); };

step = "fs";
const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cruft-fs-"));
const file = path.join(dir, "hello.txt");
fs.writeFileSync(file, "hello cruft");
if (fs.readFileSync(file, "utf8") !== "hello cruft") fail("fs round-trip");
fs.rmSync(dir, { recursive: true, force: true });

step = "stdlib";
if (JSON.parse('{"n":7}').n !== 7) fail("JSON");
if (Buffer.from("aGk=", "base64").toString() !== "hi") fail("Buffer base64");
if (new URL("https://h/a?b=1").searchParams.get("b") !== "1") fail("URL");
if (structuredClone({ x: [1, 2] }).x[1] !== 2) fail("structuredClone");
if (new TextEncoder().encode("hi").length !== 2) fail("TextEncoder");

step = "async";
await new Promise((r) => setTimeout(r, 5));
const settled = await Promise.all([Promise.resolve(1), Promise.resolve(2)]);
if (settled[0] + settled[1] !== 3) fail("promises");

console.log("FIRST_SESSION_OK");
