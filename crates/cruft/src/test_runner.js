
(function () {
  'use strict';

  var performance = globalThis.performance;
  function now() {
    return performance && performance.now ? performance.now() : 0;
  }

  function TestNode(name, options, fn, parent) {
    this.name = name == null ? '<anonymous>' : String(name);
    this.options = options || {};
    this.fn = fn || null;
    this.parent = parent || null;
    this.depth = parent ? parent.depth + 1 : 0;
    this.children = [];

    this.hooks = { before: [], after: [], beforeEach: [], afterEach: [] };
    this.result = {
      name: this.name,
      passed: false,
      skipped: false,
      todo: false,
      expectFailure: false,
      expectFailureLabel: '',
      cancelled: false,
      error: null,
      duration_ms: 0,
      isSuite: false,
      depth: this.depth,
      location: this.options.__loc || null,
    };

    this.isDescribe = false;

    this._donePromise = null;
    this._resolveDone = null;
  }

  TestNode.prototype.donePromise = function () {
    var self = this;
    if (this._done) return Promise.resolve();
    if (!this._donePromise) {
      this._donePromise = new Promise(function (res) {
        self._resolveDone = res;
      });
    }
    return this._donePromise;
  };

  TestNode.prototype.markDone = function () {
    this._done = true;
    if (this._resolveDone) this._resolveDone();
  };

  function collectEachHooks(node, kind) {
    var chain = [];
    var s = node.parent;
    var stack = [];
    while (s) {
      stack.push(s);
      s = s.parent;
    }

    if (kind === 'beforeEach') {
      for (var i = stack.length - 1; i >= 0; i--) chain = chain.concat(stack[i].hooks.beforeEach);
    } else {
      for (var j = 0; j < stack.length; j++) chain = chain.concat(stack[j].hooks.afterEach);
    }
    return chain;
  }

  var currentTestContext = undefined;
  var originalSetImmediateForContext = null;

  function getTestContext() {
    return currentTestContext;
  }

  function withTestContext(ctx, fn) {
    var prev = currentTestContext;
    currentTestContext = ctx;
    try {
      return fn();
    } finally {
      currentTestContext = prev;
    }
  }
  function sourceFileFromLocation(loc) {
    if (!loc) return null;
    loc = String(loc);
    var m = loc.match(/^(.*):\d+:\d+$/);
    var file = m ? m[1] : loc;
    if (file.indexOf('file://') === 0) file = file.slice('file://'.length);
    return file;
  }
  function withNodeFileGlobals(node, fn) {
    var file = sourceFileFromLocation(node && node.result && node.result.location);
    if (!file) return fn();
    var hadFilename = Object.prototype.hasOwnProperty.call(globalThis, '__filename');
    var hadDirname = Object.prototype.hasOwnProperty.call(globalThis, '__dirname');
    var prevFilename = globalThis.__filename;
    var prevDirname = globalThis.__dirname;
    globalThis.__filename = file;
    globalThis.__dirname = file.slice(0, file.lastIndexOf('/'));
    try {
      return fn();
    } finally {
      if (hadFilename) globalThis.__filename = prevFilename;
      else delete globalThis.__filename;
      if (hadDirname) globalThis.__dirname = prevDirname;
      else delete globalThis.__dirname;
    }
  }

  function installContextImmediateCapture() {
    var g = globalThis;
    if (!g || typeof g.setImmediate !== 'function') return;
    if (originalSetImmediateForContext) return;
    originalSetImmediateForContext = g.setImmediate;
    g.setImmediate = function (cb) {
      var captured = currentTestContext;
      var rest = Array.prototype.slice.call(arguments, 1);
      return originalSetImmediateForContext(function () {
        var args = arguments.length ? Array.prototype.slice.call(arguments) : rest;
        return withTestContext(captured, function () {
          return cb.apply(null, args);
        });
      });
    };
  }

  function runMaybeAsync(fn, args, thisArg, ctx) {

    installContextImmediateCapture();
    return new Promise(function (resolve, reject) {
      var settled = false;
      var contextRestored = false;
      var prevContext = currentTestContext;
      if (ctx !== undefined) currentTestContext = ctx;
      function restoreContext() {
        if (contextRestored) return;
        contextRestored = true;
        if (ctx !== undefined) currentTestContext = prevContext;
      }
      function done(err) {
        if (settled) return;
        settled = true;
        restoreContext();
        if (err) reject(err);
        else resolve();
      }
      var usesCallback = fn.length >= args.length + 1;
      var ret;
      try {
        ret = usesCallback ? fn.apply(thisArg || null, args.concat([done])) : fn.apply(thisArg || null, args);
      } catch (e) {
        if (!settled) {
          settled = true;
          restoreContext();
          reject(e);
        }
        return;
      }
      if (ret && typeof ret.then === 'function') {
        ret.then(
          function () {
            if (!settled) {
              settled = true;
              restoreContext();
              resolve();
            }
          },
          function (e) {
            if (!settled) {
              settled = true;
              restoreContext();
              reject(e);
            }
          }
        );
      } else if (!usesCallback) {
        if (!settled) {
          settled = true;
          restoreContext();
          resolve();
        }
      }
    });
  }

  function hookFn(entry) {
    return entry && typeof entry === 'object' && entry.fn ? entry.fn : entry;
  }

  function hookNode(entry, fallback) {
    return entry && typeof entry === 'object' && entry.node ? entry.node : fallback;
  }

  function makeHookEntry(fn, node) {
    return { fn: fn, node: node };
  }

  function runNode(node, ctxFactory, onResult, onStart) {
    var opts = node.options;
    inheritExpectedFailure(node);
    var skip = !!opts.skip;
    var todo = !!opts.todo;
    var start = now();

    if (onStart) onStart(node);

    if (skip) {
      node.result.skipped = true;
      node.result.passed = true;

      node.result.isSuite = !!node.isDescribe;
      node.result.duration_ms = now() - start;
      onResult(node);
      node.markDone();
      return Promise.resolve();
    }

    inheritTodo(node);
    var isSuite = node.isDescribe || node.children.length > 0;
    node.result.isSuite = isSuite && node.isDescribe;
    if (node.options.__todo) todo = true;

    if (node.isDescribe) {

      return runHooks(node.hooks.before, node, ctxFactory)
        .then(function () {
          return runChildrenSequential(node, ctxFactory, onResult, onStart);
        })
        .then(function () {
          return runHooks(node.hooks.after, node, ctxFactory);
        })
        .then(
          function () {
            finishSuite(node, start, onResult);
          },
          function (e) {
            node.result.error = e;
            finishSuite(node, start, onResult);
          }
        );
    }

    var beforeEach = collectEachHooks(node, 'beforeEach');
    var afterEach = collectEachHooks(node, 'afterEach');
    var ctx = ctxFactory(node);
    var hasSubtestsMarker = { any: false };

    return runHooks(beforeEach, node, ctxFactory)
      .then(function () {
        if (!node.fn) return undefined;
        return withNodeFileGlobals(node, function () {
          var result = runMaybeAsync(node.fn, [ctx], ctx, ctx);
          var timeout = node.options && node.options.timeout;
          if (
            typeof timeout === 'number' &&
            isFinite(timeout) &&
            timeout > 0 &&
            result &&
            typeof result.then === 'function' &&
            globalThis.setTimeout
          ) {
            var timer = setTimeout(function () {
              node.result.cancelled = true;
              var signal = ctx.signal;
              if (signal && typeof signal.__cruftAbort === 'function') signal.__cruftAbort();
            }, Math.floor(timeout));
            return result.then(
              function (value) {
                if (globalThis.clearTimeout) clearTimeout(timer);
                return value;
              },
              function (err) {
                if (globalThis.clearTimeout) clearTimeout(timer);
                throw err;
              }
            );
          }
          return result;
        });
      })
      .then(
        function () {
          node.result.passed = true;
        },
        function (e) {
          node.result.passed = false;
          node.result.error = e;
        }
      )
      .then(function () {

        if (node.children.length) {
          hasSubtestsMarker.any = true;
          return Promise.all(
            node.children.map(function (c) {
              return c.donePromise();
            })
          );
        }
        return undefined;
      })
      .then(function () {
        return runHooks(afterEach, node, ctxFactory);
      })
      .then(function () {
        return runHooks(node.hooks.after, node, ctxFactory);
      })
      .then(function () {

        if (ctx && ctx._mock) ctx._mock.restoreAll();

        if (ctx && ctx._planExpected != null && !node.result.error) {
          var actual = ctx._planCount || 0;
          if (actual !== ctx._planExpected) {
            node.result.passed = false;
            node.result.error = new Error(
              'plan expected ' + ctx._planExpected + ' assertions but ' + actual + ' ran'
            );
          }
        }
      })
      .then(
        function () {
          finishLeaf(node, todo, start, onResult, hasSubtestsMarker.any);
        },
        function (e) {
          if (!node.result.error) node.result.error = e;
          node.result.passed = false;
          finishLeaf(node, todo, start, onResult, hasSubtestsMarker.any);
        }
      );
  }

  function finishLeaf(node, todo, start, onResult, hadSubtests) {
    node.result.todo = todo;
    if (todo) node.result.passed = true;

    if (hadSubtests) {
      for (var i = 0; i < node.children.length; i++) {
        if (node.children[i]._omitted) continue;
        var cr = node.children[i].result;
        if (!cr.passed && !cr.skipped && !cr.todo) {
          node.result.passed = false;
          break;
        }
      }
      node.result.isSuite = false;
    }
    applyExpectedFailureResult(node, hadSubtests);
    node.result.duration_ms = now() - start;
    onResult(node);
    node.markDone();
  }

  function finishSuite(node, start, onResult) {
    var allPass = true;
    for (var i = 0; i < node.children.length; i++) {
      if (node.children[i]._omitted) continue;
      var cr = node.children[i].result;
      if (!cr.passed && !cr.skipped && !cr.todo) allPass = false;
    }
    if (node.result.error) allPass = false;
    node.result.passed = allPass;
    applyExpectedFailureResult(node, node.children.length > 0);
    node.result.duration_ms = now() - start;
    onResult(node);
    node.markDone();
  }

  function runHooks(hooks, fallbackNode, ctxFactory) {
    var p = Promise.resolve();
    hooks.forEach(function (h) {
      p = p.then(function () {
        var owner = hookNode(h, fallbackNode);
        var ctx = owner && ctxFactory ? ctxFactory(owner) : undefined;
        return ctx ? runMaybeAsync(hookFn(h), [ctx], ctx, ctx) : runMaybeAsync(hookFn(h), []);
      });
    });
    return p;
  }

  function runChildrenSequential(node, ctxFactory, onResult, onStart) {
    var p = Promise.resolve();
    var parentOnly = node._inOnly || (node.options && node.options.only) || false;
    var parentMatch = node._inMatch || nameMatches(node);
    node.children.forEach(function (child) {
      child._inOnly = parentOnly;
      child._inMatch = parentMatch;
      if (!shouldRunChild(child)) {
        child._omitted = true;
        return;
      }
      p = p.then(function () {
        return runNode(child, ctxFactory, onResult, onStart);
      });
    });
    return p;
  }

  var kernel = {
    TestNode: TestNode,
    runNode: runNode,
  };

  var activeRegistrationFile = null;

  function captureLocation() {

    if (globalThis.__cruft_current_test_file) {
      activeRegistrationFile = String(globalThis.__cruft_current_test_file);
      return activeRegistrationFile;
    }
    var err = new Error();
    var stack = err.stack ? String(err.stack).split('\n') : [];
    for (var i = 2; i < stack.length; i++) {
      var line = stack[i];
      if (line && line.indexOf('test_runner') === -1 && line.indexOf('cruft-test-driver-') === -1) {
        var m = line.match(/\(?([^()\s]+:\d+:\d+)\)?\s*$/);
        if (m) {
          var loc = m[1];
          var fm = loc.match(/^(.*):\d+:\d+$/);
          var file = fm ? fm[1] : loc;
          if (file.indexOf('file://') === 0) file = file.slice('file://'.length);
          if (!activeRegistrationFile) activeRegistrationFile = file;
          return activeRegistrationFile || loc;
        }
      }
    }
    if (activeRegistrationFile) return activeRegistrationFile;
    return null;
  }

  function normalizeArgs(name, options, fn) {

    if (typeof name === 'function') {
      fn = name;
      name = undefined;
      options = undefined;
    } else if (typeof name === 'object' && name !== null) {
      fn = options;
      options = name;
      name = undefined;
    }
    if (typeof options === 'function') {
      fn = options;
      options = undefined;
    }
    options = options || {};
    return { name: name, options: options, fn: fn };
  }

  function outOfRange(name, value) {
    var e = new RangeError('The value of "' + name + '" is out of range. Received ' + String(value));
    e.code = 'ERR_OUT_OF_RANGE';
    return e;
  }

  function invalidArgValue(name, value) {
    var e = new TypeError('The argument "' + name + '" is invalid. Received ' + String(value));
    e.code = 'ERR_INVALID_ARG_VALUE';
    return e;
  }

  function invalidState(message) {
    var e = new Error(message || 'Invalid state');
    e.code = 'ERR_INVALID_STATE';
    return e;
  }

  function defineFunctionShape(fn, source) {
    if (typeof source !== 'function') return;
    try {
      Object.defineProperty(fn, 'name', Object.getOwnPropertyDescriptor(source, 'name'));
    } catch (_) {}
    try {
      Object.defineProperty(fn, 'length', Object.getOwnPropertyDescriptor(source, 'length'));
    } catch (_) {}
  }

  function ownKeys(object) {
    return Object.keys(Object(object));
  }

  function normalizeExpectedFailure(value) {
    if (value === undefined || value === false || value === null) return null;
    if (value === true) return { active: true, label: '', matcher: null };
    if (typeof value === 'string') return { active: true, label: value, matcher: null };
    if (typeof value === 'function') return { active: true, label: '', matcher: value };
    if (value instanceof RegExp) return { active: true, label: '', matcher: value };
    if (typeof value === 'object') {
      var keys = ownKeys(value);
      if (keys.length === 0) throw invalidArgValue('options.expectFailure', value);
      var hasLabel = Object.prototype.hasOwnProperty.call(value, 'label');
      var hasMatch = Object.prototype.hasOwnProperty.call(value, 'match');
      if ((hasLabel || hasMatch) && keys.every(function (k) { return k === 'label' || k === 'match'; })) {
        return {
          active: true,
          label: hasLabel ? String(value.label) : '',
          matcher: hasMatch ? value.match : null,
        };
      }
      return { active: true, label: '', matcher: value };
    }
    return { active: true, label: '', matcher: null };
  }

  function inheritTodo(node) {
    if (!node || !node.options) return;
    if (node.options.__todoNormalized) return;
    var t = !!node.options.todo;
    if (!t && node.parent && node.parent.options && node.parent.options.__todo) t = true;
    node.options.__todo = t;
    node.options.__todoNormalized = true;
  }

  function inheritExpectedFailure(node) {
    if (!node || !node.options) return;
    if (node.options.__expectFailureNormalized) return;
    if (Object.prototype.hasOwnProperty.call(node.options, 'expectFailure')) {
      node.options.__expectFailure = normalizeExpectedFailure(node.options.expectFailure);
    } else if (node.parent && node.parent.options && node.parent.options.__expectFailure) {
      node.options.__expectFailure = node.parent.options.__expectFailure;
    } else {
      node.options.__expectFailure = null;
    }
    node.options.__expectFailureNormalized = true;
  }

  function errorString(error) {
    if (!error) return '';
    var msg = error && error.message !== undefined ? String(error.message) : String(error);
    var name = error && error.name ? String(error.name) : '';
    return name ? name + ': ' + msg : msg;
  }

  function objectMatcherMatches(pattern, error) {
    if (!pattern || typeof pattern !== 'object') return false;
    var keys = ownKeys(pattern);
    for (var i = 0; i < keys.length; i++) {
      var key = keys[i];
      if (!error || error[key] !== pattern[key]) return false;
    }
    return true;
  }

  function expectedFailureMatches(meta, error) {
    if (!meta || !meta.active) return false;
    var matcher = meta.matcher;
    if (!matcher) return true;
    if (matcher instanceof RegExp) return matcher.test(errorString(error));
    if (typeof matcher === 'function') {
      if (matcher.prototype) return error instanceof matcher;
      try {
        return !!matcher(error);
      } catch (e) {
        return false;
      }
    }
    if (typeof matcher === 'object') return objectMatcherMatches(matcher, error);
    return true;
  }

  function applyExpectedFailureResult(node, hadSubtests) {
    var meta = node.options && node.options.__expectFailure;
    if (!meta || !meta.active || node.result.skipped || node.result.todo) return;
    node.result.expectFailure = true;
    node.result.expectFailureLabel = meta.label || '';
    if (hadSubtests) {
      node.result.passed = true;
      return;
    }
    if (node.result.passed) {
      node.result.passed = false;
      node.result.error = new Error('test was expected to fail but passed');
      return;
    }
    if (expectedFailureMatches(meta, node.result.error)) {
      node.result.passed = true;
    }
  }

  function validateTestOptions(options) {
    if (!options || typeof options !== 'object') return;
    if (Object.prototype.hasOwnProperty.call(options, 'expectFailure')) {
      options.__expectFailure = normalizeExpectedFailure(options.expectFailure);
      options.__expectFailureNormalized = true;
    }
    if (Object.prototype.hasOwnProperty.call(options, 'timeout')) {
      var timeout = options.timeout;
      if (timeout !== null && timeout !== undefined && timeout !== Infinity) {
        if (typeof timeout !== 'number') throw invalidArgType('timeout', 'number', timeout);
        if (Number.isNaN(timeout) || timeout < 0 || timeout > 0xffffffff) {
          throw outOfRange('timeout', timeout);
        }
      }
    }
    if (Object.prototype.hasOwnProperty.call(options, 'concurrency')) {
      var concurrency = options.concurrency;
      if (concurrency !== null && concurrency !== undefined && concurrency !== true && concurrency !== false) {
        if (typeof concurrency !== 'number') throw invalidArgType('concurrency', 'number', concurrency);
        if (
          !Number.isFinite(concurrency) ||
          Math.floor(concurrency) !== concurrency ||
          concurrency < 1 ||
          concurrency > 0x80000000
        ) {
          throw outOfRange('concurrency', concurrency);
        }
      }
    }
    if (Object.prototype.hasOwnProperty.call(options, 'signal')) {
      var signal = options.signal;
      if (
        signal !== undefined &&
        signal !== null &&
        (!signal ||
          (typeof signal !== 'object' && typeof signal !== 'function') ||
          typeof signal.aborted !== 'boolean' ||
          typeof signal.addEventListener !== 'function')
      ) {
        throw invalidArgType('signal', 'AbortSignal', signal);
      }
    }
    if (Object.prototype.hasOwnProperty.call(options, 'tags')) {
      options.__tags = normalizeTags(options.tags);
      if (options.__tags.length) emitTagsExperimentalWarning();
    }
  }

  var emittedTagsExperimentalWarning = false;
  function emitTagsExperimentalWarning() {
    if (emittedTagsExperimentalWarning || globalThis.__cruft_test_tags_warning_emitted) return;
    emittedTagsExperimentalWarning = true;
    globalThis.__cruft_test_tags_warning_emitted = true;
    if (typeof process !== 'undefined' && process && typeof process.emitWarning === 'function') {
      process.emitWarning('Test tags is an experimental feature and might change at any time', {
        type: 'ExperimentalWarning',
      });
    }
  }

  function normalizeTags(tags) {
    if (tags === undefined) return [];
    if (!Array.isArray(tags)) throw invalidArgType('options.tags', 'Array', tags);
    var out = [];
    var seen = Object.create(null);
    for (var i = 0; i < tags.length; i++) {
      var tag = tags[i];
      if (typeof tag !== 'string') throw invalidArgType('options.tags', 'string', tag);
      if (tag === '') throw invalidArgValue('options.tags', tag);
      var canonical = tag.toLowerCase();
      if (!seen[canonical]) {
        seen[canonical] = true;
        out.push(canonical);
      }
    }
    return out;
  }

  function ownTags(node) {
    if (!node || !node.options) return [];
    if (node.options.__tags) return node.options.__tags;
    if (Object.prototype.hasOwnProperty.call(node.options, 'tags')) {
      node.options.__tags = normalizeTags(node.options.tags);
      return node.options.__tags;
    }
    return [];
  }

  function flattenedTags(node) {
    var chain = [];
    var n = node;
    while (n && n.parent) {
      chain.push(n);
      n = n.parent;
    }
    var out = [];
    var seen = Object.create(null);
    for (var i = chain.length - 1; i >= 0; i--) {
      var tags = ownTags(chain[i]);
      for (var j = 0; j < tags.length; j++) {
        var tag = tags[j];
        if (!seen[tag]) {
          seen[tag] = true;
          out.push(tag);
        }
      }
    }
    return Object.freeze(out);
  }

  var root = new TestNode('<root>', {}, null, null);
  root.isDescribe = true;
  var currentSuite = root;
  var scheduled = false;
  var hasOnly = false;
  var namePattern = null;

  function nameMatches(node) {
    if (!namePattern) return false;
    try {
      return namePattern.test(node.name);
    } catch (e) {
      return false;
    }
  }
  function nodeContainsOnly(node) {
    if (node.options && node.options.only) return true;
    return node.children.some(nodeContainsOnly);
  }
  function nodeMatchesTree(node) {
    if (nameMatches(node)) return true;
    return node.children.some(nodeMatchesTree);
  }
  function shouldRunChild(child) {
    var onlyOk = !hasOnly || child._inOnly || nodeContainsOnly(child);
    var matchOk = !namePattern || child._inMatch || nodeMatchesTree(child);
    return onlyOk && matchOk;
  }

  var counts = { tests: 0, suites: 0, pass: 0, fail: 0, cancelled: 0, skipped: 0, todo: 0 };
  var failures = [];
  var runStart = 0;
  var outputBuffer = '';
  function reporterDestination() {
    var d = globalThis.__cruft_test_reporter_destination;
    return d && d !== 'stdout' && d !== 'stderr' ? String(d) : '';
  }

  function out(s) {
    if (reporterDestination()) {
      outputBuffer += s;
      return;
    }
    if (globalThis.process && process.stdout && process.stdout.write) {
      process.stdout.write(s);
    } else {

      globalThis.console && console.log(s.replace(/\n$/, ''));
    }
  }

  function indent(depth) {
    var s = '';
    for (var i = 0; i < depth; i++) s += '  ';
    return s;
  }

  function fmtDuration(ms) {
    return '(' + ms.toFixed(6).replace(/0+$/, '').replace(/\.$/, '') + 'ms)';
  }

  var diagnosticsChannel = null;
  function dc() {
    if (diagnosticsChannel !== null) return diagnosticsChannel;
    try {
      diagnosticsChannel = require('node:diagnostics_channel');
    } catch (_) {
      diagnosticsChannel = false;
    }
    return diagnosticsChannel;
  }

  function testEventType(node) {
    return node.isDescribe || node.name === '<root>' ? 'suite' : 'test';
  }

  function publishTestDiagnostic(kind, node) {
    var mod = dc();
    if (!mod || typeof mod.channel !== 'function') return;
    try {
      mod.channel('tracing:node.test:' + kind).publish({
        name: node.name,
        type: testEventType(node),
      });
    } catch (_) {}
  }

  var reporterName = null;
  function reporter() {
    if (reporterName === null) {
      var r = globalThis.__cruft_test_reporter;
      reporterName = r === 'tap' || r === 'junit' ? r : 'spec';
    }
    return reporterName;
  }
  var tapHeaderEmitted = false;
  function ensureTapHeader() {
    if (!tapHeaderEmitted) {
      tapHeaderEmitted = true;
      out('TAP version 13\n');
    }
  }
  function finishRunTap(total) {
    out('1..' + root.children.length + '\n');
    out('# tests ' + counts.tests + '\n');
    out('# suites ' + counts.suites + '\n');
    out('# pass ' + counts.pass + '\n');
    out('# fail ' + counts.fail + '\n');
    out('# cancelled ' + counts.cancelled + '\n');
    out('# skipped ' + counts.skipped + '\n');
    out('# todo ' + counts.todo + '\n');
    out('# duration_ms ' + total.toFixed(6) + '\n');
  }
  function finishRunJunit(total) {
    out('<testsuites>\n');
    out('<!-- tests ' + counts.tests + ' -->\n');
    out('<!-- pass ' + counts.pass + ' -->\n');
    out('<!-- fail ' + counts.fail + ' -->\n');
    out('<!-- duration_ms ' + total.toFixed(6) + ' -->\n');
    out('</testsuites>\n');
  }
  function flushReporterDestination() {
    var dest = reporterDestination();
    if (!dest) return;
    try {
      require('node:fs').writeFileSync(dest, outputBuffer);
    } catch (_) {}
  }

  function onStart(node) {
    publishTestDiagnostic('start', node);
    if (node === root) return;
    if (reporter() === 'junit') return;
    if (reporter() === 'tap') {
      ensureTapHeader();
      if (!node._announced) {
        node._announced = true;
        out(indent(node.depth - 1) + '# Subtest: ' + node.name + '\n');
      }
      return;
    }
    if (node.isDescribe) announceSpec(node);
  }

  function announceSpec(node) {
    if (node === root || node._announced) return;
    node._announced = true;
    out(indent(node.depth - 1) + '▶ ' + node.name + '\n');
  }

  function announceSubtestParent(node) {
    if (reporter() === 'spec') announceSpec(node);
  }

  function onResult(node) {
    if (node === root) publishTestDiagnostic('start', node);
    if (
      node !== root &&
      !node.isDescribe &&
      !node.result.passed &&
      !node.result.skipped &&
      !node.result.todo &&
      !node.result.cancelled
    ) {
      publishTestDiagnostic('error', node);
    }
    publishTestDiagnostic('end', node);

    if (node !== root) {
      var rc = node.result;
      if (rc.isSuite && node.isDescribe) {

        counts.suites++;

        if (rc.cancelled) counts.cancelled++;
      } else {
        counts.tests++;
        if (rc.cancelled) counts.cancelled++;
        else if (rc.skipped) counts.skipped++;
        else if (rc.todo) counts.todo++;
        else if (rc.passed) counts.pass++;
        else counts.fail++;
      }
    }
    if (node === root) return;
    if (
      !node.result.passed &&
      !node.result.skipped &&
      !node.result.todo &&
      !node.result.cancelled &&
      node.result.error
    ) {
      failures.push(node);
    }
    if (reporter() === 'tap') emitTapResult(node);
    else if (reporter() === 'spec') emitSpecResult(node);
  }

  function emitSpecResult(node) {
    var r = node.result;
    var pad = indent(node.depth - 1 < 0 ? 0 : node.depth - 1);
    var expectedFailureDirective = r.expectFailure
      ? ' # EXPECTED FAILURE' + (r.expectFailureLabel ? ' ' + r.expectFailureLabel : '')
      : '';
    var suffix = r.skipped ? ' # SKIP' : r.todo ? ' # TODO' : expectedFailureDirective;
    var symbol = r.skipped ? '﹣' : r.cancelled ? '✖' : r.passed ? '✔' : '✖';
    out(pad + symbol + ' ' + node.name + ' ' + fmtDuration(r.duration_ms) + suffix + '\n');
  }

  function emitTapResult(node) {
    ensureTapHeader();
    var r = node.result;
    var pad = indent(node.depth - 1 < 0 ? 0 : node.depth - 1);

    var num = node.parent ? node.parent.children.indexOf(node) + 1 : 1;

    if (node.children.length) {
      out(pad + '  1..' + node.children.length + '\n');
    }
    var status = r.passed || r.skipped || r.todo ? 'ok' : 'not ok';
    var expectedFailureDirective = r.expectFailure
      ? ' # EXPECTED FAILURE' + (r.expectFailureLabel ? ' ' + r.expectFailureLabel : '')
      : '';
    var directive = r.skipped ? ' # SKIP' : r.todo ? ' # TODO' : expectedFailureDirective;
    out(pad + status + ' ' + num + ' - ' + node.name + directive + '\n');
    out(pad + '  ---\n');
    out(pad + '  duration_ms: ' + r.duration_ms.toFixed(6) + '\n');
    out(pad + "  type: '" + (node.isDescribe ? 'suite' : 'test') + "'\n");
    if (r.expectFailure) out(pad + '  expected_failure: true\n');
    if (r.cancelled) out(pad + "  cancelled: true\n");
    if (!r.passed && !r.skipped && !r.todo && !r.cancelled && r.error) {
      var err = r.error;
      var emsg = err && err.message ? String(err.message) : String(err);
      out(pad + "  error: '" + emsg.replace(/\n/g, ' ') + "'\n");
      if (err && err.code) out(pad + "  code: '" + err.code + "'\n");
    }
    out(pad + '  ...\n');
  }

  function ctxFactory(node) {
    return new TestContext(node);
  }

  function TestContext(node) {
    this._node = node;
  }
  Object.defineProperty(TestContext.prototype, 'name', {
    get: function () {
      return this._node.name;
    },
  });
  Object.defineProperty(TestContext.prototype, 'filePath', {
    get: function () {
      return snapshotSourceFile(this._node);
    },
  });
  Object.defineProperty(TestContext.prototype, 'fullName', {
    get: function () {
      var names = [];
      var n = this._node;
      while (n && n.parent) {
        names.push(n.name);
        n = n.parent;
      }
      return names.reverse().join(' > ');
    },
  });
  Object.defineProperty(TestContext.prototype, 'passed', {
    get: function () {
      return !!(this._node.result && this._node.result.passed);
    },
  });
  Object.defineProperty(TestContext.prototype, 'attempt', {
    get: function () {
      return 0;
    },
  });
  function makeTestSignal() {
    var listeners = [];
    return {
      aborted: false,
      addEventListener: function (type, fn) {
        if (type === 'abort' && typeof fn === 'function') listeners.push(fn);
      },
      removeEventListener: function (type, fn) {
        if (type !== 'abort') return;
        listeners = listeners.filter(function (entry) {
          return entry !== fn;
        });
      },
      __cruftAbort: function () {
        if (this.aborted) return;
        this.aborted = true;
        listeners.slice().forEach(function (fn) {
          fn.call(null);
        });
      },
    };
  }
  Object.defineProperty(TestContext.prototype, 'signal', {
    get: function () {
      if (!this._signal) this._signal = makeTestSignal();
      return this._signal;
    },
  });
  Object.defineProperty(TestContext.prototype, 'workerId', {
    get: function () {
      return 0;
    },
  });
  Object.defineProperty(TestContext.prototype, 'tags', {
    get: function () {
      return flattenedTags(this._node);
    },
  });

  Object.defineProperty(TestContext.prototype, 'mock', {
    get: function () {
      if (!this._mock) this._mock = new MockTracker();
      return this._mock;
    },
  });

  var ASSERT_METHODS = [
    'ok', 'equal', 'notEqual', 'strictEqual', 'notStrictEqual', 'deepEqual',
    'notDeepEqual', 'deepStrictEqual', 'notDeepStrictEqual', 'throws',
    'doesNotThrow', 'rejects', 'doesNotReject', 'match', 'doesNotMatch',
    'ifError', 'fail', 'partialDeepStrictEqual',
  ];
  var customAssertions = {};

  function invalidArgType(name, expected, value) {
    var actual = typeof value;
    var e = new TypeError(
      'The "' + name + '" argument must be of type ' + expected +
        '. Received type ' + actual + ' (' + String(value) + ')'
    );
    e.code = 'ERR_INVALID_ARG_TYPE';
    return e;
  }

  var testAssertions = {
    register: function (name, fn) {
      if (typeof name !== 'string') throw invalidArgType('name', 'string', name);
      if (typeof fn !== 'function') throw invalidArgType('fn', 'function', fn);
      customAssertions[name] = fn;
    },
  };

  function makeContextAssert(ctx) {
    var nodeAssert = null;
    try {
      nodeAssert = require('node:assert');
    } catch (e) {
      nodeAssert = globalThis.assert || null;
    }
    var out = {};
    function installAssertMethod(name) {
      out[name] = function () {
        ctx._planCount = (ctx._planCount || 0) + 1;
        if (Object.prototype.hasOwnProperty.call(customAssertions, name)) {
          return customAssertions[name].apply(ctx, arguments);
        }
        if (nodeAssert && typeof nodeAssert[name] === 'function') {
          return nodeAssert[name].apply(nodeAssert, arguments);
        }
        throw new Error('t.assert.' + name + ' is unavailable');
      };
    }
    ASSERT_METHODS.forEach(installAssertMethod);
    Object.keys(customAssertions).forEach(function (name) {
      if (!Object.prototype.hasOwnProperty.call(out, name)) installAssertMethod(name);
    });
    out.snapshot = function (value, options) {
      ctx._planCount = (ctx._planCount || 0) + 1;
      if (Object.prototype.hasOwnProperty.call(customAssertions, 'snapshot')) {
        return customAssertions.snapshot.apply(ctx, arguments);
      }
      return assertSnapshot(ctx, value, options);
    };
    out.fileSnapshot = function (value, path, options) {
      ctx._planCount = (ctx._planCount || 0) + 1;
      if (Object.prototype.hasOwnProperty.call(customAssertions, 'fileSnapshot')) {
        return customAssertions.fileSnapshot.apply(ctx, arguments);
      }
      return assertFileSnapshot(ctx, value, path, options);
    };
    return out;
  }
  Object.defineProperty(TestContext.prototype, 'assert', {
    get: function () {
      if (!this._assert) this._assert = makeContextAssert(this);
      return this._assert;
    },
  });

  TestContext.prototype.plan = function (count) {
    this._planExpected = count;
  };
  TestContext.prototype.test = function (name, options, fn) {
    var a = normalizeArgs(name, options, fn);
    validateTestOptions(a.options);
    a.options.__loc = captureLocation();

    this._planCount = (this._planCount || 0) + 1;
    var child = new TestNode(a.name, a.options, a.fn, this._node);
    this._node.children.push(child);

    announceSubtestParent(this._node);

    var p = runNode(child, ctxFactory, onResult, onStart);
    return p.then(function () {
      return undefined;
    });
  };
  TestContext.prototype.diagnostic = function (msg) {
    out(indent(this._node.depth) + 'ℹ ' + msg + '\n');
  };
  TestContext.prototype.skip = function (msg) {
    this._node.result.skipped = true;
    this._node.options.skip = true;
    if (msg) this._node.result.skipMessage = msg;
  };
  TestContext.prototype.todo = function (msg) {
    this._node.options.todo = true;
    if (msg) this._node.result.todoMessage = msg;
  };
  TestContext.prototype.runOnly = function () {};
  TestContext.prototype.before = function (fn) {
    this._node.hooks.before.push(makeHookEntry(fn, this._node));
  };
  TestContext.prototype.after = function (fn) {
    this._node.hooks.after.push(makeHookEntry(fn, this._node));
  };
  TestContext.prototype.beforeEach = function (fn) {
    this._node.hooks.beforeEach.push(makeHookEntry(fn, this._node));
  };
  TestContext.prototype.afterEach = function (fn) {
    this._node.hooks.afterEach.push(makeHookEntry(fn, this._node));
  };
  TestContext.prototype.waitFor = function (value) {
    try {
      if (typeof value === 'function') value = value();
    } catch (e) {
      return Promise.reject(e);
    }
    return Promise.resolve(value);
  };

  var snapshotCache = {};
  function defaultResolveSnapshotPath(testFile) {
    return String(testFile) + '.snapshot';
  }
  var snapshotResolvePath = defaultResolveSnapshotPath;
  function defaultSnapshotSerializer(value) {
    return JSON.stringify(value, null, 2);
  }
  var defaultSerializers = [defaultSnapshotSerializer];
  var snapshotDefaultSerializers = defaultSerializers.slice();
  function snapshotSourceFile(node) {
    var loc = node && node.result && node.result.location;
    if (!loc) return null;
    loc = String(loc);
    var m = loc.match(/^(.*):\d+:\d+$/);
    var file = m ? m[1] : loc;
    if (file.indexOf('file://') === 0) file = file.slice('file://'.length);
    return file;
  }
  function snapshotError(message, code) {
    var e = new Error(message);
    if (code) e.code = code;
    return e;
  }
  function snapshotInvalidArg(name, expected, value) {
    var actual = value === null ? 'null' : typeof value;
    var e = new TypeError(
      'The "' + name + '" argument must be of type ' + expected +
        '. Received ' + actual
    );
    e.code = 'ERR_INVALID_ARG_TYPE';
    return e;
  }
  function snapshotInvalidArray(name) {
    var e = new TypeError('The "' + name + '" property must be an instance of Array');
    e.code = 'ERR_INVALID_ARG_TYPE';
    return e;
  }
  function validateSnapshotOptions(options) {
    if (options === undefined) return null;
    if (options === null || typeof options !== 'object' || Array.isArray(options)) {
      throw snapshotInvalidArg('options', 'object', options);
    }
    if (options.serializers !== undefined) {
      if (!Array.isArray(options.serializers)) throw snapshotInvalidArray('options.serializers');
      for (var i = 0; i < options.serializers.length; i++) {
        if (typeof options.serializers[i] !== 'function') {
          throw snapshotInvalidArg('options.serializers[' + i + ']', 'function', options.serializers[i]);
        }
      }
    }
    return options;
  }
  function snapshotSerialize(value) {
    var s = JSON.stringify(value, null, 2);
    if (s === undefined) s = String(value);
    return s;
  }
  function snapshotApplySerializers(value, serializers) {
    if (serializers && serializers.length) {
      var current = value;
      for (var i = 0; i < serializers.length; i++) {
        if (typeof serializers[i] !== 'function') {
          throw snapshotError('Snapshot serializer must be a function', 'ERR_INVALID_ARG_TYPE');
        }
        current = serializers[i](current);
      }
      return String(current);
    }
    return null;
  }
  function snapshotSerializeWithOptions(value, options) {
    options = validateSnapshotOptions(options);
    var serializers = options && options.serializers;
    var serialized = snapshotApplySerializers(value, serializers);
    if (serialized !== null) return serialized;
    serialized = snapshotApplySerializers(value, snapshotDefaultSerializers);
    if (serialized !== null) return serialized;
    return snapshotSerialize(value);
  }
  function snapshotEscapeKey(key) {
    return String(key).replace(/\\/g, '\\\\').replace(/`/g, '\\`').replace(/\$\{/g, '\\${');
  }
  function snapshotEscapeBody(body) {
    return String(body).replace(/\\/g, '\\\\').replace(/`/g, '\\`').replace(/\$\{/g, '\\${');
  }
  function readSnapshotFile(filename) {
    var fs = require('node:fs');
    var src = fs.readFileSync(filename, 'utf8');
    if (src.indexOf('exports[`') === -1) {
      throw snapshotError('Malformed snapshot file', 'ERR_INVALID_STATE');
    }
    var entries = {};
    try {
      Function('exports', src)(entries);
    } catch (e) {
      var malformed = snapshotError('Malformed snapshot file', 'ERR_INVALID_STATE');
      malformed.cause = e;
      throw malformed;
    }
    var order = Object.keys(entries);
    return { entries: entries, order: order };
  }
  function loadSnapshotState(filename, update) {
    var state = snapshotCache[filename];
    if (state) return state;
    if (update) {
      state = { entries: {}, order: [] };
    } else {
      state = readSnapshotFile(filename);
    }
    snapshotCache[filename] = state;
    return state;
  }
  function writeSnapshotFile(filename, state) {
    var fs = require('node:fs');
    var body = '';
    state.order.forEach(function (key) {
      body += 'exports[`' + snapshotEscapeKey(key) + '`] = `\n';
      body += snapshotEscapeBody(state.entries[key]) + '\n';
      body += '`;\n\n';
    });
    fs.writeFileSync(filename, body);
  }
  function assertSnapshot(ctx, value, options) {
    var file = snapshotSourceFile(ctx._node);
    if (!file) throw snapshotError('Cannot determine snapshot file', 'ERR_INVALID_STATE');
    var filename = String(snapshotResolvePath(file));
    var update = !!globalThis.__cruft_test_update_snapshots;
    var state;
    try {
      state = loadSnapshotState(filename, update);
    } catch (e) {
      var missing = snapshotError(
        "Cannot read snapshot file '" + filename + "'. Missing snapshots can be generated by rerunning the command with the --test-update-snapshots flag.",
        'ERR_INVALID_STATE'
      );
      missing.cause = e;
      missing.filename = filename;
      throw missing;
    }
    var index = (ctx._snapshotCount || 0) + 1;
    ctx._snapshotCount = index;
    var key = escapeSnapshotName(ctx._node.name + ' ' + index);
    var actual = snapshotSerializeWithOptions(value, options);
    var comparableActual = '\n' + actual + '\n';
    if (update) {
      if (!Object.prototype.hasOwnProperty.call(state.entries, key)) state.order.push(key);
      state.entries[key] = actual;
      writeSnapshotFile(filename, state);
      return;
    }
    if (!Object.prototype.hasOwnProperty.call(state.entries, key)) {
      throw snapshotError("Snapshot '" + key + "' is missing", 'ERR_INVALID_STATE');
    }
    if (state.entries[key] !== comparableActual) {
      var err = snapshotError("Snapshot '" + key + "' does not match", 'ERR_ASSERTION');
      err.actual = comparableActual;
      err.expected = state.entries[key];
      err.operator = 'snapshot';
      throw err;
    }
  }
  function assertFileSnapshot(ctx, value, path, options) {
    if (typeof path !== 'string') {
      throw snapshotInvalidArg('path', 'string', path);
    }
    var filename = String(path);
    var update = !!globalThis.__cruft_test_update_snapshots;
    var actual = snapshotSerializeWithOptions(value, options);
    var fs = require('node:fs');
    if (update) {
      try {
        var pathMod = require('node:path');
        fs.mkdirSync(pathMod.dirname(filename), { recursive: true });
      } catch (e) {
        void e;
      }
      fs.writeFileSync(filename, actual);
      return;
    }
    var expected;
    try {
      expected = fs.readFileSync(filename, 'utf8');
    } catch (e) {
      var missing = snapshotError(
        "Cannot read snapshot file '" + filename + "'. Missing snapshots can be generated by rerunning the command with the --test-update-snapshots flag.",
        'ERR_INVALID_STATE'
      );
      missing.cause = e;
      missing.filename = filename;
      throw missing;
    }
    if (expected !== actual) {
      var err = snapshotError("Snapshot file '" + filename + "' does not match", 'ERR_ASSERTION');
      err.actual = actual;
      err.expected = expected;
      err.operator = 'fileSnapshot';
      throw err;
    }
  }

  var snapshot = {
    setResolveSnapshotPath: function (fn) {
      if (typeof fn !== 'function') {
        throw snapshotError('Snapshot path resolver must be a function', 'ERR_INVALID_ARG_TYPE');
      }
      snapshotResolvePath = fn;
    },
    setDefaultSnapshotSerializers: function (serializers) {
      if (!Array.isArray(serializers)) {
        throw snapshotInvalidArray('serializers');
      }
      var next = [];
      for (var i = 0; i < serializers.length; i++) {
        if (typeof serializers[i] !== 'function') {
          throw snapshotInvalidArg('serializers[' + i + ']', 'function', serializers[i]);
        }
        next.push(serializers[i]);
      }
      snapshotDefaultSerializers = next;
    },
  };
  function escapeSnapshotName(name) {
    return String(name)
      .replace(/\\/g, '\\\\')
      .replace(/`/g, '\\`')
      .replace(/\$\{/g, '\\${')
      .replace(/\r/g, '\\r')
      .replace(/\n/g, '\\n');
  }
  function SnapshotFile(snapshotFile) {
    this.snapshotFile = snapshotFile;
    this.snapshots = {};
    this._counts = {};
    this._dirty = false;
  }
  SnapshotFile.prototype.nextId = function (name) {
    name = String(name);
    var next = (this._counts[name] || 0) + 1;
    this._counts[name] = next;
    return name + ' ' + next;
  };
  SnapshotFile.prototype.readFile = function () {
    if (this._manager && this._manager.update) return;
    try {
      var state = readSnapshotFile(this.snapshotFile);
      this.snapshots = {};
      for (var i = 0; i < state.order.length; i++) {
        var key = state.order[i];
        this.snapshots[key] = state.entries[key];
      }
    } catch (e) {
      var err = snapshotError(
        "Cannot read snapshot file '" + this.snapshotFile + "'. Missing snapshots can be generated by rerunning the command with the --test-update-snapshots flag.",
        'ERR_INVALID_STATE'
      );
      err.filename = this.snapshotFile;
      err.cause = e;
      throw err;
    }
  };
  SnapshotFile.prototype.getSnapshot = function (name) {
    name = String(name);
    if (!Object.prototype.hasOwnProperty.call(this.snapshots, name)) {
      var err = snapshotError("Snapshot '" + name + "' not found", 'ERR_INVALID_STATE');
      err.snapshot = name;
      err.filename = this.snapshotFile;
      throw err;
    }
    return this.snapshots[name];
  };
  SnapshotFile.prototype.setSnapshot = function (name, value) {
    this.snapshots[escapeSnapshotName(name)] = value;
    this._dirty = true;
  };
  SnapshotFile.prototype.writeFile = function () {
    var fs = require('node:fs');
    var path = require('node:path');
    var body = '';
    var keys = Object.keys(this.snapshots);
    keys.forEach(function (key) {
      body += 'exports[`' + key + '`] = `\n';
      body += String(this.snapshots[key]);
      body += '\n`;\n';
    }, this);
    try {
      fs.mkdirSync(path.dirname(this.snapshotFile), { recursive: true });
      fs.writeFileSync(this.snapshotFile, body);
    } catch (e) {
      var err = snapshotError("Cannot write snapshot file '" + this.snapshotFile + "'", 'ERR_INVALID_STATE');
      err.filename = this.snapshotFile;
      err.cause = e;
      throw err;
    }
  };
  function SnapshotManager(update) {
    this.update = !!update;
    this._files = {};
  }
  SnapshotManager.prototype.resolveSnapshotFile = function (testFile) {
    var filename = snapshotResolvePath(testFile);
    if (typeof filename !== 'string' || !filename) {
      var err = snapshotError('Invalid snapshot filename', 'ERR_INVALID_STATE');
      err.filename = filename;
      throw err;
    }
    var file = this._files[filename];
    if (!file) {
      file = new SnapshotFile(filename);
      file._manager = this;
      this._files[filename] = file;
    }
    return file;
  };
  SnapshotManager.prototype.serialize = function (value, serializers) {
    if (serializers === undefined) serializers = snapshotDefaultSerializers;
    try {
      var current = value;
      for (var i = 0; i < serializers.length; i++) current = serializers[i](current);
      if (typeof current !== 'string') current = String(current);
      return '\n' + snapshotEscapeBody(current) + '\n';
    } catch (e) {
      var err = snapshotError('The provided serializers did not generate a string', 'ERR_INVALID_STATE');
      err.input = value;
      err.cause = e;
      throw err;
    }
  };
  SnapshotManager.prototype.createAssert = function () {
    var manager = this;
    return function (value, options) {
      var filename = this && this.filePath;
      if (typeof filename !== 'string' || !filename) {
        var err = snapshotError('Invalid snapshot filename', 'ERR_INVALID_STATE');
        err.filename = filename;
        throw err;
      }
      var file = manager.resolveSnapshotFile(filename);
      var id = file.nextId(this.name || 'snapshot');
      var serialized = manager.serialize(value, options && options.serializers);
      if (manager.update) {
        file.setSnapshot(id, serialized);
        return;
      }
      var expected = file.getSnapshot(id);
      if (expected !== serialized) {
        var mismatch = snapshotError("Snapshot '" + id + "' does not match", 'ERR_ASSERTION');
        mismatch.actual = serialized;
        mismatch.expected = expected;
        mismatch.operator = 'snapshot';
        throw mismatch;
      }
    };
  };
  SnapshotManager.prototype.writeSnapshotFiles = function () {
    if (!this.update) return;
    var keys = Object.keys(this._files);
    for (var i = 0; i < keys.length; i++) this._files[keys[i]].writeFile();
  };
  var internalSnapshot = {
    SnapshotManager: SnapshotManager,
    defaultResolveSnapshotPath: defaultResolveSnapshotPath,
    defaultSerializers: defaultSerializers,
  };

  function runAll() {
    runStart = now();
    if (namePattern === null && globalThis.__cruft_test_name_pattern) {
      try {
        namePattern = new RegExp(globalThis.__cruft_test_name_pattern);
      } catch (e) {
        namePattern = null;
      }
    }
    return runNode(root, ctxFactory, onResult, onStart).then(
      function () {
        finishRun();
        return counts.fail;
      },
      function (e) {
        out('✖ test harness error: ' + (e && e.stack ? e.stack : e) + '\n');
        finishRun();
        return counts.fail || 1;
      }
    );
  }

  function scheduleDrain() {
    if (scheduled) return;
    scheduled = true;

    if (globalThis.__cruft_test_cli_mode) return;

    if (globalThis.queueMicrotask)
      queueMicrotask(function () {

        Promise.resolve().then(runAll);
      });
    else setTimeout(runAll, 0);
  }

  function finishRun() {
    var total = now() - runStart;
    if (reporter() === 'junit') {
      finishRunJunit(total);
      flushReporterDestination();
      if ((counts.fail > 0 || counts.cancelled > 0) && globalThis.process) process.exitCode = 1;
      return;
    }
    if (reporter() === 'tap') {
      finishRunTap(total);
      flushReporterDestination();
      if ((counts.fail > 0 || counts.cancelled > 0) && globalThis.process) process.exitCode = 1;
      return;
    }
    out('ℹ tests ' + counts.tests + '\n');
    out('ℹ suites ' + counts.suites + '\n');
    out('ℹ pass ' + counts.pass + '\n');
    out('ℹ fail ' + counts.fail + '\n');
    out('ℹ cancelled ' + counts.cancelled + '\n');
    out('ℹ skipped ' + counts.skipped + '\n');
    out('ℹ todo ' + counts.todo + '\n');
    out('ℹ duration_ms ' + total.toFixed(6) + '\n');

    if (failures.length) {
      out('\n✖ failing tests:\n');
      failures.forEach(function (node) {
        var r = node.result;
        out('\n');
        if (r.location) out('test at ' + r.location + '\n');
        out('✖ ' + node.name + ' ' + fmtDuration(r.duration_ms) + '\n');
        var err = r.error;
        var msg = err && err.stack ? String(err.stack) : String(err && err.message ? err.message : err);
        out(
          msg
            .split('\n')
            .map(function (l) {
              return '  ' + l;
            })
            .join('\n') + '\n'
        );
      });
    }

    if ((counts.fail > 0 || counts.cancelled > 0) && globalThis.process) {
      process.exitCode = 1;
    }
    flushReporterDestination();
  }

  function MockTimers() {
    this._enabled = false;
    this._now = 0;
    this._seq = 0;
    this._queue = [];
    this._orig = {};
  }
  MockTimers.prototype.enable = function (options) {
    if (this._enabled) throw invalidState('Mock timers are already enabled');
    var apis = (options && options.apis) || ['setTimeout', 'setInterval', 'setImmediate', 'Date'];
    for (var ai = 0; ai < apis.length; ai++) {
      if (typeof apis[ai] !== 'string') throw invalidArgType('apis', 'string', apis[ai]);
      if (
        apis[ai] !== 'setTimeout' &&
        apis[ai] !== 'setInterval' &&
        apis[ai] !== 'setImmediate' &&
        apis[ai] !== 'Date' &&
        apis[ai] !== 'scheduler.wait'
      ) {
        throw invalidArgValue('apis', apis[ai]);
      }
    }
    var initialNow =
      options && options.now !== undefined
        ? options.now instanceof Date
          ? options.now.getTime()
          : Number(options.now)
        : 0;
    if (options && options.now !== undefined) {
      if (typeof options.now !== 'number' && !(options.now instanceof Date)) {
        throw invalidArgType('now', 'number', options.now);
      }
      if (!isFinite(initialNow) || initialNow < 0) throw invalidArgValue('now', options.now);
    }
    this._now = initialNow;
    this._enabled = true;
    var self = this;
    var g = globalThis;
    function normalizeMockDelay(delay) {
      var n = Number(delay || 0);
      if (!isFinite(n) || n < 0 || n > 2147483647) return 1;
      return n;
    }
    function timerHandle(id) {
      return {
        __mockTimerId: id,
        close: function () {
          cancel(id);
          return this;
        },
        ref: function () { return this; },
        unref: function () { return this; },
        hasRef: function () { return true; },
        [Symbol.dispose]: function () {
          cancel(id);
        },
      };
    }
    function schedule(cb, delay, args, interval, kind) {
      var id = ++self._seq;
      var d = kind === 'immediate' ? 0 : normalizeMockDelay(delay);
      self._queue.push({ id: id, due: self._now + d, cb: cb, args: args, interval: interval, kind: kind || 'timeout' });
      return timerHandle(id);
    }
    function cancel(id) {
      if (id && typeof id === 'object' && typeof id.close === 'function' && id.__mockTimerId !== undefined) {
        id = id.__mockTimerId;
      }
      self._queue = self._queue.filter(function (t) {
        return t.id !== id;
      });
    }
    function makeSet(kind, interval) {
      return function (cb, delay) {
        return schedule(cb, delay, Array.prototype.slice.call(arguments, 2), interval ? normalizeMockDelay(delay) || 1 : 0, kind);
      };
    }
    function abortError() {
      var e = new Error('The operation was aborted');
      e.name = 'AbortError';
      e.code = 'ABORT_ERR';
      return e;
    }
    function validateSignal(signal) {
      if (
        signal !== undefined &&
        (!signal ||
          (typeof signal !== 'object' && typeof signal !== 'function') ||
          typeof signal.aborted !== 'boolean' ||
          typeof signal.addEventListener !== 'function')
      ) {
        throw invalidArgType('options.signal', 'AbortSignal', signal);
      }
    }
    function mockPromiseTimeout(delay, value, options) {
      var signal = options && options.signal;
      try {
        validateSignal(signal);
      } catch (e) {
        return Promise.reject(e);
      }
      return new Promise(function (resolve, reject) {
        var done = false;
        var handle;
        function cleanup() {
          if (signal && signal.removeEventListener) signal.removeEventListener('abort', onAbort);
        }
        function onAbort() {
          if (done) return;
          done = true;
          cancel(handle);
          cleanup();
          reject(abortError());
        }
        if (signal) {
          if (signal.aborted) {
            reject(abortError());
            return;
          }
          signal.addEventListener('abort', onAbort);
        }
        handle = schedule(function () {
          if (done) return;
          done = true;
          cleanup();
          resolve(value);
        }, delay || 0, [], 0, 'timeout');
      });
    }
    function mockPromiseImmediate(value, options) {
      var signal = options && options.signal;
      try {
        validateSignal(signal);
      } catch (e) {
        return Promise.reject(e);
      }
      return new Promise(function (resolve, reject) {
        var done = false;
        var handle;
        function cleanup() {
          if (signal && signal.removeEventListener) signal.removeEventListener('abort', onAbort);
        }
        function onAbort() {
          if (done) return;
          done = true;
          cancel(handle);
          cleanup();
          reject(abortError());
        }
        if (signal) {
          if (signal.aborted) {
            reject(abortError());
            return;
          }
          signal.addEventListener('abort', onAbort);
        }
        handle = schedule(function () {
          if (done) return;
          done = true;
          cleanup();
          resolve(value);
        }, 0, [], 0, 'immediate');
      });
    }
    function mockPromiseInterval(delay, value, options) {
      var signal = options && options.signal;
      try {
        validateSignal(signal);
      } catch (e) {
        return {
          next: function () { return Promise.reject(e); },
          return: function () { return Promise.resolve({ done: true, value: undefined }); },
          [Symbol.asyncIterator]: function () { return this; },
        };
      }
      var waiters = [];
      var values = [];
      var done = false;
      var handle;
      function cleanup() {
        if (signal && signal.removeEventListener) signal.removeEventListener('abort', onAbort);
      }
      function settleNext(result) {
        var waiter = waiters.shift();
        if (waiter) waiter.resolve(result);
        else values.push(result);
      }
      function rejectNext(err) {
        var waiter = waiters.shift();
        if (waiter) waiter.reject(err);
        else values.push({ error: err });
      }
      function onAbort() {
        if (done) return;
        done = true;
        cancel(handle);
        cleanup();
        rejectNext(abortError());
      }
      if (signal) {
        if (signal.aborted) done = true;
        else signal.addEventListener('abort', onAbort);
      }
      if (!done) {
        handle = schedule(function () {
          settleNext({ done: false, value: value });
        }, delay || 0, [], normalizeMockDelay(delay) || 1, 'interval');
      }
      return {
        next: function () {
          if (signal && signal.aborted && done) return Promise.reject(abortError());
          if (values.length) {
            var result = values.shift();
            if (result && result.error) return Promise.reject(result.error);
            return Promise.resolve(result);
          }
          if (done) return Promise.reject(abortError());
          return new Promise(function (resolve, reject) {
            waiters.push({ resolve: resolve, reject: reject });
          });
        },
        return: function () {
          done = true;
          cancel(handle);
          cleanup();
          return Promise.resolve({ done: true, value: undefined });
        },
        [Symbol.asyncIterator]: function () { return this; },
      };
    }
    if (apis.indexOf('setTimeout') !== -1) {
      this._orig.setTimeout = g.setTimeout;
      this._orig.clearTimeout = g.clearTimeout;
      g.setTimeout = makeSet('timeout', false);
      g.clearTimeout = function (id) {
        cancel(id);
      };
      if (g.timers) {
        this._orig.timersSetTimeout = g.timers.setTimeout;
        this._orig.timersClearTimeout = g.timers.clearTimeout;
        g.timers.setTimeout = g.setTimeout;
        g.timers.clearTimeout = g.clearTimeout;
      }
      if (g.timers_promises) {
        this._orig.timersPromisesSetTimeout = g.timers_promises.setTimeout;
        g.timers_promises.setTimeout = mockPromiseTimeout;
      }
    }
    if (apis.indexOf('setInterval') !== -1) {
      this._orig.setInterval = g.setInterval;
      this._orig.clearInterval = g.clearInterval;
      g.setInterval = makeSet('interval', true);
      g.clearInterval = function (id) {
        cancel(id);
      };
      if (g.timers) {
        this._orig.timersSetInterval = g.timers.setInterval;
        this._orig.timersClearInterval = g.timers.clearInterval;
        g.timers.setInterval = g.setInterval;
        g.timers.clearInterval = g.clearInterval;
      }
      if (g.timers_promises) {
        this._orig.timersPromisesSetInterval = g.timers_promises.setInterval;
        g.timers_promises.setInterval = mockPromiseInterval;
      }
    }
    if (apis.indexOf('setImmediate') !== -1) {
      this._orig.setImmediate = g.setImmediate;
      this._orig.clearImmediate = g.clearImmediate;
      g.setImmediate = function (cb) {
        return schedule(cb, 0, Array.prototype.slice.call(arguments, 1), 0, 'immediate');
      };
      g.clearImmediate = function (id) {
        cancel(id);
      };
      if (g.timers) {
        this._orig.timersSetImmediate = g.timers.setImmediate;
        this._orig.timersClearImmediate = g.timers.clearImmediate;
        g.timers.setImmediate = g.setImmediate;
        g.timers.clearImmediate = g.clearImmediate;
      }
      if (g.timers_promises) {
        this._orig.timersPromisesSetImmediate = g.timers_promises.setImmediate;
        g.timers_promises.setImmediate = mockPromiseImmediate;
      }
    }
    if (apis.indexOf('Date') !== -1) {
      var OriginalDate = g.Date;
      if (OriginalDate && OriginalDate.isMock) throw invalidState('Date is already mocked');
      this._orig.Date = OriginalDate;
      function MockDate() {
        var args = Array.prototype.slice.call(arguments);
        if (!(this instanceof MockDate)) {
          if (args.length === 0) return OriginalDate(self._now).toString();
          return OriginalDate.apply(null, args);
        }
        if (args.length === 0) return new OriginalDate(self._now);
        switch (args.length) {
          case 1:
            return new OriginalDate(args[0]);
          case 2:
            return new OriginalDate(args[0], args[1]);
          case 3:
            return new OriginalDate(args[0], args[1], args[2]);
          case 4:
            return new OriginalDate(args[0], args[1], args[2], args[3]);
          case 5:
            return new OriginalDate(args[0], args[1], args[2], args[3], args[4]);
          case 6:
            return new OriginalDate(args[0], args[1], args[2], args[3], args[4], args[5]);
          default:
            return new OriginalDate(
              args[0],
              args[1],
              args[2],
              args[3],
              args[4],
              args[5],
              args[6]
            );
        }
      }
      MockDate.now = function () {
        return self._now;
      };
      MockDate.parse = OriginalDate.parse;
      MockDate.UTC = OriginalDate.UTC;
      MockDate.isMock = true;
      MockDate.toString = function () {
        return 'function Date() { [native code] }';
      };
      MockDate.prototype = OriginalDate.prototype;
      g.Date = MockDate;
    }
    if (apis.indexOf('scheduler.wait') !== -1) {
      var timersPromises = g.timers_promises;
      if (
        timersPromises &&
        timersPromises.scheduler &&
        typeof timersPromises.scheduler.wait === 'function'
      ) {
        this._orig.schedulerWait = timersPromises.scheduler.wait;
        timersPromises.scheduler.wait = function (delay, options) {
          var signal = options && options.signal;
          if (
            signal !== undefined &&
            (!signal ||
              (typeof signal !== 'object' && typeof signal !== 'function') ||
              typeof signal.aborted !== 'boolean' ||
              typeof signal.addEventListener !== 'function')
          ) {
            return Promise.reject(invalidArgType('options.signal', 'AbortSignal', signal));
          }
          return new Promise(function (resolve, reject) {
            var done = false;
            function abortError() {
              var e = new Error('The operation was aborted');
              e.name = 'AbortError';
              e.code = 'ABORT_ERR';
              return e;
            }
            function finish(kind) {
              if (done) return;
              done = true;
              if (kind === 'abort') reject(abortError());
              else resolve(undefined);
            }
            if (signal) {
              if (signal.aborted) {
                finish('abort');
                return;
              }
              signal.addEventListener('abort', function () {
                finish('abort');
              });
            }
            schedule(function () {
              finish('resolve');
            }, delay || 0, [], 0, 'timeout');
          });
        };
      }
    }
  };
  MockTimers.prototype._fireDue = function (upTo) {

    var guard = 0;
    while (guard++ < 1e6) {
      var next = null;
      for (var i = 0; i < this._queue.length; i++) {
        var t = this._queue[i];
        if (
          t.due <= upTo &&
          (next === null ||
            t.due < next.due ||
            (t.due === next.due &&
              ((t.kind === 'immediate' && next.kind !== 'immediate') ||
                (t.kind === next.kind && t.id < next.id))))
        ) {
          next = t;
        }
      }
      if (!next) break;
      this._queue = this._queue.filter(function (t) {
        return t !== next;
      });
      this._now = next.due;
      if (next.interval) {
        next.due = this._now + next.interval;
        this._queue.push(next);
      }
      next.cb.apply(null, next.args || []);
    }
    this._now = upTo;
  };
  MockTimers.prototype.tick = function (ms) {
    if (!this._enabled) throw invalidState('Mock timers are not enabled');
    if (ms !== undefined && Number(ms) < 0) throw invalidArgValue('ms', ms);
    this._fireDue(this._now + (ms || 0));
  };
  MockTimers.prototype.runAll = function () {
    if (!this._enabled) throw invalidState('Mock timers are not enabled');
    var max = this._now;
    this._queue.forEach(function (t) {
      if (t.due > max) max = t.due;
    });
    this._fireDue(max);
  };
  MockTimers.prototype.setTime = function (ms) {
    if (!this._enabled) throw invalidState('Mock timers are not enabled');
    this._now = ms;
  };
  MockTimers.prototype.reset = function () {
    if (!this._enabled) return;
    var g = globalThis;
    if (this._orig.schedulerWait && g.timers_promises && g.timers_promises.scheduler) {
      g.timers_promises.scheduler.wait = this._orig.schedulerWait;
    }
    if (this._orig.timersSetTimeout && g.timers) g.timers.setTimeout = this._orig.timersSetTimeout;
    if (this._orig.timersClearTimeout && g.timers) g.timers.clearTimeout = this._orig.timersClearTimeout;
    if (this._orig.timersSetInterval && g.timers) g.timers.setInterval = this._orig.timersSetInterval;
    if (this._orig.timersClearInterval && g.timers) g.timers.clearInterval = this._orig.timersClearInterval;
    if (this._orig.timersSetImmediate && g.timers) g.timers.setImmediate = this._orig.timersSetImmediate;
    if (this._orig.timersClearImmediate && g.timers) g.timers.clearImmediate = this._orig.timersClearImmediate;
    if (this._orig.timersPromisesSetTimeout && g.timers_promises) g.timers_promises.setTimeout = this._orig.timersPromisesSetTimeout;
    if (this._orig.timersPromisesSetInterval && g.timers_promises) g.timers_promises.setInterval = this._orig.timersPromisesSetInterval;
    if (this._orig.timersPromisesSetImmediate && g.timers_promises) g.timers_promises.setImmediate = this._orig.timersPromisesSetImmediate;
    for (var k in this._orig) {
      if (
        k !== 'schedulerWait' &&
        k !== 'timersSetTimeout' &&
        k !== 'timersClearTimeout' &&
        k !== 'timersSetInterval' &&
        k !== 'timersClearInterval' &&
        k !== 'timersSetImmediate' &&
        k !== 'timersClearImmediate' &&
        k !== 'timersPromisesSetTimeout' &&
        k !== 'timersPromisesSetInterval' &&
        k !== 'timersPromisesSetImmediate'
      ) {
        g[k] = this._orig[k];
      }
    }
    this._orig = {};
    this._queue = [];
    this._enabled = false;
    this._now = 0;
  };
  MockTimers.prototype[Symbol.dispose] = function () {
    this.reset();
  };

  function MockTracker() {
    this._mocks = [];
    this.timers = new MockTimers();
  }
  var activeModuleMocks = [];
  var originalRequireForModuleMocks = null;
  var moduleMockRequireInstalled = false;

  function normalizeMockModuleSpecifier(specifier) {
    if (typeof specifier === 'string') return specifier;
    if (specifier && typeof specifier === 'object' && typeof specifier.href === 'string') {
      return String(specifier.href);
    }
    throw invalidArgType('specifier', 'string', specifier);
  }

  function normalizeRequiredSpecifier(specifier) {
    var s = String(specifier);
    if (s.indexOf('node:') === 0) s = s.slice(5);
    if (s.indexOf('file://') === 0) {
      var rest = s.slice('file://'.length);
      var cut = rest.search(/[?#]/);
      if (cut !== -1) rest = rest.slice(0, cut);
      return rest;
    }
    return s;
  }

  function copyEnumerableProperties(target, source, skipName) {
    if (!source || (typeof source !== 'object' && typeof source !== 'function')) return;
    var names = Object.keys(source);
    for (var i = 0; i < names.length; i++) {
      var name = names[i];
      if (name === skipName) continue;
      try {
        Object.defineProperty(target, name, Object.getOwnPropertyDescriptor(source, name));
      } catch (_) {
        target[name] = source[name];
      }
    }
  }

  function materializeModuleMock(record, mode) {
    if (record.error) throw record.error;
    var exportsValue = record.hasExports ? record.options.exports : undefined;
    var namedExports = record.hasNamed ? record.options.namedExports : undefined;
    var defaultExport = record.hasDefault ? record.options.defaultExport : undefined;
    if (
      mode === 'cjs' &&
      !record.hasExports &&
      !record.hasNamed &&
      record.hasDefault
    ) {
      return defaultExport;
    }
    if (mode === 'cjs' && record.hasExports && exportsValue && Object.prototype.hasOwnProperty.call(exportsValue, 'default')) {
      var cjsDefault = exportsValue.default;
      if (cjsDefault && (typeof cjsDefault === 'object' || typeof cjsDefault === 'function')) {
        copyEnumerableProperties(cjsDefault, exportsValue, 'default');
        return cjsDefault;
      }
      return cjsDefault;
    }
    var out = {};
    if (record.hasExports) {
      copyEnumerableProperties(out, exportsValue);
      return out;
    }
    if (record.hasDefault) {
      if (
        mode === 'cjs' &&
        record.hasNamed &&
        (defaultExport === null ||
          (typeof defaultExport !== 'object' && typeof defaultExport !== 'function'))
      ) {
        throw new Error('Cannot create mock module from non-object default export');
      }
      copyEnumerableProperties(out, defaultExport);
    }
    if (
      mode === 'esm' &&
      record.hasNamed &&
      record.hasDefault &&
      record.normalized.slice(-3) === '.js' &&
      (defaultExport === null ||
        (typeof defaultExport !== 'object' && typeof defaultExport !== 'function'))
    ) {
      throw new Error('Cannot create mock module from non-object default export');
    }
    copyEnumerableProperties(out, namedExports);
    if (record.hasDefault && out.default === undefined) out.default = defaultExport;
    return out;
  }

  function findModuleMock(specifier) {
    var normalized = normalizeRequiredSpecifier(specifier);
    for (var i = activeModuleMocks.length - 1; i >= 0; i--) {
      var record = activeModuleMocks[i];
      if (!record.active) continue;
      if (record.normalized === normalized || record.specifier === normalized) return record;
      if (record.normalized.indexOf('/') !== -1 && normalized === record.normalized) return record;
    }
    return null;
  }

  function ensureModuleMockRequireWrapper() {
    if (moduleMockRequireInstalled) return;
    if (typeof globalThis.require !== 'function') return;
    originalRequireForModuleMocks = globalThis.require;
    var wrapped = function (specifier) {
      var record = findModuleMock(specifier);
      if (record) {
        if (record.cache && record.cachedValue !== undefined) return record.cachedValue;
        var value = materializeModuleMock(record);
        if (record.cache) record.cachedValue = value;
        return value;
      }
      return originalRequireForModuleMocks.apply(this, arguments);
    };
    try {
      Object.defineProperty(wrapped, 'resolve', Object.getOwnPropertyDescriptor(originalRequireForModuleMocks, 'resolve'));
    } catch (_) {
      wrapped.resolve = originalRequireForModuleMocks.resolve;
    }
    if (originalRequireForModuleMocks.cache !== undefined) wrapped.cache = originalRequireForModuleMocks.cache;
    if (originalRequireForModuleMocks.extensions !== undefined) wrapped.extensions = originalRequireForModuleMocks.extensions;
    globalThis.require = wrapped;
    moduleMockRequireInstalled = true;
  }

  function maybeRestoreModuleMockRequireWrapper() {
    for (var i = 0; i < activeModuleMocks.length; i++) {
      if (activeModuleMocks[i].active) return;
    }
    if (moduleMockRequireInstalled && originalRequireForModuleMocks) {
      globalThis.require = originalRequireForModuleMocks;
    }
    moduleMockRequireInstalled = false;
    originalRequireForModuleMocks = null;
    activeModuleMocks = [];
  }

  Object.defineProperty(globalThis, '__cruft_test_module_mock_require', {
    value: function (specifier, mode) {
      if ((mode || 'cjs') === 'cjs' && typeof specifier === 'string' && specifier.indexOf('file://') === 0) {
        var err = new Error("Cannot find module '" + specifier + "'");
        err.code = 'MODULE_NOT_FOUND';
        throw err;
      }
      var record = findModuleMock(specifier);
      if (!record) return { __cruftNoModuleMock: true };
      if (record.cache && record.cachedValue !== undefined) return record.cachedValue;
      var value = materializeModuleMock(record, mode || 'cjs');
      if (record.cache) record.cachedValue = value;
      return value;
    },
    writable: true,
    enumerable: false,
    configurable: true,
  });

  function validateMockOptions(options) {
    var times = options && options.times;
    if (times === undefined) return Infinity;
    if (typeof times !== 'number') throw invalidArgType('options.times', 'number', times);
    if (!Number.isInteger(times) || times < 1) throw outOfRange('options.times', times);
    return times;
  }
  MockTracker.prototype.fn = function (original, implementation, options) {

    if (original && typeof original === 'object') {
      options = original;
      original = undefined;
    }
    if (implementation && typeof implementation === 'object') {
      options = implementation;
      implementation = undefined;
    }
    original = typeof original === 'function' ? original : undefined;
    implementation = typeof implementation === 'function' ? implementation : undefined;
    var times = validateMockOptions(options);
    var ctx = { calls: [], impl: implementation, original: original, once: [], times: times };
    var mockFn = function () {
      var args = Array.prototype.slice.call(arguments);
      var idx = ctx.calls.length;
      var useImpl = idx >= ctx.times ? ctx.original : (ctx.once[idx] !== undefined ? ctx.once[idx] : ctx.impl || ctx.original);
      var isConstruct = !!new.target;
      var receiver = isConstruct ? this : (this === globalThis ? undefined : this);
      var target = isConstruct ? (ctx.original || mockFn) : undefined;
      var call = { arguments: args, this: receiver, result: undefined, error: undefined, target: target };
      ctx.calls.push(call);
      try {
        var r;
        if (isConstruct) {
          r = useImpl ? Reflect.construct(useImpl, args, ctx.original || useImpl) : this;
          if (r && (typeof r === 'object' || typeof r === 'function')) {
            try { Object.setPrototypeOf(r, (ctx.original || useImpl).prototype); } catch (_) {}
          }
          call.this = r && (typeof r === 'object' || typeof r === 'function') ? r : this;
          r = call.this;
        } else {
          r = useImpl ? useImpl.apply(this, args) : undefined;
        }
        call.result = r;
        return r;
      } catch (e) {
        if (isConstruct) call.this = undefined;
        call.error = e;
        throw e;
      }
    };
    var ctrl = {
      calls: ctx.calls,
      callCount: function () {
        return ctx.calls.length;
      },
      mockImplementation: function (fn) {
        ctx.impl = fn;
      },
      mockImplementationOnce: function (fn, onCall) {
        if (onCall !== undefined && onCall < ctx.calls.length) throw outOfRange('onCall', onCall);
        ctx.once[onCall === undefined ? ctx.calls.length : onCall] = fn;
      },
      resetCalls: function () {
        ctx.calls.length = 0;
      },
      restore: function () {
        ctx.impl = undefined;
      },
    };
    Object.defineProperty(mockFn, 'mock', { value: ctrl, enumerable: false });
    defineFunctionShape(mockFn, original || implementation);
    this._mocks.push(ctrl);
    return mockFn;
  };
  MockTracker.prototype.method = function (object, methodName, implementation, options) {
    if (implementation && typeof implementation === 'object') {
      options = implementation;
      implementation = undefined;
    }
    if (object === null || object === undefined) throw invalidArgType('object', 'Object', object);
    if (typeof methodName !== 'string' && typeof methodName !== 'symbol') throw invalidArgType('methodName', 'string or symbol', methodName);
    options = options || {};
    if (options.getter && options.setter) throw invalidArgValue('options.setter', options.setter);
    if (options.getter) return this.getter(object, methodName, implementation, options);
    if (options.setter) return this.setter(object, methodName, implementation, options);
    var found = findOwnOrInheritedDescriptor(object, methodName);
    if (!found) throw invalidArgValue('methodName', methodName);
    var desc = found.descriptor;
    var original = desc && desc.value;
    if (typeof original !== 'function') throw invalidArgValue('methodName', methodName);
    var mockFn = this.fn(original, implementation, options);
    Object.defineProperty(object, methodName, {
      configurable: desc.configurable,
      enumerable: desc.enumerable,
      writable: true,
      value: mockFn,
    });

    mockFn.mock.restore = function () {
      if (found && found.owner === object) {
        Object.defineProperty(object, methodName, desc);
      } else {
        Object.defineProperty(object, methodName, {
          configurable: desc.configurable,
          enumerable: desc.enumerable,
          writable: true,
          value: original,
        });
      }
    };
    return mockFn;
  };
  function findOwnOrInheritedDescriptor(object, name) {
    var cur = object;
    while (cur !== null && cur !== undefined) {
      var desc = Object.getOwnPropertyDescriptor(cur, name);
      if (desc) return { owner: cur, descriptor: desc };
      cur = Object.getPrototypeOf(cur);
    }
    return null;
  }
  MockTracker.prototype.getter = function (object, methodName, implementation, options) {
    if (implementation && typeof implementation === 'object') {
      options = implementation;
      implementation = undefined;
    }
    var found = findOwnOrInheritedDescriptor(object, methodName);
    var desc = found && found.descriptor;
    var original = desc && typeof desc.get === 'function' ? desc.get : undefined;
    if (!desc || typeof original !== 'function') throw invalidArgValue('methodName', methodName);
    var mockFn = this.fn(original, implementation, options);
    var next = {
      configurable: desc ? desc.configurable : true,
      enumerable: desc ? desc.enumerable : true,
      get: mockFn,
      set: desc && desc.set,
    };
    Object.defineProperty(object, methodName, next);
    mockFn.mock.restore = function () {
      if (found && found.owner === object) {
        Object.defineProperty(object, methodName, desc);
      } else {
        delete object[methodName];
      }
    };
    return mockFn;
  };
  MockTracker.prototype.setter = function (object, methodName, implementation, options) {
    if (implementation && typeof implementation === 'object') {
      options = implementation;
      implementation = undefined;
    }
    var found = findOwnOrInheritedDescriptor(object, methodName);
    var desc = found && found.descriptor;
    var original = desc && typeof desc.set === 'function' ? desc.set : undefined;
    if (!desc || typeof original !== 'function') throw invalidArgValue('methodName', methodName);
    var mockFn = this.fn(original, implementation, options);
    var next = {
      configurable: desc ? desc.configurable : true,
      enumerable: desc ? desc.enumerable : true,
      get: desc && desc.get,
      set: mockFn,
    };
    Object.defineProperty(object, methodName, next);
    mockFn.mock.restore = function () {
      if (found && found.owner === object) {
        Object.defineProperty(object, methodName, desc);
      } else {
        delete object[methodName];
      }
    };
    return mockFn;
  };
  MockTracker.prototype.property = function (object, propertyName, value) {
    if (object === null || object === undefined) throw invalidArgType('object', 'Object', object);
    if (typeof propertyName !== 'string' && typeof propertyName !== 'symbol') throw invalidArgType('propertyName', 'string or symbol', propertyName);
    var found = findOwnOrInheritedDescriptor(object, propertyName);
    if (!found) throw invalidArgValue('propertyName', propertyName);
    var desc = found.descriptor;
    var current = arguments.length >= 3 ? value : object[propertyName];
    var accesses = [];
    var once = {};
    var implSet = false;
    var implValue;
    function nextValue() {
      var idx = accesses.length;
      if (Object.prototype.hasOwnProperty.call(once, String(idx))) return once[String(idx)];
      if (implSet) return implValue;
      return current;
    }
    var ctrl = {
      accesses: accesses,
      accessCount: function () { return accesses.length; },
      resetAccesses: function () { accesses.length = 0; },
      mockImplementation: function (v) {
        implSet = true;
        implValue = v;
        accesses.push({ type: 'get', value: v });
      },
      mockImplementationOnce: function (v, onAccess) {
        if (onAccess !== undefined && onAccess < accesses.length) throw outOfRange('onAccess', onAccess);
        once[String(onAccess === undefined ? accesses.length : onAccess)] = v;
      },
      restore: function () {
        if (found && found.owner === object) Object.defineProperty(object, propertyName, desc);
        else delete object[propertyName];
      },
    };
    var prop = {};
    Object.defineProperty(prop, 'mock', { value: ctrl, enumerable: false });
    Object.defineProperty(object, propertyName, {
      configurable: true,
      enumerable: desc ? desc.enumerable : true,
      get: function () {
        var v = nextValue();
        accesses.push({ type: 'get', value: v });
        return v;
      },
      set: function (v) {
        if (desc && desc.writable === false) throw invalidArgValue('propertyName', propertyName);
        current = v;
        implSet = false;
        accesses.push({ type: 'set', value: v });
      },
    });
    this._mocks.push(ctrl);
    return prop;
  };
  MockTracker.prototype.module = function (specifier, options) {
    var spec = normalizeMockModuleSpecifier(specifier);
    if (options === undefined) options = {};
    if (!options || typeof options !== 'object') throw invalidArgType('options', 'object', options);
    if (options.cache !== undefined && typeof options.cache !== 'boolean') {
      throw invalidArgType('options.cache', 'boolean', options.cache);
    }
    var hasExports = Object.prototype.hasOwnProperty.call(options, 'exports');
    var hasNamed = Object.prototype.hasOwnProperty.call(options, 'namedExports');
    var hasDefault = Object.prototype.hasOwnProperty.call(options, 'defaultExport');
    if (hasNamed && (!options.namedExports || typeof options.namedExports !== 'object')) {
      throw invalidArgType('options.namedExports', 'object', options.namedExports);
    }
    if (hasExports && (!options.exports || typeof options.exports !== 'object')) {
      throw invalidArgType('options.exports', 'object', options.exports);
    }
    if (hasExports && hasNamed) throw invalidArgValue('options.exports', options.exports);
    if (hasExports && hasDefault) throw invalidArgValue('options.exports', options.exports);

    var normalized = normalizeRequiredSpecifier(spec);
    for (var mi = 0; mi < activeModuleMocks.length; mi++) {
      if (activeModuleMocks[mi].active && activeModuleMocks[mi].normalized === normalized) {
        throw invalidState('Module is already mocked');
      }
    }
    var record = {
      specifier: spec,
      normalized: normalized,
      cache: options.cache === true,
      options: options,
      hasNamed: hasNamed,
      hasDefault: hasDefault,
      hasExports: hasExports,
      cachedValue: undefined,
      active: true,
    };
    activeModuleMocks.push(record);
    ensureModuleMockRequireWrapper();
    var ctrl = {
      restore: function () {
        record.active = false;
        maybeRestoreModuleMockRequireWrapper();
      },
    };
    this._mocks.push(ctrl);
    return ctrl;
  };
  MockTracker.prototype.reset = function () {
    this._mocks.forEach(function (m) {
      if (m.restore) m.restore();
    });
    this._mocks = [];
  };
  MockTracker.prototype.restoreAll = function () {
    this._mocks.slice().reverse().forEach(function (m) {
      m.restore();
    });
    if (this.timers) this.timers.reset();
  };

  function makeTest(defaultOptions) {
    function test(name, options, fn) {
      var a = normalizeArgs(name, options, fn);
      for (var k in defaultOptions) if (a.options[k] === undefined) a.options[k] = defaultOptions[k];
      validateTestOptions(a.options);
      a.options.__loc = captureLocation();
      if (a.options.only) hasOnly = true;
      var parent = currentSuite;
      var node = new TestNode(a.name, a.options, a.fn, parent);
      parent.children.push(node);
      scheduleDrain();
      return node.donePromise ? node.donePromise() : Promise.resolve();
    }
    return test;
  }

  function makeDescribe(defaultOptions) {
    function describe(name, options, fn) {
      var a = normalizeArgs(name, options, fn);
      for (var k in defaultOptions) if (a.options[k] === undefined) a.options[k] = defaultOptions[k];
      validateTestOptions(a.options);
      a.options.__loc = captureLocation();
      var parent = currentSuite;
      var suite = new TestNode(a.name, a.options, null, parent);
      suite.isDescribe = true;
      parent.children.push(suite);

      var prev = currentSuite;
      currentSuite = suite;
      try {
        if (a.fn && !a.options.skip) {
          var ctx = new TestContext(suite);
          a.fn.call(ctx, ctx);
        }
      } finally {
        currentSuite = prev;
      }
      scheduleDrain();
      return suite.donePromise ? suite.donePromise() : Promise.resolve();
    }
    return describe;
  }

  var test = makeTest({});
  test.skip = makeTest({ skip: true });
  test.todo = makeTest({ todo: true });
  test.only = makeTest({ only: true });

  var describe = makeDescribe({});
  describe.skip = makeDescribe({ skip: true });
  describe.todo = makeDescribe({ todo: true });
  describe.only = makeDescribe({ only: true });

  var it = test;

  function before(fn) {
    currentSuite.hooks.before.push(makeHookEntry(fn, currentSuite));
  }
  function after(fn) {
    currentSuite.hooks.after.push(makeHookEntry(fn, currentSuite));
  }
  function beforeEach(fn) {
    currentSuite.hooks.beforeEach.push(makeHookEntry(fn, currentSuite));
  }
  function afterEach(fn) {
    currentSuite.hooks.afterEach.push(makeHookEntry(fn, currentSuite));
  }

  function makeTestsStream() {
    var EE = require('events').EventEmitter;
    var stream = new EE();
    stream._buffer = [];
    stream._waiters = [];
    stream._ended = false;
    stream._event = function (type, data) {
      this.emit(type, data);
      var rec = { type: type, data: data };
      if (this._waiters.length) this._waiters.shift()({ value: rec, done: false });
      else this._buffer.push(rec);
    };
    stream._finish = function (failed) {
      this.emit('test:complete', { passed: !failed });
      this._ended = true;
      while (this._waiters.length) this._waiters.shift()({ value: undefined, done: true });
    };
    stream.resume = function () {
      return this;
    };
    stream.compose = function (transform) {
      var source = this;
      var iterable = typeof transform === 'function' ? transform(source) : source;
      return {
        on: function () {
          return this;
        },
        resume: function () {
          return this;
        },
        [Symbol.asyncIterator]: function () {
          return iterable && iterable[Symbol.asyncIterator]
            ? iterable[Symbol.asyncIterator]()
            : source[Symbol.asyncIterator]();
        },
      };
    };
    stream[Symbol.asyncIterator] = function () {
      var self = this;
      return {
        next: function () {
          if (self._buffer.length)
            return Promise.resolve({ value: self._buffer.shift(), done: false });
          if (self._ended) return Promise.resolve({ value: undefined, done: true });
          return new Promise(function (resolve) {
            self._waiters.push(resolve);
          });
        },
      };
    };
    return stream;
  }
  function flushPendingTapFailure(stream, parser) {
    if (!parser.pendingFail) return;
    stream._event('test:fail', parser.pendingFail);
    stream._event('test:complete', {
      name: parser.pendingFail.name,
      nesting: parser.pendingFail.nesting,
      testNumber: parser.pendingFail.testNumber,
      testId: parser.pendingFail.testId,
      file: parser.pendingFail.file,
      passed: false,
      details: parser.pendingFail.details,
    });
    parser.pendingFail = null;
    parser.inFailureBlock = false;
  }
  function parseTapDetailValue(v) {
    v = String(v == null ? '' : v).trim();
    if ((v[0] === "'" && v[v.length - 1] === "'") || (v[0] === '"' && v[v.length - 1] === '"')) {
      v = v.slice(1, -1);
    }
    return v;
  }
  function parseTapLine(line, stream, file, parser) {
    var m = line.match(/^(\s*)(ok|not ok)\s+(\d+)\s+-\s+(.*?)(\s+#\s+(SKIP|TODO|EXPECTED FAILURE).*)?$/);
    if (m) {
      flushPendingTapFailure(stream, parser);
      var passed = m[2] === 'ok';
      var nesting = Math.floor(m[1].length / 2);
      var directive = m[6];
      parser.counter.n++;
      var testId = parser.counter.n;
      var data = {
        name: m[4],
        nesting: nesting,
        testNumber: parser.counter.n,
        testId: testId,
        skip: directive === 'SKIP',
        todo: directive === 'TODO',
        expectFailure: directive === 'EXPECTED FAILURE',
        file: file,
        details: { duration_ms: 0 },
      };
      stream._event('test:start', {
        name: data.name,
        nesting: data.nesting,
        testNumber: data.testNumber,
        testId: data.testId,
        file: data.file,
      });
      if (passed) {
        stream._event('test:pass', data);
        stream._event('test:complete', {
          name: data.name,
          nesting: data.nesting,
          testNumber: data.testNumber,
          testId: data.testId,
          file: data.file,
          passed: true,
          details: data.details,
        });
      } else {
        parser.pendingFail = data;
      }
      return;
    }
    if (parser.pendingFail) {
      var trimmed = line.trim();
      if (trimmed === '---') {
        parser.inFailureBlock = true;
        return;
      }
      if (trimmed === '...') {
        flushPendingTapFailure(stream, parser);
        return;
      }
      if (parser.inFailureBlock) {
        var dm = trimmed.match(/^duration_ms:\s*(.*)$/);
        if (dm) {
          var dur = Number(parseTapDetailValue(dm[1]));
          if (isFinite(dur)) parser.pendingFail.details.duration_ms = dur;
          return;
        }
        var em = trimmed.match(/^error:\s*(.*)$/);
        if (em) {
          parser.pendingFail.details.error = parser.pendingFail.details.error || {};
          parser.pendingFail.details.error.message = parseTapDetailValue(em[1]);
          return;
        }
        var cm = trimmed.match(/^code:\s*(.*)$/);
        if (cm) {
          parser.pendingFail.details.error = parser.pendingFail.details.error || {};
          parser.pendingFail.details.error.code = parseTapDetailValue(cm[1]);
          return;
        }
      }
    }
    var d = line.match(/^\s*# (.*)$/);
    if (d && !/^(Subtest|tests|suites|pass|fail|cancelled|skipped|todo|duration_ms)\b/.test(d[1])) {
      flushPendingTapFailure(stream, parser);
      stream._event('test:diagnostic', { message: d[1], file: file });
    }
  }
  function run(options) {
    options = options || {};
    if (Array.isArray(options.testTagFilters) && options.testTagFilters.length) {
      emitTagsExperimentalWarning();
    }
    var files = (options.files || []).slice();
    var stream = makeTestsStream();
    var counter = { n: 0 };
    var cp;
    try {
      cp = require('child_process');
    } catch (e) {
      cp = null;
    }
    if (!files.length || !cp) {
      if (globalThis.queueMicrotask)
        queueMicrotask(function () {
          stream._finish(false);
        });
      return stream;
    }
    if (options.isolation === 'none') {
      queueMicrotask(async function () {
        var anyImportFail = false;
        for (var i = 0; i < files.length; i++) {
          var file = String(files[i]);
          stream._event('test:enqueue', { file: file });
          try {
            await import(file.indexOf('file://') === 0 ? file : 'file://' + file);
          } catch (e) {
            anyImportFail = true;
            counter.n++;
            stream._event('test:fail', {
              name: file,
              nesting: 0,
              testNumber: counter.n,
              file: file,
              details: {
                duration_ms: 0,
                error: e,
              },
            });
          }
        }
        stream._finish(anyImportFail);
      });
      return stream;
    }
    var pending = files.length;
    var anyFail = false;
    var extraArgs = [];
    if (options.testNamePatterns) {
      var pats = [].concat(options.testNamePatterns);
      if (pats[0]) extraArgs.push('--test-name-pattern=' + pats[0]);
    }
    var timeoutMs =
      typeof options.timeout === 'number' && isFinite(options.timeout) && options.timeout > 0
        ? Math.floor(options.timeout)
        : 0;
    var concurrency = files.length;
    if (typeof options.concurrency === 'number' && isFinite(options.concurrency)) {
      concurrency = Math.max(1, Math.floor(options.concurrency));
    } else if (options.concurrency === false) {
      concurrency = 1;
    }
    if (concurrency > files.length) concurrency = files.length;
    var signal = options.signal && typeof options.signal === 'object' ? options.signal : null;
    var children = [];
    var aborted = false;
    var watchMode = !!options.watch;
    var watchers = [];
    var watchArmed = false;
    var watchRunning = false;
    var watchDirty = false;
    var abortMessage = 'test run aborted';
    function cleanupAbortListener() {
      if (signal && onAbort && typeof signal.removeEventListener === 'function') {
        signal.removeEventListener('abort', onAbort);
      }
    }
    function closeWatchers() {
      for (var i = 0; i < watchers.length; i++) {
        try {
          if (watchers[i] && typeof watchers[i].close === 'function') watchers[i].close();
        } catch (e) {}
      }
      watchers = [];
      watchArmed = false;
    }
    var finishStream = function () {
      cleanupAbortListener();
      closeWatchers();
      stream._finish(anyFail);
    };
    function scheduleWatchRun() {
      if (aborted || !watchMode || !watchArmed) return;
      if (watchRunning) {
        watchDirty = true;
        return;
      }
      watchRunning = true;
      watchDirty = false;
      pending = files.length;
      nextFile = 0;
      launchMore();
    }
    function armWatchers() {
      if (!watchMode || watchArmed) return;
      var fs = null;
      try {
        fs = require('node:fs');
      } catch (e) {
        fs = null;
      }
      if (!fs || typeof fs.watch !== 'function') return;
      watchArmed = true;
      for (var i = 0; i < files.length; i++) {
        try {
          watchers.push(fs.watch(files[i], function () {
            scheduleWatchRun();
          }));
        } catch (e) {}
      }
    }
    function emitAbortFailure(state) {
      if (!state || state.sawEvent) return;
      state.sawEvent = true;
      counter.n++;
      stream._event('test:fail', {
        name: state.file,
        nesting: 0,
        testNumber: counter.n,
        file: state.file,
        details: {
          duration_ms: 0,
          error: {
            name: 'AbortError',
            message: abortMessage,
          },
        },
      });
    }
    var onAbort = function () {
      if (aborted) return;
      aborted = true;
      var abortsActiveWork = active > 0 || pending > 0;
      if (abortsActiveWork) anyFail = true;
      for (var i = 0; i < children.length; i++) {
        var state = children[i];
        if (!state.child) continue;
        state.aborted = true;
        if (abortsActiveWork) emitAbortFailure(state);
        if (state.child && typeof state.child.kill === 'function') state.child.kill();
      }
      while (nextFile < files.length) {
        emitAbortFailure({ file: files[nextFile++], sawEvent: false });
        pending--;
      }
      if (pending === 0) finishStream();
    };
    var nextFile = 0;
    var active = 0;
    function launchMore() {
      while (!aborted && nextFile < files.length && active < concurrency) {
        spawnFile(files[nextFile++]);
      }
    }
    if (signal) {
      if (signal.aborted) onAbort();
      else if (typeof signal.addEventListener === 'function') signal.addEventListener('abort', onAbort);
      else if ('onabort' in signal && !signal.onabort) signal.onabort = onAbort;
    }
    function spawnFile(file) {
      active++;
      var args = ['--test', '--test-reporter=tap'].concat(extraArgs, [file]);
      var child = cp.spawn(process.execPath, args, {});
      var state = { file: file, child: child, sawEvent: false, aborted: false };
      children.push(state);
      var buf = '';
      var tapParser = { counter: counter, pendingFail: null, inFailureBlock: false };
      var timedOut = false;
      var timeoutId = null;
      if (timeoutMs && globalThis.setTimeout) {
        timeoutId = setTimeout(function () {
          timedOut = true;
          anyFail = true;
          if (!state.sawEvent) {
            counter.n++;
            state.sawEvent = true;
            stream._event('test:fail', {
              name: file,
              nesting: 0,
              testNumber: counter.n,
              file: file,
              details: {
                duration_ms: timeoutMs,
                error: {
                  name: 'TimeoutError',
                  message: 'test timed out after ' + timeoutMs + 'ms',
                },
              },
            });
          }
          if (child && typeof child.kill === 'function') child.kill();
        }, timeoutMs);
      }
      child.stdout.on('data', function (chunk) {
        buf += chunk.toString();
        var lines = buf.split('\n');
        buf = lines.pop();
        lines.forEach(function (line) {
          var before = counter.n;
          parseTapLine(line, stream, file, tapParser);
          if (counter.n !== before) state.sawEvent = true;
        });
      });
      child.on('exit', function (code) {
        if (timeoutId !== null && globalThis.clearTimeout) clearTimeout(timeoutId);
        if (buf) {
          var before = counter.n;
          parseTapLine(buf, stream, file, tapParser);
          if (counter.n !== before) state.sawEvent = true;
        }
        flushPendingTapFailure(stream, tapParser);
        if (code || timedOut || state.aborted) anyFail = true;
        active--;
        pending--;
        if (pending === 0) {
          if (watchMode && !aborted) {
            watchRunning = false;
            armWatchers();
            stream._event('test:watch:drained', { files: files.slice() });
            if (watchDirty) scheduleWatchRun();
          } else {
            finishStream();
          }
        } else launchMore();
      });
      if (aborted) {
        state.aborted = true;
        emitAbortFailure(state);
        if (child && typeof child.kill === 'function') child.kill();
      }
    }
    watchRunning = watchMode;
    launchMore();
    return stream;
  }

  test.test = test;
  test.describe = describe;
  test.suite = describe;
  test.it = it;
  test.before = before;
  test.after = after;
  test.beforeEach = beforeEach;
  test.afterEach = afterEach;
  test.getTestContext = getTestContext;
  test.default = test;
  test.assert = testAssertions;

  test.mock = new MockTracker();
  test.run = run;
  test.snapshot = snapshot;

  async function* tapReporter(source) {
    yield 'TAP version 13\n';
    var n = 0;
    for await (var ev of source) {
      if (ev.type === 'test:pass' || ev.type === 'test:fail') {
        var ok = ev.type === 'test:pass' ? 'ok' : 'not ok';
        var dir = ev.data.skip ? ' # SKIP' : ev.data.todo ? ' # TODO' : '';
        yield ok + ' ' + ++n + ' - ' + ev.data.name + dir + '\n';
      } else if (ev.type === 'test:diagnostic') {
        yield '# ' + ev.data.message + '\n';
      }
    }
    yield '1..' + n + '\n';
  }
  async function* specReporter(source) {
    for await (var ev of source) {
      if (ev.type === 'test:pass') yield (ev.data.skip ? '﹣' : '✔') + ' ' + ev.data.name + '\n';
      else if (ev.type === 'test:fail') yield '✖ ' + ev.data.name + '\n';
      else if (ev.type === 'test:diagnostic') yield 'ℹ ' + ev.data.message + '\n';
    }
  }
  async function* dotReporter(source) {
    var line = '';
    for await (var ev of source) {
      if (ev.type === 'test:pass') line += '.';
      else if (ev.type === 'test:fail') line += 'X';
    }
    yield line + '\n';
  }
  async function* junitReporter(source) {
    var cases = [];
    for await (var ev of source) {
      if (ev.type === 'test:pass' || ev.type === 'test:fail') {
        cases.push(
          '  <testcase name="' + ev.data.name + '">' +
            (ev.type === 'test:fail' ? '<failure/>' : '') +
            '</testcase>'
        );
      }
    }
    yield '<?xml version="1.0"?>\n<testsuite tests="' + cases.length + '">\n' +
      cases.join('\n') + '\n</testsuite>\n';
  }
  async function* lcovReporter(source) {
    for await (var ev of source) {
      void ev;
    }
    yield '';
  }
  var reporters = {
    tap: tapReporter,
    spec: specReporter,
    dot: dotReporter,
    junit: junitReporter,
    lcov: lcovReporter,
  };
  Object.defineProperty(globalThis, '__cruft_test_reporters', {
    value: reporters,
    writable: true,
    enumerable: false,
    configurable: true,
  });
  Object.defineProperty(globalThis, '__cruft_internal_test_runner_snapshot', {
    value: internalSnapshot,
    writable: true,
    enumerable: false,
    configurable: true,
  });
  var internalUtils = {
    convertStringToRegExp: function (value, name) {
      var match = String(value).match(/^\/(.*)\/([a-z]+)$/i);
      if (!match) return new RegExp(String(value));
      try {
        if (/[^dgimsuvy]/.test(match[2])) {
          throw new SyntaxError("Invalid flags supplied to RegExp constructor '" + match[2] + "'");
        }
        return new RegExp(match[1], match[2]);
      } catch (err) {
        var e = new TypeError(
          "The argument '" + String(name) + "' is an invalid regular expression. " +
            err.message + ". Received '" + String(value) + "'"
        );
        e.code = 'ERR_INVALID_ARG_VALUE';
        throw e;
      }
    },
  };
  Object.defineProperty(globalThis, '__cruft_internal_test_runner_utils', {
    value: internalUtils,
    writable: true,
    enumerable: false,
    configurable: true,
  });
  Object.defineProperty(globalThis, '__cruft_internal_timers', {
    value: { TIMEOUT_MAX: 2147483647 },
    writable: true,
    enumerable: false,
    configurable: true,
  });

  Object.defineProperty(globalThis, '__cruft_test', {
    value: kernel,
    writable: true,
    enumerable: false,
    configurable: true,
  });

  Object.defineProperty(globalThis, '__cruft_test_run_all', {
    value: runAll,
    writable: true,
    enumerable: false,
    configurable: true,
  });

  Object.defineProperty(globalThis, '__cruft_node_test', {
    value: test,
    writable: true,
    enumerable: false,
    configurable: true,
  });
})();
