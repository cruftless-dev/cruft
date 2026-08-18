
pub const PRELUDE: &str = include_str!("prelude.js");

pub fn prelude_source() -> &'static str {
    PRELUDE
}

pub fn prelude_install_parts() -> Vec<String> {
    const READABLE: &str = "  // ---------- ReadableStream ----------";
    const READABLE_READER: &str = "  class ReadableStreamDefaultReader";
    const READABLE_STREAM: &str = "  class ReadableStream {";
    const WRITABLE: &str = "  // ---------- WritableStream ----------";
    const BYTE: &str =
        "  // ---------- Readable byte streams + BYOB (WHATWG Streams §3.7) ----------";
    const GLOBALS: &str = "  globalThis.CountQueuingStrategy = CountQueuingStrategy;";

    let body_start = PRELUDE
        .find("  'use strict';")
        .expect("Streams prelude missing strict prologue");
    let readable_start = PRELUDE
        .find(READABLE)
        .expect("Streams prelude missing readable marker");
    let writable_start = PRELUDE
        .find(WRITABLE)
        .expect("Streams prelude missing writable marker");
    let readable_reader_start = PRELUDE
        .find(READABLE_READER)
        .expect("Streams prelude missing default-reader marker");
    let readable_stream_start = PRELUDE
        .find(READABLE_STREAM)
        .expect("Streams prelude missing readable-stream marker");
    let byte_start = PRELUDE
        .find(BYTE)
        .expect("Streams prelude missing byte-stream marker");
    let globals_start = PRELUDE
        .find(GLOBALS)
        .expect("Streams prelude missing global assignment marker");

    let helpers = &PRELUDE[body_start + "  'use strict';".len()..readable_start];
    let readable_controller = &PRELUDE[readable_start..readable_reader_start];
    let readable_reader = &PRELUDE[readable_reader_start..readable_stream_start];
    let readable_stream = &PRELUDE[readable_stream_start..writable_start];
    let writable_transform = &PRELUDE[writable_start..byte_start];
    let byte_and_strategy = &PRELUDE[byte_start..globals_start];

    vec![
        format!(
            "(function () {{\n  'use strict';\n{helpers}{readable_controller}\n  globalThis.__streams_extractHWM = extractHWM;\n  globalThis.__streams_extractSizeAlgo = extractSizeAlgo;\n  globalThis.ReadableStreamDefaultController = ReadableStreamDefaultController;\n}})();"
        ),
        format!(
            "(function () {{\n  'use strict';\n{readable_reader}\n  globalThis.ReadableStreamDefaultReader = ReadableStreamDefaultReader;\n}})();"
        ),
        format!(
            "(function () {{\n  'use strict';\n{readable_stream}\n  globalThis.ReadableStream = ReadableStream;\n}})();"
        ),
        format!(
            "(function () {{\n  'use strict';\n  const extractHWM = globalThis.__streams_extractHWM;\n  const extractSizeAlgo = globalThis.__streams_extractSizeAlgo;\n{writable_transform}\n  globalThis.WritableStream = WritableStream;\n  globalThis.WritableStreamDefaultWriter = WritableStreamDefaultWriter;\n  globalThis.WritableStreamDefaultController = WritableStreamDefaultController;\n  globalThis.TransformStream = TransformStream;\n  globalThis.TransformStreamDefaultController = TransformStreamDefaultController;\n}})();"
        ),
        format!(
            "(function () {{\n  'use strict';\n  const extractHWM = globalThis.__streams_extractHWM;\n{byte_and_strategy}\n  globalThis.CountQueuingStrategy = CountQueuingStrategy;\n  globalThis.ByteLengthQueuingStrategy = ByteLengthQueuingStrategy;\n  globalThis.ReadableStreamBYOBReader = ReadableStreamBYOBReader;\n  globalThis.ReadableByteStreamController = ReadableByteStreamController;\n  globalThis.ReadableStreamBYOBRequest = ReadableStreamBYOBRequest;\n}})();"
        ),

        String::from(TEXT_STREAMS_PART),
    ]
}

pub const TEXT_STREAMS_PART: &str = r#"(function () {
  'use strict';
  class TextDecoderStream {
    constructor(label, options) {
      const decoder = new TextDecoder(label, options);
      const ts = new TransformStream({
        transform(chunk, controller) {
          const s = decoder.decode(chunk, { stream: true });
          if (s) controller.enqueue(s);
        },
        flush(controller) {
          const s = decoder.decode();
          if (s) controller.enqueue(s);
        },
      });
      Object.defineProperty(this, 'readable', { value: ts.readable, enumerable: true });
      Object.defineProperty(this, 'writable', { value: ts.writable, enumerable: true });
      Object.defineProperty(this, 'encoding', { value: decoder.encoding, enumerable: true });
      Object.defineProperty(this, 'fatal', { value: decoder.fatal, enumerable: true });
      Object.defineProperty(this, 'ignoreBOM', { value: decoder.ignoreBOM, enumerable: true });
    }
  }
  class TextEncoderStream {
    constructor() {
      const encoder = new TextEncoder();
      const ts = new TransformStream({
        transform(chunk, controller) {
          const bytes = encoder.encode(String(chunk));
          if (bytes.length) controller.enqueue(bytes);
        },
      });
      Object.defineProperty(this, 'readable', { value: ts.readable, enumerable: true });
      Object.defineProperty(this, 'writable', { value: ts.writable, enumerable: true });
      Object.defineProperty(this, 'encoding', { value: 'utf-8', enumerable: true });
    }
  }
  globalThis.TextDecoderStream = TextDecoderStream;
  globalThis.TextEncoderStream = TextEncoderStream;
})();"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prelude_is_present_and_nonempty() {
        assert!(!PRELUDE.is_empty());
        assert_eq!(prelude_source(), PRELUDE);
    }

    #[test]
    fn prelude_defines_the_core_streams_classes() {

        for sym in [
            "class ReadableStream",
            "class WritableStream",
            "class TransformStream",
            "ReadableStreamDefaultController",
            "ReadableStreamDefaultReader",
        ] {
            assert!(PRELUDE.contains(sym), "streams prelude missing: {sym}");
        }
    }

    #[test]
    fn prelude_installs_onto_globalthis() {
        assert!(
            PRELUDE.contains("globalThis"),
            "prelude must assign onto globalThis"
        );
    }

    #[test]
    fn prelude_install_parts_cover_core_surface() {
        let parts = prelude_install_parts();
        assert_eq!(parts.len(), 6);
        for sym in [
            "globalThis.ReadableStream = ReadableStream",
            "globalThis.WritableStream = WritableStream",
            "globalThis.TransformStream = TransformStream",
            "globalThis.ReadableStreamBYOBReader = ReadableStreamBYOBReader",
            "globalThis.CountQueuingStrategy = CountQueuingStrategy",
            "globalThis.TextDecoderStream = TextDecoderStream",
            "globalThis.TextEncoderStream = TextEncoderStream",
        ] {
            assert!(
                parts.iter().any(|part| part.contains(sym)),
                "split Streams prelude missing install: {sym}"
            );
        }
    }
}
