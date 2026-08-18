# Cruft

A JavaScript and TypeScript runtime with its own engine, built so you can run
npm dependencies without handing each one your files, network, and environment.

> Cruft is an alpha release (0.0.9). Do not use it in production.

## The problem

Every JavaScript project runs code it didn't write. A fresh app pulls in
hundreds of packages before your first line runs, and nothing in Node stops any
one of them from reading your `.env`, opening a socket, or shelling out in a
`postinstall` script. A dependency runs with exactly your authority. `left-pad`,
`event-stream`, the `xz` backdoor were each this same fact on a different day.

Cruft draws a boundary Node can't: authority, scoped to a value, in-process.

## Compartments

A Compartment runs code in its own global scope with only the abilities you hand
it, and stops it if it runs too long. No subprocess, no copied heap.

```js
const c = new Compartment({
  globals: { fetch },          // it gets fetch, and nothing else
  timeout_ms: 50,              // and 50ms, which it cannot escape
});
c.evaluate(untrustedSource);   // no fs, no env, no process (none were granted)
```

The same idea applies to your whole dependency tree. `cruft --sealed-deps`
withholds host authority from every package unless it declares it needs it, and
`--audit` records what each one asked for. The authority is declined before a
malicious package can use it, rather than flagged after.

## Its own engine

Node and Deno wrap V8; Bun wraps JavaScriptCore. In each, the engine is a black
box the runtime drives from outside, so the only boundaries it can draw are the
isolate and the process.

Cruft's engine is its own, written in Rust to the ECMAScript and WHATWG
specifications: the parser, bytecode compiler, interpreter, garbage collector,
and JIT. That is what lets a Compartment be a first-class value and lets
authority be withheld per module. Cruft runs the full official test262
conformance suite, tracking a small set of triaged exceptions, and continuously
diffs its output against Node and V8 on real npm workloads.

## The npm ecosystem still runs

Cruft's compatibility target is the Node ecosystem:

- resolves `node:*` and bare specifiers, runs both CommonJS and ES modules, and
  honors `package.json` resolution;
- executes `.ts`/`.mts`/`.cts` by stripping types the way Node's `--strip-types`
  does, with no build step;
- ships a package manager in the same binary: `cruft install`, a lockfile, and a
  content-addressed store.

The bar is that real packages and real workloads like a `vite build` run
unmodified. Where they don't yet, the docs name the gap.

## Install

```sh
# npm (prebuilt binaries for linux, macOS, and Windows; x64 and arm64)
npm install -g @cruftless-dev/cruft

# or build from source
cargo build --release --bin cruft -p cruft   # binary at target/release/cruft
```

## Use

```sh
cruft app.js              # run a file (cruft run app.js is the explicit form)
cruft app.ts              # TypeScript runs directly, no build step
cruft install             # install dependencies into a content-addressed store
cruft                     # start the REPL
cruft --sealed-deps app.js  # run with host authority withheld from dependencies
```

## Limitations

- **The engine is Cruft's own, not a V8 fork.** It has different bugs and a
  different performance curve, and the JIT is far younger than V8's.
- **Some surfaces are partial.** A few APIs exist so feature-detection succeeds
  but do not implement the whole interface. The docs mark each one.
- **Node compatibility is broad, not a drop-in guarantee.** Where Node diverges
  from the ECMAScript spec, Cruft follows the spec and models Node's behavior as
  an explicit compatibility exception.

## License

Licensed under either of Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE)) or
the MIT License ([LICENSE-MIT](LICENSE-MIT)), at your option.

Cruft and CruftScript are trademarks of Frist Development, LLC. The code licenses
grant no rights in these names, logos, or trade dress.
