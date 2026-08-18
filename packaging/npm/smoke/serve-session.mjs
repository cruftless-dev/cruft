
import http from "node:http";
const server = http.createServer((req, res) => {
  res.writeHead(200, { "content-type": "application/json" });
  res.end(JSON.stringify({ ok: true, path: req.url }));
});
server.listen(Number(process.argv[2]) || 0, "127.0.0.1", () => {
  console.log("SERVE_PORT:" + server.address().port);
});
setTimeout(() => process.exit(0), 15000);
