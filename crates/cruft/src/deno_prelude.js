
(function () {
  'use strict';
  const fs = require('node:fs');
  const fsp = fs.promises;
  const os = require('node:os');
  const nodeInspect = require('node:util').inspect;
  const http = require('node:http');

  function toU8(b) {
    if (b instanceof Uint8Array && b.constructor === Uint8Array) return b;

    const view = b instanceof Uint8Array ? b : new Uint8Array(b);
    const out = new Uint8Array(view.length);
    out.set(view);
    return out;
  }

  const ERR_NAMES = [
    'NotFound', 'PermissionDenied', 'ConnectionRefused', 'ConnectionReset',
    'ConnectionAborted', 'NotConnected', 'AddrInUse', 'AddrNotAvailable',
    'BrokenPipe', 'AlreadyExists', 'InvalidData', 'TimedOut', 'Interrupted',
    'WriteZero', 'UnexpectedEof', 'BadResource', 'Http', 'Busy', 'NotSupported',
    'FilesystemLoop', 'IsADirectory', 'NetworkUnreachable', 'NotADirectory',
  ];
  const errors = {};
  for (const name of ERR_NAMES) {
    const C = class extends Error {
      constructor(msg) { super(msg); this.name = name; }
    };
    Object.defineProperty(C, 'name', { value: name, configurable: true });
    errors[name] = C;
  }

  const CODE_TO_DENO = {
    ENOENT: 'NotFound', EACCES: 'PermissionDenied', EPERM: 'PermissionDenied',
    EEXIST: 'AlreadyExists', ECONNREFUSED: 'ConnectionRefused', ECONNRESET: 'ConnectionReset',
    ECONNABORTED: 'ConnectionAborted', ENOTCONN: 'NotConnected', EADDRINUSE: 'AddrInUse',
    EADDRNOTAVAIL: 'AddrNotAvailable', EPIPE: 'BrokenPipe', ETIMEDOUT: 'TimedOut',
    EINTR: 'Interrupted', EBADF: 'BadResource', EBUSY: 'Busy', ENOSYS: 'NotSupported',
    ELOOP: 'FilesystemLoop', EISDIR: 'IsADirectory', ENETUNREACH: 'NetworkUnreachable',
    ENOTDIR: 'NotADirectory',
  };
  function mapErr(e) {
    const code = e && e.code;
    const name = (code && CODE_TO_DENO[code]) || 'NotFound';
    if (code && !CODE_TO_DENO[code]) {

      return e;
    }
    const de = new errors[name](e && e.message ? e.message : String(e));
    if (code) de.code = code;
    return de;
  }
  async function wrap(p) { try { return await p; } catch (e) { throw mapErr(e); } }
  function wrapSync(fn) { try { return fn(); } catch (e) { throw mapErr(e); } }

  function statToFileInfo(s) {
    const d = (ms) => (ms == null ? null : new Date(ms));
    return {
      isFile: s.isFile(), isDirectory: s.isDirectory(), isSymlink: s.isSymbolicLink(),
      size: s.size, mtime: d(s.mtimeMs), atime: d(s.atimeMs), birthtime: d(s.birthtimeMs),
      ctime: d(s.ctimeMs), dev: s.dev, ino: s.ino, mode: s.mode, nlink: s.nlink,
      uid: s.uid, gid: s.gid, rdev: s.rdev, blksize: s.blksize, blocks: s.blocks,
      isBlockDevice: s.isBlockDevice(), isCharDevice: s.isCharacterDevice(),
      isFifo: s.isFIFO(), isSocket: s.isSocket(),
    };
  }

  const SeekMode = { Start: 0, Current: 1, End: 2 };
  class FsFile {
    constructor(fd) { this.fd = fd; this[Symbol.dispose] = () => this.close(); }
    async read(p) {
      const { bytesRead } = await wrap(fsp.read
        ? this.fd.read(p, 0, p.length, null)
        : Promise.reject(new Error('read')));
      return bytesRead === 0 ? null : bytesRead;
    }
    async write(p) {
      const { bytesWritten } = await wrap(this.fd.write(p, 0, p.length, null));
      return bytesWritten;
    }
    close() { try { this.fd.close(); } catch (_) {} }
  }

  const cp = require('node:child_process');
  class Command {
    constructor(cmd, opts) { this._cmd = cmd; this._opts = opts || {}; }
    _spawnOpts() {
      const o = this._opts;
      return { cwd: o.cwd, env: o.env, };
    }
    outputSync() {
      const o = this._opts;
      const r = cp.spawnSync(this._cmd, o.args || [], this._spawnOpts());
      if (r.error) throw mapErr(r.error);
      return {
        code: r.status == null ? (r.signal ? 128 : 0) : r.status,
        signal: r.signal || null,
        success: r.status === 0,
        stdout: toU8(r.stdout || new Uint8Array(0)),
        stderr: toU8(r.stderr || new Uint8Array(0)),
      };
    }
    output() {
      const o = this._opts;
      return new Promise((resolve, reject) => {
        const child = cp.spawn(this._cmd, o.args || [], this._spawnOpts());
        const out = [], err = [];
        if (child.stdout) child.stdout.on('data', (d) => out.push(d));
        if (child.stderr) child.stderr.on('data', (d) => err.push(d));
        child.on('error', (e) => reject(mapErr(e)));
        child.on('close', (code, signal) => resolve({
          code: code == null ? (signal ? 128 : 0) : code, signal: signal || null,
          success: code === 0,
          stdout: toU8(Buffer.concat(out)), stderr: toU8(Buffer.concat(err)),
        }));
      });
    }
    spawn() { const o = this._opts; return cp.spawn(this._cmd, o.args || [], this._spawnOpts()); }
  }

  class PermissionStatus extends EventTarget {
    constructor(state) { super(); this.state = state; this.onchange = null; this.partial = false; }
  }

  let CAPS_MODE = 'compat';
  try { CAPS_MODE = globalThis.__cruft_caps_mode || 'compat'; } catch (_) { CAPS_MODE = 'compat'; }
  function capsMode() { return CAPS_MODE; }
  function permState(name) {
    if (name === 'ffi') return 'denied';
    if (CAPS_MODE === 'sealed') return 'denied';

    return 'granted';
  }
  const permissions = {
    query: (d) => Promise.resolve(new PermissionStatus(permState(d && d.name))),
    querySync: (d) => new PermissionStatus(permState(d && d.name)),
    request: (d) => Promise.resolve(new PermissionStatus(permState(d && d.name))),
    requestSync: (d) => new PermissionStatus(permState(d && d.name)),

    revoke: (d) => Promise.resolve(new PermissionStatus('denied')),
    revokeSync: (d) => new PermissionStatus('denied'),
  };

  const enc = new TextEncoder();
  const Deno = {
    errors,
    build: {
      target: `${os.arch()}-${process.platform === 'darwin' ? 'apple-darwin' : process.platform === 'win32' ? 'pc-windows-msvc' : 'unknown-linux-gnu'}`,
      arch: os.arch() === 'x64' ? 'x86_64' : os.arch(),
      os: process.platform === 'win32' ? 'windows' : process.platform,
      vendor: 'unknown', env: 'gnu', standalone: false,
    },
    version: { deno: '2.8.0', v8: '14.9.0-cruft', typescript: '6.0.3' },
    noColor: !!process.env.NO_COLOR,
    inspect: (v, opts) => nodeInspect(v, opts),

    args: process.argv.slice(2),
    pid: process.pid, ppid: process.ppid,
    env: {
      get: (k) => process.env[k], set: (k, v) => { process.env[k] = v; },
      has: (k) => Object.prototype.hasOwnProperty.call(process.env, k),
      delete: (k) => { delete process.env[k]; }, toObject: () => ({ ...process.env }),
    },
    cwd: () => process.cwd(), chdir: (d) => process.chdir(d),
    exit: (c) => process.exit(c), execPath: () => process.execPath,
    hostname: () => os.hostname(), loadavg: () => os.loadavg(),
    osRelease: () => os.release(), osUptime: () => Math.floor(os.uptime()),
    SeekMode, FsFile, Command, permissions, PermissionStatus,

    stdout: { write: async (p) => { process.stdout.write(Buffer.from(p)); return p.length; }, writeSync: (p) => { process.stdout.write(Buffer.from(p)); return p.length; }, close() {}, isTerminal: () => !!(process.stdout && process.stdout.isTTY) },
    stderr: { write: async (p) => { process.stderr.write(Buffer.from(p)); return p.length; }, writeSync: (p) => { process.stderr.write(Buffer.from(p)); return p.length; }, close() {}, isTerminal: () => !!(process.stderr && process.stderr.isTTY) },
    stdin: { read: async () => null, close() {}, isTerminal: () => !!(process.stdin && process.stdin.isTTY), setRaw() {} },

    readFile: (p) => wrap(fsp.readFile(p)).then(toU8),
    readFileSync: (p) => toU8(wrapSync(() => fs.readFileSync(p))),
    readTextFile: (p) => wrap(fsp.readFile(p, 'utf8')),
    readTextFileSync: (p) => wrapSync(() => fs.readFileSync(p, 'utf8')),
    writeFile: (p, data) => wrap(fsp.writeFile(p, data)),
    writeFileSync: (p, data) => wrapSync(() => fs.writeFileSync(p, data)),
    writeTextFile: (p, s) => wrap(fsp.writeFile(p, s)),
    writeTextFileSync: (p, s) => wrapSync(() => fs.writeFileSync(p, s)),
    stat: (p) => wrap(fsp.stat(p)).then(statToFileInfo),
    statSync: (p) => statToFileInfo(wrapSync(() => fs.statSync(p))),
    lstat: (p) => wrap(fsp.lstat(p)).then(statToFileInfo),
    lstatSync: (p) => statToFileInfo(wrapSync(() => fs.lstatSync(p))),
    mkdir: (p, o) => wrap(fsp.mkdir(p, o)),
    mkdirSync: (p, o) => wrapSync(() => fs.mkdirSync(p, o)),
    remove: (p, o) => wrap(fsp.rm(p, { recursive: !!(o && o.recursive), force: true })),
    removeSync: (p, o) => wrapSync(() => fs.rmSync(p, { recursive: !!(o && o.recursive), force: true })),
    rename: (a, b) => wrap(fsp.rename(a, b)),
    renameSync: (a, b) => wrapSync(() => fs.renameSync(a, b)),
    copyFile: (a, b) => wrap(fsp.copyFile(a, b)),
    copyFileSync: (a, b) => wrapSync(() => fs.copyFileSync(a, b)),
    chmod: (p, m) => wrap(fsp.chmod(p, m)),
    chmodSync: (p, m) => wrapSync(() => fs.chmodSync(p, m)),
    chown: (p, u, g) => wrap(fsp.chown(p, u, g)),
    chownSync: (p, u, g) => wrapSync(() => fs.chownSync(p, u, g)),
    symlink: (a, b) => wrap(fsp.symlink(a, b)),
    symlinkSync: (a, b) => wrapSync(() => fs.symlinkSync(a, b)),
    link: (a, b) => wrap(fsp.link(a, b)),
    linkSync: (a, b) => wrapSync(() => fs.linkSync(a, b)),
    truncate: (p, len) => wrap(fsp.truncate(p, len || 0)),
    truncateSync: (p, len) => wrapSync(() => fs.truncateSync(p, len || 0)),
    readLink: (p) => wrap(fsp.readlink(p)),
    readLinkSync: (p) => wrapSync(() => fs.readlinkSync(p)),
    realPath: (p) => wrap(fsp.realpath(p)),
    realPathSync: (p) => wrapSync(() => fs.realpathSync(p)),
    utime: (p, a, m) => wrap(fsp.utimes(p, a, m)),
    utimeSync: (p, a, m) => wrapSync(() => fs.utimesSync(p, a, m)),
    async *readDir(p) {
      const ents = await wrap(fsp.readdir(p, { withFileTypes: true }));
      for (const e of ents) yield { name: e.name, isFile: e.isFile(), isDirectory: e.isDirectory(), isSymlink: e.isSymbolicLink() };
    },
    *readDirSync(p) {
      const ents = wrapSync(() => fs.readdirSync(p, { withFileTypes: true }));
      for (const e of ents) yield { name: e.name, isFile: e.isFile(), isDirectory: e.isDirectory(), isSymlink: e.isSymbolicLink() };
    },
    makeTempDir: (o) => wrap(fsp.mkdtemp((o && o.dir ? o.dir : os.tmpdir()) + '/' + ((o && o.prefix) || ''))),
    makeTempDirSync: (o) => wrapSync(() => fs.mkdtempSync((o && o.dir ? o.dir : os.tmpdir()) + '/' + ((o && o.prefix) || ''))),
    open: async (p, o) => { const fd = await wrap(fsp.open(p, (o && o.write) ? (o.create ? 'w' : 'r+') : 'r')); return new FsFile(fd); },
    create: async (p) => { const fd = await wrap(fsp.open(p, 'w')); return new FsFile(fd); },

    gid: () => (typeof process.getgid === 'function' ? process.getgid() : null),
    uid: () => (typeof process.getuid === 'function' ? process.getuid() : null),
    memoryUsage: () => process.memoryUsage(),
    networkInterfaces: () => { const n = os.networkInterfaces(); const out = []; for (const k in n) for (const a of n[k]) out.push({ name: k, family: a.family, address: a.address, netmask: a.netmask, scopeid: a.scopeid, cidr: a.cidr, mac: a.mac }); return out; },
    consoleSize: () => ({ columns: (process.stdout && process.stdout.columns) || 80, rows: (process.stdout && process.stdout.rows) || 24 }),
    umask: (m) => (m == null ? process.umask() : process.umask(m)),
    kill: (pid, sig) => process.kill(pid, sig),
    mainModule: (typeof process.argv[1] === 'string' ? 'file://' + process.argv[1] : undefined),

    serve(a1, a2) {
      let opts = {}, handler;
      if (typeof a1 === 'function') handler = a1;
      else { opts = a1 || {}; handler = a2 || opts.handler || opts.fetch; }
      const hostname = opts.hostname || '0.0.0.0';
      let done; const finished = new Promise((r) => { done = r; });
      const srv = http.createServer((req, res) => {
        (async () => {
          const url = `http://${req.headers.host || hostname}${req.url}`;
          const chunks = [];
          req.on('data', (d) => chunks.push(d));
          await new Promise((r) => req.on('end', r));
          const hasBody = req.method !== 'GET' && req.method !== 'HEAD' && chunks.length;
          const request = new Request(url, { method: req.method, headers: req.headers, body: hasBody ? Buffer.concat(chunks) : undefined });
          let response;
          try { response = await handler(request, { remoteAddr: { transport: 'tcp', hostname, port: (req.socket && req.socket.remotePort) || 0 } }); }
          catch (e) { res.writeHead(500); res.end(String(e)); return; }
          const hdrs = {}; for (const [k, v] of response.headers) hdrs[k] = v;
          res.writeHead(response.status, hdrs);
          const ab = await response.arrayBuffer();
          res.end(Buffer.from(ab));
        })();
      });

      let boundPort = opts.port == null ? 8000 : opts.port;
      srv.listen(boundPort, hostname, () => {
        const a = srv.address();
        if (a && a.port) boundPort = a.port;
        if (opts.onListen) opts.onListen({ hostname, port: boundPort });
      });
      return {
        get addr() { const a = srv.address(); return { transport: 'tcp', hostname, port: (a && a.port) ? a.port : boundPort }; },
        finished,
        async shutdown() { await new Promise((r) => { try { srv.close(() => r()); } catch (_) { r(); } }); if (done) done(); },
        ref() { try { srv.ref && srv.ref(); } catch (_) {} }, unref() { try { srv.unref && srv.unref(); } catch (_) {} },
      };
    },
    addSignalListener: (sig, fn) => { process.on(sig, fn); },
    removeSignalListener: (sig, fn) => { process.removeListener(sig, fn); },
    async *watchFs(paths) {
      const list = Array.isArray(paths) ? paths : [paths];
      const queue = []; let resolve = null;
      const push = (kind, p) => { const ev = { kind, paths: [p] }; if (resolve) { resolve(ev); resolve = null; } else queue.push(ev); };
      for (const p of list) fs.watch(p, (etype, fname) => push(etype === 'rename' ? 'remove' : 'modify', fname ? p + '/' + fname : p));
      while (true) { yield queue.length ? queue.shift() : await new Promise((r) => { resolve = r; }); }
    },

    dlopen: () => { throw new Error('Deno FFI (dlopen) is not supported on cruft'); },
    UnsafeCallback: function () { throw new Error('Deno FFI (UnsafeCallback) is not supported on cruft'); },
    UnsafeFnPointer: function () { throw new Error('Deno FFI (UnsafeFnPointer) is not supported on cruft'); },
    UnsafePointer: { of: () => { throw new Error('Deno FFI (UnsafePointer) is not supported on cruft'); } },
    UnsafePointerView: function () { throw new Error('Deno FFI (UnsafePointerView) is not supported on cruft'); },
  };
  const _ = enc;

  try { if (process.versions && !process.versions.cruft) process.versions.cruft = '0.0.9'; } catch (_) {}

  Object.defineProperty(globalThis, 'Deno', {
    value: Deno, writable: true, enumerable: false, configurable: true,
  });
})();
