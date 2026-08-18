
(function () {
  'use strict';
  const DEFAULT_HWM = 1;

  function extractHWM(strategy, fallback) {
    if (strategy && strategy.highWaterMark !== undefined) {
      const hwm = Number(strategy.highWaterMark);
      if (Number.isNaN(hwm) || hwm < 0) throw new RangeError('Invalid highWaterMark');
      return hwm;
    }
    return fallback;
  }
  function extractSizeAlgo(strategy) {
    if (strategy && typeof strategy.size === 'function') return (chunk) => Number(strategy.size(chunk));
    return () => 1;
  }

  class ReadableStreamDefaultController {
    constructor(stream, source, strategy) {
      this._stream = stream;
      this._queue = [];
      this._queueTotalSize = 0;
      this._hwm = extractHWM(strategy, DEFAULT_HWM);
      this._sizeAlgo = extractSizeAlgo(strategy);
      this._closeRequested = false;
      this._pulling = false;
      this._pullAgain = false;
      this._started = false;
      this._pull = source && source.pull ? (c) => source.pull(c) : () => {};
      this._cancel = source && source.cancel ? (r) => source.cancel(r) : () => {};
      const startRes = source && source.start ? source.start(this) : undefined;
      Promise.resolve(startRes).then(
        () => { this._started = true; this._callPullIfNeeded(); },
        (e) => this.error(e)
      );
    }
    get desiredSize() {
      const s = this._stream._state;
      if (s === 'errored') return null;
      if (s === 'closed') return 0;
      return this._hwm - this._queueTotalSize;
    }

    _dequeue() {
      const entry = this._queue.shift();
      this._queueTotalSize -= entry.size;
      if (this._queueTotalSize < 0) this._queueTotalSize = 0;
      return entry.value;
    }
    _callPullIfNeeded() {
      if (!this._started) return;
      const stream = this._stream;
      if (stream._state !== 'readable') return;
      if (this._closeRequested) return;

      const wantsMore = (stream._reader && stream._reader._readRequests.length > 0) || this.desiredSize > 0;
      if (!wantsMore) return;
      if (this._pulling) { this._pullAgain = true; return; }
      this._pulling = true;
      Promise.resolve(this._pull(this)).then(
        () => {
          this._pulling = false;
          if (this._pullAgain) { this._pullAgain = false; this._callPullIfNeeded(); }
        },
        (e) => this.error(e)
      );
    }
    enqueue(chunk) {
      const stream = this._stream;
      if (stream._state !== 'readable') throw new TypeError('Cannot enqueue: stream is not readable');
      const reader = stream._reader;
      if (reader && reader._readRequests.length > 0) {
        const req = reader._readRequests.shift();
        req.resolve({ value: chunk, done: false });
      } else {
        let size = 1;
        try { size = this._sizeAlgo(chunk); } catch (e) { this.error(e); throw e; }
        this._queue.push({ value: chunk, size });
        this._queueTotalSize += size;
      }
      this._callPullIfNeeded();
    }
    close() {
      const stream = this._stream;
      if (stream._state !== 'readable') return;
      this._closeRequested = true;
      if (this._queue.length === 0) this._closeStream();
    }
    _closeStream() {
      const stream = this._stream;
      stream._state = 'closed';
      const reader = stream._reader;
      if (reader) {
        for (const req of reader._readRequests) req.resolve({ value: undefined, done: true });
        reader._readRequests = [];
        reader._closedResolve && reader._closedResolve();
      }
    }
    error(e) {
      const stream = this._stream;
      if (stream._state !== 'readable') return;
      stream._state = 'errored';
      stream._storedError = e;
      this._queue = [];
      this._queueTotalSize = 0;
      const reader = stream._reader;
      if (reader) {
        for (const req of reader._readRequests) req.reject(e);
        reader._readRequests = [];
        reader._closedReject && reader._closedReject(e);
      }
    }
  }

  class ReadableStreamDefaultReader {
    constructor(stream) {
      if (stream._reader) throw new TypeError('ReadableStream is already locked to a reader');
      this._stream = stream;
      this._readRequests = [];
      stream._reader = this;
      const self = this;
      this._closedPromise = new Promise((res, rej) => { self._closedResolve = res; self._closedReject = rej; });

      this._closedPromise.catch(() => {});
      if (stream._state === 'closed') this._closedResolve();
      else if (stream._state === 'errored') this._closedReject(stream._storedError);
    }
    get closed() { return this._closedPromise; }
    read() {
      const stream = this._stream;
      if (!stream) return Promise.reject(new TypeError('Reader has no associated stream'));
      if (stream._state === 'errored') return Promise.reject(stream._storedError);
      const ctrl = stream._controller;

      if (stream._isByteStream) {
        if (ctrl._totalBytes > 0) {
          const chunk = ctrl._shiftChunk();
          if (ctrl._totalBytes === 0 && ctrl._closeRequested) ctrl._closeStream();
          else ctrl._callPullIfNeeded();
          return Promise.resolve({ value: chunk, done: false });
        }
        if (stream._state === 'closed') return Promise.resolve({ value: undefined, done: true });
        const self2 = this;
        const pb = new Promise((resolve, reject) => { self2._readRequests.push({ resolve, reject }); });
        ctrl._callPullIfNeeded();
        return pb;
      }
      if (ctrl._queue.length > 0) {
        const chunk = ctrl._dequeue();
        if (ctrl._queue.length === 0 && ctrl._closeRequested) ctrl._closeStream();
        else ctrl._callPullIfNeeded();
        return Promise.resolve({ value: chunk, done: false });
      }
      if (stream._state === 'closed') return Promise.resolve({ value: undefined, done: true });
      const self = this;
      const p = new Promise((resolve, reject) => { self._readRequests.push({ resolve, reject }); });
      ctrl._callPullIfNeeded();
      return p;
    }
    releaseLock() {
      const stream = this._stream;
      if (!stream) return;
      for (const req of this._readRequests) req.reject(new TypeError('Reader was released'));
      this._readRequests = [];
      stream._reader = undefined;
      this._stream = undefined;
    }
    cancel(reason) {
      if (!this._stream) return Promise.reject(new TypeError('Reader has no associated stream'));

      return this._stream._cancelInternal(reason);
    }
  }

  class ReadableStream {
    constructor(source = {}, strategy = {}) {
      this._state = 'readable';
      this._storedError = undefined;
      this._reader = undefined;

      this._isByteStream = !!(source && source.type === 'bytes');
      this._controller = this._isByteStream
        ? new ReadableByteStreamController(this, source, strategy)
        : new ReadableStreamDefaultController(this, source, strategy);
    }
    get locked() { return this._reader !== undefined; }
    getReader(opts) {
      if (opts && opts.mode === 'byob') {
        if (!this._isByteStream) throw new TypeError('Cannot get a BYOB reader for a non-byte stream');
        return new ReadableStreamBYOBReader(this);
      }
      return new ReadableStreamDefaultReader(this);
    }
    cancel(reason) {
      if (this.locked) return Promise.reject(new TypeError('Cannot cancel a locked stream'));
      return this._cancelInternal(reason);
    }
    _cancelInternal(reason) {
      if (this._state === 'closed') return Promise.resolve();
      if (this._state === 'errored') return Promise.reject(this._storedError);
      this._controller._queue = [];
      this._controller._queueTotalSize = 0;
      this._controller._closeStream();
      return Promise.resolve(this._controller._cancel(reason)).then(() => undefined);
    }
    tee() {
      const reader = this.getReader();
      let reading = false, canceled1 = false, canceled2 = false;
      let branch1, branch2;
      let resolveCancel;
      const cancelPromise = new Promise((r) => { resolveCancel = r; });
      const pull = () => {
        if (reading) return Promise.resolve();
        reading = true;
        return reader.read().then(({ value, done }) => {
          reading = false;
          if (done) {
            if (!canceled1) branch1._controller.close();
            if (!canceled2) branch2._controller.close();
            return;
          }
          if (!canceled1) branch1._controller.enqueue(value);
          if (!canceled2) branch2._controller.enqueue(value);
        });
      };
      const cancel1 = (reason) => {
        canceled1 = true;
        if (canceled2) { reader.cancel(reason); resolveCancel(); }
        return cancelPromise;
      };
      const cancel2 = (reason) => {
        canceled2 = true;
        if (canceled1) { reader.cancel(reason); resolveCancel(); }
        return cancelPromise;
      };
      branch1 = new ReadableStream({ pull, cancel: cancel1 });
      branch2 = new ReadableStream({ pull, cancel: cancel2 });
      return [branch1, branch2];
    }

    pipeTo(dest, opts) {
      opts = opts || {};
      const preventClose = !!opts.preventClose;
      const preventAbort = !!opts.preventAbort;
      const preventCancel = !!opts.preventCancel;
      const signal = opts.signal;
      const source = this;
      const reader = source.getReader();
      const writer = dest.getWriter();
      let shuttingDown = false;
      return new Promise((resolve, reject) => {
        const finalize = (isError, error) => {
          if (signal && typeof signal.removeEventListener === 'function' && abortAlgorithm) {
            signal.removeEventListener('abort', abortAlgorithm);
          }
          writer.releaseLock();
          reader.releaseLock();
          if (isError) reject(error); else resolve();
        };
        const shutdownWith = (action, isError, error) => {
          if (shuttingDown) return;
          shuttingDown = true;
          Promise.resolve()
            .then(action)
            .then(() => finalize(isError, error), (e) => finalize(true, e));
        };
        const shutdown = (isError, error) => {
          if (shuttingDown) return;
          shuttingDown = true;
          finalize(isError, error);
        };

        let abortAlgorithm;
        if (signal) {
          abortAlgorithm = () => {
            const error = signal.reason !== undefined
              ? signal.reason
              : new DOMException('The operation was aborted', 'AbortError');
            const actions = [];
            if (!preventAbort) actions.push(() => writer.abort(error));
            if (!preventCancel) actions.push(() => reader.cancel(error));
            shutdownWith(() => Promise.all(actions.map((a) => a())), true, error);
          };
          if (signal.aborted) { abortAlgorithm(); return; }
          if (typeof signal.addEventListener === 'function') {
            signal.addEventListener('abort', abortAlgorithm);
          }
        }

        const step = () => {
          if (shuttingDown) return;
          reader.read().then(({ value, done }) => {
            if (shuttingDown) return;
            if (done) {

              if (!preventClose) shutdownWith(() => writer.close(), false);
              else shutdown(false);
              return;
            }

            Promise.resolve(writer.ready).then(() => {
              if (shuttingDown) return;
              writer.write(value).then(undefined, (e) => {

                if (!preventCancel) shutdownWith(() => reader.cancel(e), true, e);
                else shutdown(true, e);
              });
              step();
            }, (e) => {

              if (!preventCancel) shutdownWith(() => reader.cancel(e), true, e);
              else shutdown(true, e);
            });
          }, (e) => {

            if (!preventAbort) shutdownWith(() => writer.abort(e), true, e);
            else shutdown(true, e);
          });
        };
        step();
      });
    }
    pipeThrough(transform, opts) {

      this.pipeTo(transform.writable, opts).then(undefined, () => {});
      return transform.readable;
    }
    static from(iterable) {
      let iter;
      if (iterable && typeof iterable[Symbol.asyncIterator] === 'function') iter = iterable[Symbol.asyncIterator]();
      else if (iterable && typeof iterable[Symbol.iterator] === 'function') iter = iterable[Symbol.iterator]();
      else throw new TypeError('ReadableStream.from: argument is not iterable');
      return new ReadableStream({
        async pull(controller) {
          const { value, done } = await iter.next();
          if (done) controller.close();
          else controller.enqueue(value);
        },
        async cancel(reason) { if (iter.return) await iter.return(reason); }
      });
    }
  }
  ReadableStream.prototype[Symbol.asyncIterator] = function (opts) {
    const reader = this.getReader();
    const preventCancel = !!(opts && opts.preventCancel);
    return {
      next() {
        return reader.read().then((r) => {
          if (r.done) reader.releaseLock();
          return r;
        });
      },
      return(value) {
        if (!preventCancel) reader.cancel(value);
        reader.releaseLock();
        return Promise.resolve({ value, done: true });
      },
      [Symbol.asyncIterator]() { return this; }
    };
  };
  ReadableStream.prototype.values = ReadableStream.prototype[Symbol.asyncIterator];

  class WritableStreamDefaultController {
    constructor(stream, sink, strategy) {
      this._stream = stream;
      this._sink = sink;
      this._hwm = extractHWM(strategy, 1);
      this._sizeAlgo = extractSizeAlgo(strategy);
      this._queueTotalSize = 0;
    }
    get desiredSize() { return this._hwm - this._queueTotalSize; }
    error(e) {
      if (this._stream._state === 'writable') {
        this._stream._state = 'errored';
        this._stream._storedError = e;
      }
    }
  }

  class WritableStreamDefaultWriter {
    constructor(stream) {
      if (stream._writer) throw new TypeError('WritableStream is already locked to a writer');
      this._stream = stream;
      stream._writer = this;
    }
    get closed() { return this._stream._closedPromise; }
    write(chunk) {
      const stream = this._stream;
      if (stream._state === 'errored') return Promise.reject(stream._storedError);
      const ctrl = stream._controller;

      let size = 1;
      try { size = ctrl._sizeAlgo(chunk); } catch (e) {   }
      ctrl._queueTotalSize += size;
      stream._updateBackpressure();
      stream._writePromise = stream._writePromise.then(() => {
        if (stream._state !== 'writable') return;
        return ctrl._sink.write ? ctrl._sink.write(chunk, ctrl) : undefined;
      }).then(() => {
        ctrl._queueTotalSize -= size;
        if (ctrl._queueTotalSize < 0) ctrl._queueTotalSize = 0;
        stream._updateBackpressure();
      }, (e) => {
        ctrl._queueTotalSize -= size;
        if (ctrl._queueTotalSize < 0) ctrl._queueTotalSize = 0;
        stream._updateBackpressure();
        throw e;
      });
      return stream._writePromise;
    }
    close() {
      const stream = this._stream;
      return stream._writePromise.then(() => {
        if (stream._state !== 'writable') return;
        stream._state = 'closed';
        const r = stream._controller._sink.close ? stream._controller._sink.close() : undefined;
        return Promise.resolve(r).then(() => { stream._closedResolve(); });
      });
    }
    abort(reason) {
      const stream = this._stream;
      if (stream._state === 'closed' || stream._state === 'errored') return Promise.resolve();
      stream._state = 'errored';
      const r = stream._controller._sink.abort ? stream._controller._sink.abort(reason) : undefined;
      return Promise.resolve(r).then(() => undefined);
    }
    releaseLock() {
      const stream = this._stream;
      if (!stream) return;
      stream._writer = undefined;
      this._stream = undefined;
    }
    get desiredSize() { return this._stream._controller.desiredSize; }
    get ready() { return this._stream._readyPromise; }
  }

  class WritableStream {
    constructor(sink = {}, strategy = {}) {
      this._state = 'writable';
      this._storedError = undefined;
      this._writer = undefined;
      this._writePromise = Promise.resolve();
      this._controller = new WritableStreamDefaultController(this, sink, strategy);
      this._backpressure = false;
      this._readyPromise = Promise.resolve();
      this._readyResolve = undefined;
      const self = this;
      this._closedPromise = new Promise((res, rej) => { self._closedResolve = res; self._closedReject = rej; });
      const startRes = sink.start ? sink.start(this._controller) : undefined;
      this._writePromise = Promise.resolve(startRes);
    }

    _updateBackpressure() {
      const bp = this._controller.desiredSize <= 0;
      if (bp && !this._backpressure) {
        const self = this;
        this._readyPromise = new Promise((res) => { self._readyResolve = res; });
      } else if (!bp && this._backpressure) {
        if (this._readyResolve) this._readyResolve();
        this._readyResolve = undefined;
      }
      this._backpressure = bp;
    }
    get locked() { return this._writer !== undefined; }
    getWriter() { return new WritableStreamDefaultWriter(this); }
    abort(reason) {
      if (this.locked) return Promise.reject(new TypeError('Cannot abort a locked stream'));
      const w = this.getWriter();
      const p = w.abort(reason);
      w.releaseLock();
      return p;
    }
    close() {
      if (this.locked) return Promise.reject(new TypeError('Cannot close a locked stream'));
      const w = this.getWriter();
      const p = w.close();
      w.releaseLock();
      return p;
    }
  }

  class TransformStreamDefaultController {
    constructor(ts) { this._ts = ts; }
    enqueue(chunk) { this._ts._readableController.enqueue(chunk); }
    error(e) {
      this._ts._readableController.error(e);
      this._ts._writable._state = 'errored';
      this._ts._writable._storedError = e;
    }
    terminate() { this._ts._readableController.close(); }
    get desiredSize() { return this._ts._readableController.desiredSize; }
  }

  class TransformStream {
    constructor(transformer = {}, writableStrategy = {}, readableStrategy = {}) {
      const ts = this;
      this._transformer = transformer;
      this._controller = new TransformStreamDefaultController(this);
      this._readable = new ReadableStream({
        start(c) { ts._readableController = c; }
      });
      const transformFn = transformer.transform
        ? (chunk) => transformer.transform(chunk, ts._controller)
        : (chunk) => { ts._controller.enqueue(chunk); };
      this._writable = new WritableStream({
        write(chunk) { return Promise.resolve(transformFn(chunk)); },
        close() {
          const fr = transformer.flush ? transformer.flush(ts._controller) : undefined;
          return Promise.resolve(fr).then(() => { ts._readableController.close(); });
        },
        abort(reason) { ts._readableController.error(reason); }
      });
      const startRes = transformer.start ? transformer.start(this._controller) : undefined;
      void startRes;
    }
    get readable() { return this._readable; }
    get writable() { return this._writable; }
  }

  class ReadableByteStreamController {
    constructor(stream, source, strategy) {
      this._stream = stream;
      this._byteQueue = [];
      this._totalBytes = 0;
      this._closeRequested = false;
      this._pulling = false;
      this._pullAgain = false;
      this._started = false;
      this._autoAllocateChunkSize = source && source.autoAllocateChunkSize;
      this._byobRequest = null;
      this._hwm = extractHWM(strategy, 0);
      this._pull = source && source.pull ? (c) => source.pull(c) : () => {};
      this._cancel = source && source.cancel ? (r) => source.cancel(r) : () => {};
      const startRes = source && source.start ? source.start(this) : undefined;
      Promise.resolve(startRes).then(
        () => { this._started = true; this._callPullIfNeeded(); },
        (e) => this.error(e)
      );
    }
    get desiredSize() {
      const s = this._stream._state;
      if (s === 'errored') return null;
      if (s === 'closed') return 0;
      return this._hwm - this._totalBytes;
    }

    get byobRequest() {
      if (this._byobRequest) return this._byobRequest;
      const reader = this._stream._reader;
      if (this._totalBytes === 0 && reader && reader._readRequests &&
          reader._readRequests.length > 0 && reader._readRequests[0].view) {
        const readReq = reader._readRequests[0];
        const v = readReq.view;
        const reqView = new Uint8Array(v.buffer, v.byteOffset, v.byteLength);
        this._byobRequest = new ReadableStreamBYOBRequest(this, reqView, readReq);
      }
      return this._byobRequest || null;
    }
    _callPullIfNeeded() {
      if (!this._started) return;
      const stream = this._stream;
      if (stream._state !== 'readable') return;
      if (this._closeRequested) return;
      const wantsMore = (stream._reader && stream._reader._readRequests.length > 0) || this.desiredSize > 0;
      if (!wantsMore) return;
      if (this._pulling) { this._pullAgain = true; return; }
      this._pulling = true;
      Promise.resolve(this._pull(this)).then(
        () => { this._pulling = false; if (this._pullAgain) { this._pullAgain = false; this._callPullIfNeeded(); } },
        (e) => this.error(e)
      );
    }

    _normalize(chunk) {
      if (chunk instanceof Uint8Array) return chunk;
      if (ArrayBuffer.isView(chunk)) return new Uint8Array(chunk.buffer, chunk.byteOffset, chunk.byteLength);
      if (chunk instanceof ArrayBuffer) return new Uint8Array(chunk);
      throw new TypeError('byte stream chunk must be an ArrayBuffer view');
    }
    enqueue(chunk) {
      const stream = this._stream;
      if (stream._state !== 'readable') throw new TypeError('Cannot enqueue: stream is not readable');
      const u8 = this._normalize(chunk);
      if (u8.byteLength > 0) { this._byteQueue.push(u8); this._totalBytes += u8.byteLength; }
      this._serviceReadRequests();
      this._callPullIfNeeded();
    }

    _fillView(view) {
      const out = new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
      let filled = 0;
      while (filled < out.byteLength && this._byteQueue.length > 0) {
        const head = this._byteQueue[0];
        const need = out.byteLength - filled;
        if (head.byteLength <= need) {
          out.set(head, filled); filled += head.byteLength; this._totalBytes -= head.byteLength; this._byteQueue.shift();
        } else {
          out.set(head.subarray(0, need), filled); filled += need; this._totalBytes -= need; this._byteQueue[0] = head.subarray(need);
        }
      }
      const Ctor = view.constructor;
      const bpe = view.BYTES_PER_ELEMENT || 1;
      return new Ctor(out.buffer, out.byteOffset, filled / bpe);
    }
    _shiftChunk() { const c = this._byteQueue.shift(); this._totalBytes -= c.byteLength; return c; }
    _serviceReadRequests() {
      const reader = this._stream._reader;
      if (!reader || !reader._readRequests) return;
      while (reader._readRequests.length > 0 && this._totalBytes > 0) {
        const req = reader._readRequests.shift();
        if (req.view) req.resolve({ value: this._fillView(req.view), done: false });
        else req.resolve({ value: this._shiftChunk(), done: false });
      }
      if (this._totalBytes === 0 && this._closeRequested) this._closeStream();
    }
    close() {
      const stream = this._stream;
      if (stream._state !== 'readable') return;
      this._closeRequested = true;
      if (this._totalBytes === 0) this._closeStream();
    }
    _closeStream() {
      const stream = this._stream;
      stream._state = 'closed';
      const reader = stream._reader;
      if (reader) {
        for (const req of reader._readRequests) {
          if (req.view) req.resolve({ value: new req.view.constructor(req.view.buffer, req.view.byteOffset, 0), done: true });
          else req.resolve({ value: undefined, done: true });
        }
        reader._readRequests = [];
        reader._closedResolve && reader._closedResolve();
      }
    }
    error(e) {
      const stream = this._stream;
      if (stream._state !== 'readable') return;
      stream._state = 'errored';
      stream._storedError = e;
      this._byteQueue = [];
      this._totalBytes = 0;
      const reader = stream._reader;
      if (reader) {
        for (const req of reader._readRequests) req.reject(e);
        reader._readRequests = [];
        reader._closedReject && reader._closedReject(e);
      }
    }
  }

  class ReadableStreamBYOBRequest {
    constructor(controller, view, readReq) {
      this._controller = controller;
      this._view = view;
      this._readReq = readReq;
    }
    get view() { return this._view; }
    respond(bytesWritten) {
      const c = this._controller;
      if (c._byobRequest !== this) throw new TypeError('byobRequest is no longer valid');
      c._byobRequest = null;
      const reader = c._stream._reader;
      if (reader && reader._readRequests) {
        const idx = reader._readRequests.indexOf(this._readReq);
        if (idx >= 0) reader._readRequests.splice(idx, 1);
      }
      const v = this._readReq.view;
      const Ctor = v.constructor;
      const bpe = v.BYTES_PER_ELEMENT || 1;
      const n = Number(bytesWritten) | 0;
      const filled = new Ctor(v.buffer, v.byteOffset, n / bpe);
      this._readReq.resolve({ value: filled, done: false });
      c._callPullIfNeeded();
    }

    respondWithNewView(view) {
      if (!ArrayBuffer.isView(view)) throw new TypeError('respondWithNewView requires an ArrayBuffer view');
      this.respond(view.byteLength);
    }
  }

  class ReadableStreamBYOBReader {
    constructor(stream) {
      if (stream._reader) throw new TypeError('ReadableStream is already locked to a reader');
      this._stream = stream;
      this._readRequests = [];
      stream._reader = this;
      const self = this;
      this._closedPromise = new Promise((res, rej) => { self._closedResolve = res; self._closedReject = rej; });
      this._closedPromise.catch(() => {});
      if (stream._state === 'closed') this._closedResolve();
      else if (stream._state === 'errored') this._closedReject(stream._storedError);
    }
    get closed() { return this._closedPromise; }
    read(view) {
      const stream = this._stream;
      if (!stream) return Promise.reject(new TypeError('Reader has no associated stream'));
      if (!ArrayBuffer.isView(view)) return Promise.reject(new TypeError('read(view) requires an ArrayBuffer view'));
      if (stream._state === 'errored') return Promise.reject(stream._storedError);
      const ctrl = stream._controller;
      if (ctrl._totalBytes > 0) {
        const filled = ctrl._fillView(view);
        if (ctrl._totalBytes === 0 && ctrl._closeRequested) ctrl._closeStream();
        else ctrl._callPullIfNeeded();
        return Promise.resolve({ value: filled, done: false });
      }
      if (stream._state === 'closed') {
        return Promise.resolve({ value: new view.constructor(view.buffer, view.byteOffset, 0), done: true });
      }
      const self = this;
      const p = new Promise((resolve, reject) => { self._readRequests.push({ resolve, reject, view }); });
      ctrl._callPullIfNeeded();
      return p;
    }
    releaseLock() {
      const stream = this._stream;
      if (!stream) return;
      for (const req of this._readRequests) req.reject(new TypeError('Reader was released'));
      this._readRequests = [];
      stream._reader = undefined;
      this._stream = undefined;
    }
    cancel(reason) {
      if (!this._stream) return Promise.reject(new TypeError('Reader has no associated stream'));

      return this._stream._cancelInternal(reason);
    }
  }

  class CountQueuingStrategy {
    constructor(opts) { this._hwm = opts && opts.highWaterMark; }
    get highWaterMark() { return this._hwm; }
    get size() { return () => 1; }
  }
  class ByteLengthQueuingStrategy {
    constructor(opts) { this._hwm = opts && opts.highWaterMark; }
    get highWaterMark() { return this._hwm; }
    get size() { return (chunk) => chunk.byteLength; }
  }

  globalThis.CountQueuingStrategy = CountQueuingStrategy;
  globalThis.ByteLengthQueuingStrategy = ByteLengthQueuingStrategy;
  globalThis.ReadableStream = ReadableStream;
  globalThis.ReadableStreamDefaultReader = ReadableStreamDefaultReader;
  globalThis.ReadableStreamDefaultController = ReadableStreamDefaultController;
  globalThis.ReadableStreamBYOBReader = ReadableStreamBYOBReader;
  globalThis.ReadableByteStreamController = ReadableByteStreamController;
  globalThis.ReadableStreamBYOBRequest = ReadableStreamBYOBRequest;
  globalThis.WritableStream = WritableStream;
  globalThis.WritableStreamDefaultWriter = WritableStreamDefaultWriter;
  globalThis.WritableStreamDefaultController = WritableStreamDefaultController;
  globalThis.TransformStream = TransformStream;
  globalThis.TransformStreamDefaultController = TransformStreamDefaultController;
})();
