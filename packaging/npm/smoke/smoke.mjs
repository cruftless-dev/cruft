
import fs from "node:fs";

const MARK = "CRUFT_SMOKE";

const sum = [1, 2, 3, 4].reduce((a, b) => a + b, 0);
if (sum !== 10) {
  console.error("smoke: arithmetic failed");
  process.exit(1);
}

const obj = JSON.parse('{"ok":true,"n":42}');
if (!obj.ok || obj.n !== 42 || JSON.stringify(obj) !== '{"ok":true,"n":42}') {
  console.error("smoke: JSON failed");
  process.exit(1);
}

const self = fs.readFileSync(process.argv[1], "utf8");
if (!self.includes(MARK)) {
  console.error("smoke: fs read failed");
  process.exit(1);
}

console.log(MARK + "_OK");
