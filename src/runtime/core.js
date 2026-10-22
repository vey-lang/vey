// The console, the conversions and the tests.
function veyCore(ctx) {
  var isNode = ctx.isNode;
  var nodeModule = ctx.nodeModule;
  var log = ctx.log;
  var scope = ctx.scope;
  var veyError = ctx.veyError;

  function print() {
    var args = Array.prototype.slice.call(arguments).map(function (value) {
      return value && value.__veyEnum ? value.toString() : value;
    });
    log.log.apply(log, args);
  }

  function input(prompt) {
    if (!isNode) throw veyError("input() is only available in Node.");
    if (prompt) nodeModule("node:process").stdout.write(String(prompt));
    var fs = nodeModule("node:fs");
    // Read one byte at a time so that only the current line is consumed and
    // the rest of stdin stays available for the next call.
    var one = Buffer.alloc(1);
    var bytes = [];
    for (;;) {
      var count = fs.readSync(0, one, 0, 1, null);
      if (count === 0) break;
      var ch = one.toString("utf8", 0, 1);
      if (ch === "\n") break;
      bytes.push(one[0]);
    }
    var line = Buffer.from(bytes).toString("utf8");
    return line.replace(/\r$/, "");
  }

  function sleep(ms) {
    return new Promise(function (resolve) { setTimeout(resolve, ms); });
  }

  function str(value) {
    if (value === null) return "null";
    if (value === undefined) return "undefined";
    if (value && value.__veyEnum) return value.toString();
    if (typeof value === "object") {
      try {
        return JSON.stringify(value, null, 2);
      } catch (err) {
        return Object.prototype.toString.call(value);
      }
    }
    return String(value);
  }

  function num(value) { return Number(value); }

  function int(value) { return parseInt(value, 10); }

  function bool(value) { return Boolean(value); }

  function type(value) {
    if (value === null) return "null";
    if (value === undefined) return "undefined";
    if (value && value.__veyEnum) return "enum";
    if (Array.isArray(value)) return "list";
    var kind = typeof value;
    if (kind === "number") return "number";
    if (kind === "string") return "string";
    if (kind === "boolean") return "boolean";
    if (kind === "function") return "function";
    return "object";
  }

  function len(value) {
    if (value === null || value === undefined) return 0;
    if (typeof value === "string" || Array.isArray(value)) return value.length;
    return Object.keys(value).length;
  }

  function error(message) {
    throw new Error(String(message));
  }

  function assert(condition, message) {
    if (!condition) {
      throw new Error(message === undefined ? "assertion failed" : String(message));
    }
  }

  var pendingTests = 0;

  var testKeepAlive = null;

  function startTestKeepAlive() {
    if (!isNode || testKeepAlive) return;
    testKeepAlive = setInterval(function () {}, 1000);
  }

  function finishTest() {
    pendingTests -= 1;
    if (pendingTests <= 0 && testKeepAlive) {
      clearInterval(testKeepAlive);
      testKeepAlive = null;
    }
  }

  function test(name, fn) {
    try {
      var result = fn();
      if (result && typeof result.then === "function") {
        pendingTests += 1;
        startTestKeepAlive();
        return result.then(
          function () {
            try {
              log.log("ok " + name);
            } finally {
              finishTest();
            }
          },
          function (err) {
            try {
              failTest(name, err);
            } finally {
              finishTest();
            }
          }
        );
      }
      log.log("ok " + name);
    } catch (err) {
      failTest(name, err);
    }
  }

  function failTest(name, err) {
    log.error("fail " + name);
    log.error(testFailure(err));
    if (isNode) nodeModule("node:process").exitCode = 1;
  }

  function testFailure(err) {
    if (!err) return "unknown error";
    var message = err && err.message !== undefined ? String(err.message) : String(err);
    var stack = err && err.stack ? String(err.stack) : "";
    var place = /\(([^()]*\.vey:\d+:\d+)\)/.exec(stack);
    if (place) {
      return message + "\n  " + place[1];
    }
    return message;
  }

  function all(list) {
    return Promise.all(list || []);
  }

  function race(list) {
    return Promise.race(list || []);
  }

  function uuid() {
    if (isNode) {
      var cryptoModule = nodeModule("node:crypto");
      if (cryptoModule.randomUUID) return cryptoModule.randomUUID();
    }
    if (scope.crypto && scope.crypto.randomUUID) return scope.crypto.randomUUID();
    return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, function (ch) {
      var value = Math.random() * 16 | 0;
      var nibble = ch === "x" ? value : (value & 0x3 | 0x8);
      return nibble.toString(16);
    });
  }

  function random(min, max) {
    if (min === undefined && max === undefined) return Math.random();
    if (max === undefined) { max = min; min = 0; }
    return Math.floor(Math.random() * (max - min + 1)) + min;
  }

  function clamp(value, min, max) {
    return Math.min(Math.max(value, min), max);
  }

  ctx.print = print;
  ctx.input = input;
  ctx.sleep = sleep;
  ctx.str = str;
  ctx.num = num;
  ctx.int = int;
  ctx.bool = bool;
  ctx.type = type;
  ctx.len = len;
  ctx.error = error;
  ctx.assert = assert;
  ctx.test = test;
  ctx.all = all;
  ctx.race = race;
  ctx.uuid = uuid;
  ctx.random = random;
  ctx.clamp = clamp;

  return {
    print: print,
    input: input,
    sleep: sleep,
    str: str,
    num: num,
    int: int,
    bool: bool,
    type: type,
    len: len,
    error: error,
    assert: assert,
    test: test,
    all: all,
    race: race,
    uuid: uuid,
    random: random,
    clamp: clamp,
  };
}
