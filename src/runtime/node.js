// The parts of Node: the file system, the paths, the environment and the
// packages of npm.
function veyNode(ctx) {
  var isNode = ctx.isNode;
  var req = ctx.req;
  var toText = ctx.toText;
  var toBytes = ctx.toBytes;
  var utf8Decode = ctx.utf8Decode;
  var veyError = ctx.veyError;

  function env(name, fallback) {
    if (!isNode) {
      // The other helpers of Node say so, and so does this one.
      throw veyError("env() is only available in Node.");
    }
    // The module of Node is asked for the environment, not the ambient
    // `process`, which a bundler or a test runner may have replaced.
    var value = nodeModule("node:process").env[name];
    if (value === undefined) {
      return fallback === undefined ? null : fallback;
    }
    return value;
  }

  function nodeModule(name) {
    if (!isNode) throw veyError(name + " is only available in Node.");
    return req(name);
  }

  /// A helper of Node says its own name when Node is not there.
  function needNode(helper) {
    if (!isNode) throw veyError(helper + " is only available in Node.");
  }

  // A package of npm, such as `express` or `marked`.
  //
  // The package has to be installed in the project, with `npm install`. It
  // is loaded when the program starts, so that a program only pays for the
  // packages it uses.
  function require(name) {
    if (!isNode) throw veyError("require() is only available in Node.");
    try {
      return req(name);
    } catch (err) {
      throw veyError(
        "the package '" + name + "' is not installed (npm install " + name + ")"
      );
    }
  }

  // The fingerprint of a value, as text of hexadecimal numbers.
  function hash(value) {
    if (!isNode) throw veyError("hash() is only available in Node.");
    return nodeModule("node:crypto")
      .createHash("sha256")
      .update(toText(value))
      .digest("hex");
  }

  // Another program is run.
  //
  // The program is started at once, and the ones who watch it are told
  // when it writes or when it ends.
  function spawn(command, args) {
    if (!isNode) throw veyError("process.spawn() is only available in Node.");
    var child = nodeModule("node:child_process").spawn(String(command), args || []);
    var listeners = {};
    var out = [];
    var err = [];
    var code = null;

    function call(event, value) {
      var list = listeners[event];
      if (!list) return;
      for (var i = 0; i < list.length; i++) list[i](value);
    }

    child.stdout.on("data", function (chunk) {
      var text = chunk.toString("utf8");
      out.push(text);
      call("data", text);
    });
    child.stderr.on("data", function (chunk) {
      var text = chunk.toString("utf8");
      err.push(text);
      call("stderr", text);
    });
    child.on("error", function (error) { call("error", error); });
    child.on("close", function (value) {
      code = value;
      call("close", value);
    });

    return {
      pid: child.pid,
      // The ones who watch the program are told what happens.
      on: function (event, fn) {
        var name = String(event);
        if (!listeners[name]) listeners[name] = [];
        listeners[name].push(fn);
        return this;
      },
      // The program is ended.
      kill: function (signal) {
        child.kill(signal === undefined ? undefined : String(signal));
        return this;
      },
      // The program is waited for.
      wait: function () {
        return new Promise(function (resolve) {
          if (code !== null) {
            resolve({ code: code, stdout: out.join(""), stderr: err.join(""), ok: code === 0 });
            return;
          }
          child.on("close", function (value) {
            resolve({ code: value, stdout: out.join(""), stderr: err.join(""), ok: value === 0 });
          });
        });
      }
    };
  }

  // Another program is run, and the answer of it is waited for.
  function run(command, args) {
    if (!isNode) throw veyError("process.run() is only available in Node.");
    var child = spawn(command, args);
    return child.wait();
  }

  // What is known about the process that runs the program.
  //
  // The name is not `process`, because that would hide the global of Node.
  var nodeProcess = {
    cwd: function () {
      needNode("process.cwd()");
      return nodeModule("node:process").cwd();
    },
    argv: function () {
      needNode("process.argv()");
      return nodeModule("node:process").argv.slice();
    },
    platform: function () {
      needNode("process.platform()");
      return nodeModule("node:process").platform;
    },
    exit: function (code) {
      needNode("process.exit()");
      nodeModule("node:process").exit(code === undefined ? 0 : code);
    },
    // Another program is run.
    spawn: spawn,
    // Another program is run, and the answer of it is waited for.
    run: run
  };

  var fs = {
    read: function (path, encoding) {
      needNode("fs.read()");
      return nodeModule("node:fs").readFileSync(path, encoding || "utf8");
    },
    write: function (path, value) {
      needNode("fs.write()");
      nodeModule("node:fs").writeFileSync(path, toText(value), "utf8");
    },
    append: function (path, value) {
      needNode("fs.append()");
      nodeModule("node:fs").appendFileSync(path, toText(value), "utf8");
    },
    exists: function (path) {
      needNode("fs.exists()");
      return nodeModule("node:fs").existsSync(path);
    },
    list: function (path) {
      needNode("fs.list()");
      return nodeModule("node:fs").readdirSync(path);
    },
    remove: function (path) {
      needNode("fs.remove()");
      nodeModule("node:fs").rmSync(path, { recursive: true, force: true });
    },
    mkdir: function (path) {
      needNode("fs.mkdir()");
      nodeModule("node:fs").mkdirSync(path, { recursive: true });
    },
    // A file is read as it comes, piece by piece.
    readStream: function (path) {
      needNode("fs.readStream()");
      if (!isNode) throw veyError("fs.readStream() is only available in Node.");
      var stream = nodeModule("node:fs").createReadStream(path);
      var listeners = {};
      var chunks = [];
      var text = "";
      var ended = false;

      function call(event, value) {
        var list = listeners[event];
        if (!list) return;
        for (var i = 0; i < list.length; i++) list[i](value);
      }

      function finish(error) {
        if (ended) return;
        ended = true;
        if (error) {
          call("error", error);
          return;
        }
        text = chunks.join("");
        call("end", text);
      }

      stream.on("data", function (chunk) {
        chunks.push(chunk.toString("utf8"));
        call("data", chunk.toString("utf8"));
      });
      stream.on("error", finish);
      stream.on("end", function () { finish(null); });

      return {
        // The ones who watch the file are told what happens.
        on: function (event, fn) {
          var name = String(event);
          if (!listeners[name]) listeners[name] = [];
          listeners[name].push(fn);
          return this;
        },
        // The whole file is read.
        read: function () {
          return new Promise(function (resolve, reject) {
            if (ended) {
              resolve(text);
              return;
            }
            listeners.end = listeners.end || [];
            listeners.end.push(resolve);
            listeners.error = listeners.error || [];
            listeners.error.push(reject);
          });
        },
        // The file is closed.
        close: function () {
          stream.close();
          return this;
        }
      };
    },
    // A file is written as it comes.
    writeStream: function (path, value) {
      needNode("fs.writeStream()");
      if (!isNode) throw veyError("fs.writeStream() is only available in Node.");
      return new Promise(function (resolve, reject) {
        var stream = nodeModule("node:fs").createWriteStream(path);
        stream.on("error", reject);
        stream.on("finish", function () { resolve(); });
        stream.end(toText(value));
      });
    }
  };

  // The bytes that are random, and the signatures of the values.
  var crypto = {
    // The bytes that are random.
    randomBytes: function (count) {
      needNode("crypto.randomBytes()");
      if (!isNode) throw veyError("crypto.randomBytes() is only available in Node.");
      var size = Math.max(0, Math.floor(Number(count) || 0));
      var bytes = nodeModule("node:crypto").randomBytes(size);
      var out = [];
      for (var i = 0; i < bytes.length; i++) out.push(bytes[i]);
      return out;
    },

    // The signature of a value.
    //
    // A key that is a text is a secret, and the signature is made with it.
    // A key that is a key of a pair is a key of a pair, and the signature is
    // made with it.
    sign: function (value, key) {
      needNode("crypto.sign()");
      if (!isNode) throw veyError("crypto.sign() is only available in Node.");
      var text = toText(value);
      var secret = String(key);
      var module_ = nodeModule("node:crypto");
      if (secret.indexOf("-----BEGIN") === 0) {
        var signer = module_.createSign("sha256");
        signer.update(text);
        return signer.sign(secret, "base64");
      }
      return module_.createHmac("sha256", secret).update(text).digest("hex");
    },

    // Whether the signature is the signature of the value.
    verify: function (value, signature, key) {
      needNode("crypto.verify()");
      if (!isNode) throw veyError("crypto.verify() is only available in Node.");
      var text = toText(value);
      var secret = String(key);
      var given = String(signature);
      var module_ = nodeModule("node:crypto");
      if (secret.indexOf("-----BEGIN") === 0) {
        var verifier = module_.createVerify("sha256");
        verifier.update(text);
        try {
          return verifier.verify(secret, given, "base64");
        } catch (err) {
          return false;
        }
      }
      var expected = module_.createHmac("sha256", secret).update(text).digest("hex");
      return expected === given;
    },

    // A pair of keys, one that is open and one that is not.
    keyPair: function () {
      needNode("crypto.keyPair()");
      if (!isNode) throw veyError("crypto.keyPair() is only available in Node.");
      var pair = nodeModule("node:crypto").generateKeyPairSync("ec", {
        namedCurve: "P-256",
        publicKeyEncoding: { type: "spki", format: "pem" },
        privateKeyEncoding: { type: "pkcs8", format: "pem" }
      });
      return {
        publicKey: pair.publicKey,
        privateKey: pair.privateKey
      };
    }
  };

  // The files that are smaller when they are compressed.
  var zlib = {
    // The bytes of a value in the compressed form of it.
    gzip: function (value) {
      needNode("zlib.gzip()");
      if (!isNode) throw veyError("zlib.gzip() is only available in Node.");
      return toBytes(nodeModule("node:zlib").gzipSync(Buffer.from(toText(value), "utf8")));
    },

    // The value of the bytes that were compressed.
    gunzip: function (value) {
      needNode("zlib.gunzip()");
      if (!isNode) throw veyError("zlib.gunzip() is only available in Node.");
      return utf8Decode(nodeModule("node:zlib").gunzipSync(Buffer.from(toBytes(value))));
    },

    // The bytes of a value in the other compressed form of it.
    deflate: function (value) {
      needNode("zlib.deflate()");
      if (!isNode) throw veyError("zlib.deflate() is only available in Node.");
      return toBytes(nodeModule("node:zlib").deflateSync(Buffer.from(toText(value), "utf8")));
    },

    // The value of the bytes that were compressed.
    inflate: function (value) {
      needNode("zlib.inflate()");
      if (!isNode) throw veyError("zlib.inflate() is only available in Node.");
      return utf8Decode(nodeModule("node:zlib").inflateSync(Buffer.from(toBytes(value))));
    }
  };

  var path = {
    join: function () {
      needNode("path.join()");
      var parts = Array.prototype.slice.call(arguments);
      return nodeModule("node:path").join.apply(null, parts);
    },
    dirname: function (value) {
      needNode("path.dirname()");
      return nodeModule("node:path").dirname(value);
    },
    basename: function (value) {
      needNode("path.basename()");
      return nodeModule("node:path").basename(value);
    },
    extname: function (value) {
      needNode("path.extname()");
      return nodeModule("node:path").extname(value);
    },
    resolve: function () {
      needNode("path.resolve()");
      var parts = Array.prototype.slice.call(arguments);
      return nodeModule("node:path").resolve.apply(null, parts);
    }
  };

  ctx.fs = fs;
  ctx.path = path;
  ctx.env = env;
  ctx.nodeModule = nodeModule;
  ctx.require = require;
  ctx.hash = hash;
  ctx.process = nodeProcess;
  ctx.crypto = crypto;
  ctx.zlib = zlib;

  return {
    fs: fs,
    path: path,
    env: env,
    require: require,
    hash: hash,
    process: nodeProcess,
    crypto: crypto,
    zlib: zlib,
  };
}
