
(function () {
  if (typeof Response === 'undefined' || typeof ReadableStream === 'undefined') {
    return;
  }

  function bytesFromLatin1(s) {
    const u = new Uint8Array(s.length);
    for (let i = 0; i < s.length; i++) u[i] = s.charCodeAt(i) & 0xff;
    return u;
  }

  function instanceBytes(self) {
    const latin = self.__body_bytes;
    return (typeof latin === 'string') ? bytesFromLatin1(latin) : new Uint8Array(0);
  }

  function isReadable(x) {
    return x && typeof x === 'object' && typeof x.getReader === 'function';
  }

  async function drainStream(stream) {
    const reader = stream.getReader();
    const chunks = [];
    let total = 0;
    while (true) {
      const r = await reader.read();
      if (r.done) break;
      const c = r.value instanceof Uint8Array ? r.value : new Uint8Array(r.value);
      chunks.push(c);
      total += c.length;
    }
    const out = new Uint8Array(total);
    let o = 0;
    for (const c of chunks) { out.set(c, o); o += c.length; }
    return out;
  }

  const decoder = new TextDecoder();

  function patch(proto) {

    Object.defineProperty(proto, 'body', {
      configurable: true,
      get() {
        if (this.__body_used) return null;
        if (isReadable(this.__body_stream)) return this.__body_stream;
        return ReadableStream.from([instanceBytes(this)]);
      },
    });

    const origText = proto.text;
    const origArrayBuffer = proto.arrayBuffer;
    const origBlob = proto.blob;

    proto.text = async function () {
      if (isReadable(this.__body_stream) && !this.__body_used) {
        const b = await drainStream(this.__body_stream);
        this.__body_used = true;
        return decoder.decode(b);
      }
      return origText.call(this);
    };
    proto.json = async function () {
      const t = await this.text();
      return JSON.parse(t);
    };
    proto.arrayBuffer = async function () {
      if (isReadable(this.__body_stream) && !this.__body_used) {
        const b = await drainStream(this.__body_stream);
        this.__body_used = true;
        return b.buffer;
      }
      return origArrayBuffer.call(this);
    };
    proto.blob = async function () {
      if (isReadable(this.__body_stream) && !this.__body_used) {
        const b = await drainStream(this.__body_stream);
        this.__body_used = true;
        return new Blob([b]);
      }
      return origBlob.call(this);
    };
  }

  patch(Response.prototype);
  patch(Request.prototype);
})();
