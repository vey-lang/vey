// The parts of Node: the file system, the paths, the environment and the
// packages of npm.
function veyNode(ctx) {
  var isNode = ctx.isNode;
  var req = ctx.req;
  var toText = ctx.toText;
  var veyError = ctx.veyError;

  function env(name, fallback) {
    // The module of Node is asked for the environment, not the ambient
    // `process`, which a bundler or a test runner may have replaced.
    var value = isNode ? nodeModule("node:process").env[name] : undefined;
    if (value === undefined) {
      return fallback === undefined ? null : fallback;
    }
    return value;
  }

  function nodeModule(name) {
    if (!isNode) throw veyError(name + " is only available in Node.");
    return req(name);
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

  // What is known about the process that runs the program.
  //
  // The name is not `process`, because that would hide the global of Node.
  var nodeProcess = {
    cwd: function () {
      return nodeModule("node:process").cwd();
    },
    argv: function () {
      return nodeModule("node:process").argv.slice();
    },
    platform: function () {
      return nodeModule("node:process").platform;
    },
    exit: function (code) {
      nodeModule("node:process").exit(code === undefined ? 0 : code);
    }
  };

  var fs = {
    read: function (path, encoding) {
      return nodeModule("node:fs").readFileSync(path, encoding || "utf8");
    },
    write: function (path, value) {
      nodeModule("node:fs").writeFileSync(path, toText(value), "utf8");
    },
    append: function (path, value) {
      nodeModule("node:fs").appendFileSync(path, toText(value), "utf8");
    },
    exists: function (path) {
      return nodeModule("node:fs").existsSync(path);
    },
    list: function (path) {
      return nodeModule("node:fs").readdirSync(path);
    },
    remove: function (path) {
      nodeModule("node:fs").rmSync(path, { recursive: true, force: true });
    },
    mkdir: function (path) {
      nodeModule("node:fs").mkdirSync(path, { recursive: true });
    }
  };

  var path = {
    join: function () {
      var parts = Array.prototype.slice.call(arguments);
      return nodeModule("node:path").join.apply(null, parts);
    },
    dirname: function (value) {
      return nodeModule("node:path").dirname(value);
    },
    basename: function (value) {
      return nodeModule("node:path").basename(value);
    },
    extname: function (value) {
      return nodeModule("node:path").extname(value);
    },
    resolve: function () {
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

  return {
    fs: fs,
    path: path,
    env: env,
    require: require,
    hash: hash,
    process: nodeProcess,
  };
}
