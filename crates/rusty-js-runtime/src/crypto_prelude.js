
(function () {
  'use strict';

  const KEYMAT = new WeakMap();

  function normalizeAlg(a) {
    if (typeof a === 'string') return { name: a };
    if (a && typeof a === 'object' && typeof a.name === 'string') return a;
    throw new TypeError('Algorithm: expected a string or an object with a name');
  }
  function hashOf(alg) {
    var h = alg && alg.hash;
    if (typeof h === 'string') return h;
    if (h && typeof h.name === 'string') return h.name;
    throw new TypeError('Algorithm: a hash is required');
  }

  function hashAlg(alg) { return { name: hashOf(alg) }; }

  function jwkOctAlg(algorithm, bitLen) {
    var name = algorithm && algorithm.name;
    if (name === 'HMAC') {
      var h = (algorithm.hash && algorithm.hash.name) || algorithm.hash;
      if (h === 'SHA-1') return 'HS1';
      if (h === 'SHA-256') return 'HS256';
      if (h === 'SHA-384') return 'HS384';
      if (h === 'SHA-512') return 'HS512';
      return undefined;
    }
    var suffix;
    if (name === 'AES-GCM') suffix = 'GCM';
    else if (name === 'AES-CBC') suffix = 'CBC';
    else if (name === 'AES-CTR') suffix = 'CTR';
    else if (name === 'AES-KW') suffix = 'KW';
    if (suffix && bitLen) return 'A' + bitLen + suffix;
    return undefined;
  }
  function hashName(h) {
    if (h == null) return h;
    if (typeof h === 'string') return h;
    if (typeof h.name === 'string') return h.name;
    return h;
  }
  function isRsaAlg(n) {
    return n === 'RSASSA-PKCS1-v1_5' || n === 'RSA-PSS' || n === 'RSA-OAEP';
  }
  function ecCurveBytes(curve) {
    if (curve === 'P-256') return 32;
    if (curve === 'P-384') return 48;
    if (curve === 'P-521') return 66;
    throw new TypeError('EC: unsupported namedCurve ' + curve);
  }
  function normalizeEcCurve(curve) {
    curve = curve || 'P-256';
    ecCurveBytes(curve);
    return curve;
  }

  function b64urlToBytes(s) {
    var b64 = String(s).replace(/-/g, '+').replace(/_/g, '/');
    while (b64.length % 4) b64 += '=';
    var bin = atob(b64);
    var out = [];
    for (var i = 0; i < bin.length; i++) out.push(bin.charCodeAt(i) & 0xff);
    return out;
  }

  function bytesToB64url(ab) {
    var view = new Uint8Array(ab);
    var bin = '';
    for (var i = 0; i < view.length; i++) bin += String.fromCharCode(view[i]);
    return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  }
  function ecJwkPoint(jwk, curve) {
    if (!jwk || jwk.kty !== 'EC' || jwk.crv !== curve || typeof jwk.x !== 'string' || typeof jwk.y !== 'string') {
      throw new TypeError("importKey: EC jwk requires kty 'EC', crv '" + curve + "', x and y");
    }
    var cb = ecCurveBytes(curve);
    var x = b64urlToBytes(jwk.x);
    var y = b64urlToBytes(jwk.y);
    if (x.length !== cb || y.length !== cb) throw new TypeError('importKey: EC ' + curve + ' jwk coordinates must be ' + cb + ' bytes');
    var point = [0x04];
    for (var i = 0; i < x.length; i++) point.push(x[i]);
    for (var j = 0; j < y.length; j++) point.push(y[j]);
    return point;
  }

  var EC_P256_SPKI_PREFIX = [0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce,
    0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00];
  function asBytes(src) {
    if (src instanceof ArrayBuffer) return new Uint8Array(src);
    if (ArrayBuffer.isView(src)) return new Uint8Array(src.buffer, src.byteOffset, src.byteLength);
    return new Uint8Array(src);
  }

  function CryptoKey(type, extractable, algorithm, usages) {
    this.type = type;
    this.extractable = !!extractable;
    this.algorithm = algorithm;
    this.usages = usages ? usages.slice() : [];
  }

  var subtle = {
    digest: function (algorithm, data) {
      try {
        var alg = normalizeAlg(algorithm);
        return Promise.resolve(__crypto_digest(alg.name, data));
      } catch (e) { return Promise.reject(e); }
    },

    importKey: function (format, keyData, algorithm, extractable, keyUsages) {
      try {
        var alg = normalizeAlg(algorithm);
        var material;
        var ecJwkCurve;
        if (format === 'raw') {
          material = keyData;
        } else if (format === 'jwk') {
          if (keyData && keyData.kty === 'EC') {
            if (alg.name !== 'ECDSA' && alg.name !== 'ECDH') throw new TypeError('importKey: jwk is an EC key but algorithm is ' + alg.name);
            if (keyData.d != null) throw new TypeError('importKey: EC private jwk is not wired yet');
            ecJwkCurve = normalizeEcCurve(alg.namedCurve || keyData.crv);
            material = ecJwkPoint(keyData, ecJwkCurve);
          } else {
            if (!keyData || keyData.kty !== 'oct' || typeof keyData.k !== 'string') {
              throw new TypeError("importKey: jwk requires an octet key (kty 'oct', 'k')");
            }
            material = b64urlToBytes(keyData.k);
          }
        } else if (format === 'spki') {
          if (alg.name === 'ECDSA' || alg.name === 'ECDH') {
            var spkiCurve = normalizeEcCurve(alg.namedCurve);
            if (spkiCurve !== 'P-256') throw new TypeError('importKey: EC spki is wired for P-256 only (' + spkiCurve + ')');
            var sb = asBytes(keyData);
            if (sb.length !== 91 || !EC_P256_SPKI_PREFIX.every(function (b, i) { return sb[i] === b; })) {
              throw new TypeError('importKey: unsupported spki (only EC P-256 SubjectPublicKeyInfo)');
            }
            material = sb.slice(26);
          } else if (isRsaAlg(alg.name)) {
            var sinfo = __crypto_spki_import(keyData);
            if (sinfo.kind !== 'rsa') throw new TypeError('importKey: spki algorithm mismatch (expected RSA)');
            material = { n: sinfo.n, e: sinfo.e };
          } else {
            throw new TypeError("importKey: spki is wired for EC P-256 and RSA (" + alg.name + ")");
          }
        } else if (format === 'pkcs8') {
          var pinfo = __crypto_pkcs8_import(keyData);
          if (pinfo.kind === 'rsa') {
            if (!isRsaAlg(alg.name)) throw new TypeError('importKey: pkcs8 is an RSA key but algorithm is ' + alg.name);
            material = { n: pinfo.n, e: pinfo.e, d: pinfo.d, p: pinfo.p, q: pinfo.q, dp: pinfo.dp, dq: pinfo.dq, qi: pinfo.qi };
          } else {
            if (alg.name !== 'ECDSA' && alg.name !== 'ECDH') throw new TypeError('importKey: pkcs8 is an EC key but algorithm is ' + alg.name);
            var pkcs8Curve = normalizeEcCurve(alg.namedCurve);
            if (pkcs8Curve !== 'P-256') throw new TypeError('importKey: EC pkcs8 is wired for P-256 only (' + pkcs8Curve + ')');
            material = pinfo.scalar;
          }
        } else {
          throw new TypeError("importKey: 'raw', 'jwk', 'spki', 'pkcs8' are the wired formats");
        }
        var keyAlg = { name: alg.name };
        if (alg.name === 'HMAC') {
          keyAlg.hash = hashAlg(alg);

          var hmacBytes = (material && material.byteLength != null) ? material.byteLength
            : (material && material.length != null ? material.length : null);
          if (hmacBytes != null) keyAlg.length = hmacBytes * 8;
        }
        var type = 'secret';
        if (format === 'jwk' && (alg.name === 'ECDSA' || alg.name === 'ECDH')) {
          type = 'public';
          keyAlg.namedCurve = ecJwkCurve;
        } else if (format === 'spki') {
          type = 'public';
          if (alg.name === 'ECDSA' || alg.name === 'ECDH') keyAlg.namedCurve = spkiCurve || 'P-256';
          else keyAlg.hash = hashAlg(alg);
        } else if (format === 'pkcs8') {
          type = 'private';
          if (alg.name === 'ECDSA' || alg.name === 'ECDH') keyAlg.namedCurve = pkcs8Curve || 'P-256';
          else keyAlg.hash = hashAlg(alg);
        }
        var key = new CryptoKey(type, extractable, keyAlg, keyUsages);
        KEYMAT.set(key, material);
        return Promise.resolve(key);
      } catch (e) { return Promise.reject(e); }
    },

    exportKey: function (format, key) {
      try {
        if (!(key instanceof CryptoKey)) throw new TypeError('exportKey: not a CryptoKey');
        if (!key.extractable) throw new DOMException('key is not extractable', 'InvalidAccessError');
        var material = KEYMAT.get(key);
        if (format === 'raw') {
          return Promise.resolve(__crypto_buf(material));
        } else if (format === 'jwk') {
          if (key.algorithm.name === 'ECDSA' || key.algorithm.name === 'ECDH') {
            if (key.type !== 'public') throw new TypeError('exportKey: EC private jwk is not wired yet');
            var curve = normalizeEcCurve(key.algorithm.namedCurve);
            var cb = ecCurveBytes(curve);
            var point = asBytes(__crypto_buf(material));
            if (point.length !== 1 + (2 * cb) || point[0] !== 0x04) throw new TypeError('exportKey: EC public key material is not an uncompressed ' + curve + ' point');
            return Promise.resolve({
              kty: 'EC',
              crv: curve,
              x: bytesToB64url(point.slice(1, 1 + cb)),
              y: bytesToB64url(point.slice(1 + cb, 1 + (2 * cb))),
              ext: true
            });
          }
          var ab = __crypto_buf(material);

          var octJwk = {};
          if (key.usages) octJwk.key_ops = key.usages.slice();
          octJwk.ext = key.extractable;
          var octAlg = jwkOctAlg(key.algorithm, asBytes(ab).length * 8);
          if (octAlg) octJwk.alg = octAlg;
          octJwk.kty = 'oct';
          octJwk.k = bytesToB64url(ab);
          return Promise.resolve(octJwk);
        } else if (format === 'spki') {
          if (key.algorithm.name === 'ECDSA' || key.algorithm.name === 'ECDH') {
            var exportSpkiCurve = normalizeEcCurve(key.algorithm.namedCurve);
            if (exportSpkiCurve !== 'P-256') throw new TypeError('exportKey: EC spki is wired for P-256 only (' + exportSpkiCurve + ')');
            var pt = asBytes(__crypto_buf(material));
            var outSpki = new Uint8Array(EC_P256_SPKI_PREFIX.length + pt.length);
            outSpki.set(EC_P256_SPKI_PREFIX, 0);
            outSpki.set(pt, EC_P256_SPKI_PREFIX.length);
            return Promise.resolve(outSpki.buffer);
          }
          if (isRsaAlg(key.algorithm.name)) {
            if (!material || !material.n) throw new TypeError('exportKey: spki requires an RSA public key');
            return Promise.resolve(__crypto_spki_export_rsa(material.n, material.e));
          }
          throw new TypeError('exportKey: spki is wired for EC P-256 and RSA');
        } else if (format === 'pkcs8') {
          if (key.type !== 'private') throw new TypeError('exportKey: pkcs8 requires a private key');
          if (key.algorithm.name === 'ECDSA' || key.algorithm.name === 'ECDH') {
            var exportPkcs8Curve = normalizeEcCurve(key.algorithm.namedCurve);
            if (exportPkcs8Curve !== 'P-256') throw new TypeError('exportKey: EC pkcs8 is wired for P-256 only (' + exportPkcs8Curve + ')');
            return Promise.resolve(__crypto_pkcs8_export_ec(material));
          }
          if (isRsaAlg(key.algorithm.name)) {
            if (!material || !material.p) throw new TypeError('exportKey: pkcs8 needs RSA CRT params (key was imported without them)');
            return Promise.resolve(__crypto_pkcs8_export_rsa(material.n, material.e, material.d, material.p, material.q, material.dp, material.dq, material.qi));
          }
          throw new TypeError('exportKey: pkcs8 is wired for EC P-256 and RSA');
        }
        throw new TypeError("exportKey: 'raw', 'jwk', 'spki', 'pkcs8' are the wired formats");
      } catch (e) { return Promise.reject(e); }
    },

    sign: function (algorithm, key, data) {
      try {
        var alg = normalizeAlg(algorithm);
        if (!(key instanceof CryptoKey)) throw new TypeError('sign: not a CryptoKey');
        var material = KEYMAT.get(key);
        if (alg.name === 'HMAC') {
          var hash = hashName(key.algorithm.hash) || hashOf(alg);
          return Promise.resolve(__crypto_hmac(hash, material, data));
        }
        if (alg.name === 'ECDSA') {
          return Promise.resolve(__crypto_ecdsa_sign(hashOf(alg), material, data, key.algorithm.namedCurve));
        }
        if (alg.name === 'RSASSA-PKCS1-v1_5' || alg.name === 'RSA-PSS') {
          var rs = alg.name === 'RSA-PSS' ? 'pss' : 'pkcs1';
          var rh = hashName(key.algorithm && key.algorithm.hash) || hashOf(alg);
          return Promise.resolve(__crypto_rsa_sign(rh, rs, material.n, material.d, data));
        }
        if (alg.name === 'Ed25519') {
          return Promise.resolve(__crypto_ed25519_sign(material, data));
        }
        throw new TypeError('sign: only HMAC, ECDSA, RSA and Ed25519 are wired (' + alg.name + ')');
      } catch (e) { return Promise.reject(e); }
    },

    verify: function (algorithm, key, signature, data) {
      try {
        var alg = normalizeAlg(algorithm);
        if (!(key instanceof CryptoKey)) throw new TypeError('verify: not a CryptoKey');
        var material = KEYMAT.get(key);
        if (alg.name === 'HMAC') {
          var hash = hashName(key.algorithm.hash) || hashOf(alg);
          return Promise.resolve(__crypto_hmac_verify(hash, material, data, signature));
        }
        if (alg.name === 'ECDSA') {
          return Promise.resolve(__crypto_ecdsa_verify(hashOf(alg), material, signature, data, key.algorithm.namedCurve));
        }
        if (alg.name === 'RSASSA-PKCS1-v1_5' || alg.name === 'RSA-PSS') {
          var rs2 = alg.name === 'RSA-PSS' ? 'pss' : 'pkcs1';
          var rh2 = hashName(key.algorithm && key.algorithm.hash) || hashOf(alg);
          return Promise.resolve(__crypto_rsa_verify(rh2, rs2, material.n, material.e, data, signature));
        }
        if (alg.name === 'Ed25519') {
          return Promise.resolve(__crypto_ed25519_verify(material, data, signature));
        }
        throw new TypeError('verify: only HMAC, ECDSA, RSA and Ed25519 are wired (' + alg.name + ')');
      } catch (e) { return Promise.reject(e); }
    },

    encrypt: function (algorithm, key, data) {
      try {
        var alg = normalizeAlg(algorithm);
        if (!(key instanceof CryptoKey)) throw new TypeError('encrypt: not a CryptoKey');
        var material = KEYMAT.get(key);
        if (alg.name === 'AES-GCM') {
          if (!alg.iv) throw new TypeError('AES-GCM: iv is required');
          var aad = alg.additionalData || [];
          return Promise.resolve(__crypto_aes_gcm_encrypt(material, alg.iv, aad, data));
        }
        if (alg.name === 'AES-CBC') {
          if (!alg.iv) throw new TypeError('AES-CBC: iv is required');
          return Promise.resolve(__crypto_aes_cbc_encrypt(material, alg.iv, data));
        }
        if (alg.name === 'AES-CTR') {
          if (!alg.counter) throw new TypeError('AES-CTR: counter is required');
          return Promise.resolve(__crypto_aes_ctr(material, alg.counter, alg.length || 64, data));
        }
        if (alg.name === 'RSA-OAEP') {
          return Promise.resolve(__crypto_rsa_oaep_encrypt(hashName(key.algorithm.hash) || 'SHA-256', material.n, material.e, alg.label || [], data));
        }
        throw new TypeError('encrypt: only AES-GCM/CBC/CTR and RSA-OAEP are wired (' + alg.name + ')');
      } catch (e) { return Promise.reject(e); }
    },

    decrypt: function (algorithm, key, data) {
      try {
        var alg = normalizeAlg(algorithm);
        if (!(key instanceof CryptoKey)) throw new TypeError('decrypt: not a CryptoKey');
        var material = KEYMAT.get(key);
        if (alg.name === 'AES-GCM') {
          if (!alg.iv) throw new TypeError('AES-GCM: iv is required');
          var aad = alg.additionalData || [];
          return Promise.resolve(__crypto_aes_gcm_decrypt(material, alg.iv, aad, data));
        }
        if (alg.name === 'AES-CBC') {
          if (!alg.iv) throw new TypeError('AES-CBC: iv is required');
          return Promise.resolve(__crypto_aes_cbc_decrypt(material, alg.iv, data));
        }
        if (alg.name === 'AES-CTR') {
          if (!alg.counter) throw new TypeError('AES-CTR: counter is required');
          return Promise.resolve(__crypto_aes_ctr(material, alg.counter, alg.length || 64, data));
        }
        if (alg.name === 'RSA-OAEP') {
          return Promise.resolve(__crypto_rsa_oaep_decrypt(hashName(key.algorithm.hash) || 'SHA-256', material.n, material.d, alg.label || [], data));
        }
        throw new TypeError('decrypt: only AES-GCM/CBC/CTR and RSA-OAEP are wired (' + alg.name + ')');
      } catch (e) { return Promise.reject(e); }
    },

    generateKey: function (algorithm, extractable, keyUsages) {
      try {
        var alg = normalizeAlg(algorithm);
        if (alg.name === 'HMAC') {
          var hash = hashOf(alg);
          var bits = alg.length || ({ 'SHA-1': 512, 'SHA-256': 512, 'SHA-384': 1024, 'SHA-512': 1024 }[hash] || 512);
          var mat = __crypto_random((bits + 7) >> 3);
          var k = new CryptoKey('secret', extractable, { name: 'HMAC', hash: { name: hash }, length: bits }, keyUsages);
          KEYMAT.set(k, mat);
          return Promise.resolve(k);
        }
        if (alg.name === 'AES-GCM' || alg.name === 'AES-CBC' || alg.name === 'AES-CTR' || alg.name === 'AES-KW') {
          var len = alg.length || 256;
          if (len !== 128 && len !== 192 && len !== 256) throw new TypeError(alg.name + ': length must be 128/192/256');
          var matAes = __crypto_random(len >> 3);
          var kAes = new CryptoKey('secret', extractable, { name: alg.name, length: len }, keyUsages);
          KEYMAT.set(kAes, matAes);
          return Promise.resolve(kAes);
        }
        if (alg.name === 'ECDSA' || alg.name === 'ECDH') {
          var curve = normalizeEcCurve(alg.namedCurve);
          var kp = __crypto_ec_generate(curve);
          var u = keyUsages || [];
          var privU = alg.name === 'ECDSA' ? ['sign'] : ['deriveBits', 'deriveKey'];
          var priv = new CryptoKey('private', extractable, { name: alg.name, namedCurve: curve },
            u.filter(function (x) { return privU.indexOf(x) >= 0; }));
          var pub = new CryptoKey('public', true, { name: alg.name, namedCurve: curve },
            u.filter(function (x) { return x === 'verify'; }));
          KEYMAT.set(priv, kp[0]);
          KEYMAT.set(pub, kp[1]);
          return Promise.resolve({ privateKey: priv, publicKey: pub });
        }
        if (alg.name === 'RSASSA-PKCS1-v1_5' || alg.name === 'RSA-PSS' || alg.name === 'RSA-OAEP') {
          var bits = alg.modulusLength || 2048;
          var pe = alg.publicExponent || new Uint8Array([1, 0, 1]);
          var rkp = __crypto_rsa_generate(bits, pe);
          var ru = keyUsages || [];
          var rAlg = { name: alg.name, modulusLength: bits, publicExponent: pe, hash: hashAlg(alg) };
          var rPrivU = alg.name === 'RSA-OAEP' ? ['decrypt', 'unwrapKey'] : ['sign'];
          var rPubU = alg.name === 'RSA-OAEP' ? ['encrypt', 'wrapKey'] : ['verify'];
          var rPriv = new CryptoKey('private', extractable, rAlg, ru.filter(function (x) { return rPrivU.indexOf(x) >= 0; }));
          var rPub = new CryptoKey('public', true, rAlg, ru.filter(function (x) { return rPubU.indexOf(x) >= 0; }));
          KEYMAT.set(rPriv, { n: rkp.n, e: rkp.e, d: rkp.d, p: rkp.p, q: rkp.q, dp: rkp.dp, dq: rkp.dq, qi: rkp.qi });
          KEYMAT.set(rPub, { n: rkp.n, e: rkp.e });
          return Promise.resolve({ privateKey: rPriv, publicKey: rPub });
        }
        if (alg.name === 'X25519') {
          var xsk = __crypto_random(32);
          var xpk = __crypto_x25519_base(xsk);
          var xu = keyUsages || [];
          var xPriv = new CryptoKey('private', extractable, { name: 'X25519' },
            xu.filter(function (x) { return x === 'deriveBits' || x === 'deriveKey'; }));
          var xPub = new CryptoKey('public', true, { name: 'X25519' }, []);
          KEYMAT.set(xPriv, xsk);
          KEYMAT.set(xPub, xpk);
          return Promise.resolve({ privateKey: xPriv, publicKey: xPub });
        }
        if (alg.name === 'Ed25519') {
          var edseed = __crypto_random(32);
          var edpk = __crypto_ed25519_pubkey(edseed);
          var edu = keyUsages || [];
          var edPriv = new CryptoKey('private', extractable, { name: 'Ed25519' },
            edu.filter(function (x) { return x === 'sign'; }));
          var edPub = new CryptoKey('public', true, { name: 'Ed25519' },
            edu.filter(function (x) { return x === 'verify'; }));
          KEYMAT.set(edPriv, edseed);
          KEYMAT.set(edPub, edpk);
          return Promise.resolve({ privateKey: edPriv, publicKey: edPub });
        }
        throw new TypeError('generateKey: only HMAC, AES, ECDSA, ECDH, X25519, Ed25519 and RSA are wired (' + alg.name + ')');
      } catch (e) { return Promise.reject(e); }
    },

    deriveBits: function (algorithm, baseKey, length) {
      try {
        var alg = normalizeAlg(algorithm);
        if (!(baseKey instanceof CryptoKey)) throw new TypeError('deriveBits: baseKey is not a CryptoKey');
        var material = KEYMAT.get(baseKey);
        var byteLen = (Number(length) + 7) >> 3;
        if (alg.name === 'HKDF') {
          return Promise.resolve(__crypto_hkdf(hashOf(alg), material, alg.salt || [], alg.info || [], byteLen));
        }
        if (alg.name === 'PBKDF2') {
          var iters = alg.iterations >>> 0;
          if (!iters) throw new TypeError('PBKDF2: iterations is required');
          return Promise.resolve(__crypto_pbkdf2(hashOf(alg), material, alg.salt || [], iters, byteLen));
        }
        if (alg.name === 'ECDH') {
          if (!(alg.public instanceof CryptoKey)) throw new TypeError('ECDH: a public key is required');
          var curve = normalizeEcCurve(baseKey.algorithm.namedCurve);
          if (alg.public.algorithm.namedCurve !== curve) throw new TypeError('ECDH: public key curve mismatch');
          var secret = __crypto_ecdh(material, KEYMAT.get(alg.public), curve);
          var cb = ecCurveBytes(curve);
          return Promise.resolve(byteLen < cb ? secret.slice(0, byteLen) : secret);
        }
        if (alg.name === 'X25519') {
          if (!(alg.public instanceof CryptoKey)) throw new TypeError('X25519: a public key is required');
          var xsec = __crypto_x25519(material, KEYMAT.get(alg.public));
          return Promise.resolve(byteLen < 32 ? xsec.slice(0, byteLen) : xsec);
        }
        throw new TypeError('deriveBits: only HKDF, PBKDF2, ECDH and X25519 are wired (' + alg.name + ')');
      } catch (e) { return Promise.reject(e); }
    },

    deriveKey: function (algorithm, baseKey, derivedKeyType, extractable, keyUsages) {
      try {
        var dkt = normalizeAlg(derivedKeyType);
        var bits = dkt.length;
        if (!bits) {
          bits = dkt.name === 'HMAC'
            ? ({ 'SHA-1': 512, 'SHA-256': 512, 'SHA-384': 1024, 'SHA-512': 1024 }[hashOf(dkt)] || 512)
            : 256;
        }
        return subtle.deriveBits(algorithm, baseKey, bits).then(function (ab) {
          return subtle.importKey('raw', ab, derivedKeyType, extractable, keyUsages);
        });
      } catch (e) { return Promise.reject(e); }
    },

    wrapKey: function (format, key, wrappingKey, wrapAlgorithm) {
      try {
        var alg = normalizeAlg(wrapAlgorithm);
        if (!(wrappingKey instanceof CryptoKey)) throw new TypeError('wrapKey: wrappingKey is not a CryptoKey');
        var kek = KEYMAT.get(wrappingKey);
        return subtle.exportKey(format, key).then(function (exported) {
          var raw = (exported instanceof ArrayBuffer) ? exported : __crypto_buf(exported);
          if (alg.name === 'AES-KW') {
            return __crypto_aes_kw_wrap(kek, raw);
          }
          if (alg.name === 'AES-GCM') {
            if (!alg.iv) throw new TypeError('AES-GCM: iv is required');
            return __crypto_aes_gcm_encrypt(kek, alg.iv, alg.additionalData || [], raw);
          }
          if (alg.name === 'RSA-OAEP') {
            return __crypto_rsa_oaep_encrypt(hashName(wrappingKey.algorithm.hash) || 'SHA-256', kek.n, kek.e, alg.label || [], raw);
          }
          throw new TypeError('wrapKey: only AES-KW, AES-GCM and RSA-OAEP are wired (' + alg.name + ')');
        });
      } catch (e) { return Promise.reject(e); }
    },

    unwrapKey: function (format, wrappedKey, unwrappingKey, unwrapAlgorithm, unwrappedKeyAlgorithm, extractable, keyUsages) {
      try {
        var alg = normalizeAlg(unwrapAlgorithm);
        if (!(unwrappingKey instanceof CryptoKey)) throw new TypeError('unwrapKey: unwrappingKey is not a CryptoKey');
        var kek = KEYMAT.get(unwrappingKey);
        var raw;
        if (alg.name === 'AES-KW') {
          raw = __crypto_aes_kw_unwrap(kek, wrappedKey);
        } else if (alg.name === 'AES-GCM') {
          if (!alg.iv) throw new TypeError('AES-GCM: iv is required');
          raw = __crypto_aes_gcm_decrypt(kek, alg.iv, alg.additionalData || [], wrappedKey);
        } else if (alg.name === 'RSA-OAEP') {
          raw = __crypto_rsa_oaep_decrypt(hashName(unwrappingKey.algorithm.hash) || 'SHA-256', kek.n, kek.d, alg.label || [], wrappedKey);
        } else {
          throw new TypeError('unwrapKey: only AES-KW, AES-GCM and RSA-OAEP are wired (' + alg.name + ')');
        }
        return subtle.importKey(format, raw, unwrappedKeyAlgorithm, extractable, keyUsages);
      } catch (e) { return Promise.reject(e); }
    }
  };

  function grv(view) { return __crypto_get_random_values(view); }
  function ruuid() { return __crypto_random_uuid(); }

  var existing = globalThis.crypto || null;
  if (existing && typeof existing === 'object') {
    if (!existing.subtle) existing.subtle = subtle;
    if (typeof existing.getRandomValues !== 'function') existing.getRandomValues = grv;
    if (typeof existing.randomUUID !== 'function') existing.randomUUID = ruuid;

    if (existing.webcrypto && typeof existing.webcrypto === 'object' && !existing.webcrypto.subtle) {
      existing.webcrypto.subtle = subtle;
    }
  } else {
    globalThis.crypto = { getRandomValues: grv, randomUUID: ruuid, subtle: subtle };
  }

  Object.defineProperty(CryptoKey.prototype, Symbol.toStringTag, {
    value: 'CryptoKey', configurable: true
  });
  globalThis.CryptoKey = CryptoKey;
  function SubtleCrypto() {}
  Object.defineProperty(SubtleCrypto.prototype, Symbol.toStringTag, {
    value: 'SubtleCrypto', configurable: true
  });
  Object.setPrototypeOf(subtle, SubtleCrypto.prototype);
  globalThis.SubtleCrypto = SubtleCrypto;
})();
