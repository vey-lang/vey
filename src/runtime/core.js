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

  // The matchers of the tests. They say what is wrong when they fail, so
  // that a failure may be read without running the test again.
  function show(value) {
    if (typeof value === "string") return JSON.stringify(value);
    try {
      return JSON.stringify(value);
    } catch (err) {
      return String(value);
    }
  }

  function assertEquals(actual, expected, message) {
    if (actual !== expected) {
      throw new Error(
        (message === undefined ? "" : String(message) + ": ") +
        "expected " + show(expected) + ", found " + show(actual)
      );
    }
  }

  function assertNotEquals(actual, expected, message) {
    if (actual === expected) {
      throw new Error(
        (message === undefined ? "" : String(message) + ": ") +
        "did not expect " + show(expected)
      );
    }
  }

  function assertDeepEquals(actual, expected, message) {
    var a = JSON.stringify(actual);
    var b = JSON.stringify(expected);
    if (a !== b) {
      throw new Error(
        (message === undefined ? "" : String(message) + ": ") +
        "expected " + b + ", found " + a
      );
    }
  }

  function assertContains(haystack, needle, message) {
    var text = String(haystack);
    if (text.indexOf(String(needle)) === -1) {
      throw new Error(
        (message === undefined ? "" : String(message) + ": ") +
        show(text) + " does not hold " + show(needle)
      );
    }
  }

  function assertNotContains(haystack, needle, message) {
    var text = String(haystack);
    if (text.indexOf(String(needle)) !== -1) {
      throw new Error(
        (message === undefined ? "" : String(message) + ": ") +
        show(text) + " holds " + show(needle)
      );
    }
  }

  function assertThrows(fn, message) {
    var threw = false;
    try {
      fn();
    } catch (err) {
      threw = true;
    }
    if (!threw) {
      throw new Error(
        (message === undefined ? "" : String(message) + ": ") +
        "the code was expected to throw"
      );
    }
  }

  function assertMatch(text, pattern, message) {
    var regex = pattern instanceof RegExp ? pattern : new RegExp(String(pattern));
    if (!regex.test(String(text))) {
      throw new Error(
        (message === undefined ? "" : String(message) + ": ") +
        show(text) + " does not match " + show(pattern)
      );
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

  // The name of the tests that are run. When it is empty, every test runs.
  function testFilter() {
    if (!isNode) return "";
    var env = nodeModule("node:process").env;
    return env.VEY_TEST_FILTER || "";
  }

  function test(name, fn) {
    var filter = testFilter();
    if (filter && String(name).indexOf(filter) === -1) {
      return;
    }
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

  // The answer of a function that may fail.
  //
  // The value says whether it worked, and what it was: the answer or the
  // reason it did not work.
  function ok(value) {
    return { __veyResult: true, ok: true, value: value, error: null };
  }

  function err(error) {
    return { __veyResult: true, ok: false, value: null, error: error };
  }

  // Hand the failure of a value on.
  //
  // The failure is thrown, and the function that is around catches it and
  // hands it on in turn.
  function propagate(value) {
    if (value === null || value === undefined) {
      var nothing = new Error("there is nothing here");
      nothing.__veyNothing = true;
      throw nothing;
    }
    if (value && value.__veyResult === true) {
      if (!value.ok) {
        // The reason of the failure is the message of the error, so that a
        // `try` of the program may show it.
        var failed = new Error(
          value.error === undefined ? "the call failed" : String(value.error)
        );
        failed.__veyFailure = value.error;
        throw failed;
      }
      return value.value;
    }
    return value;
  }

  // A piece of work that is not finished.
  function todo(message) {
    throw new Error(
      "not finished yet" + (message === undefined ? "" : ": " + String(message))
    );
  }

  // A piece of work that is not written.
  function unimplemented(message) {
    throw new Error(
      "not written yet" + (message === undefined ? "" : ": " + String(message))
    );
  }

  // The kind of a value, which is what a check of a type looks at.
  function is(value, kind) {
    if (kind === "number") return typeof value === "number";
    if (kind === "string") return typeof value === "string";
    if (kind === "boolean") return typeof value === "boolean";
    if (kind === "null") return value === null;
    if (kind === "undefined") return value === undefined;
    if (kind === "list") return Array.isArray(value);
    if (kind === "function") return typeof value === "function";
    if (kind === "object") {
      return value !== null && typeof value === "object" && !Array.isArray(value);
    }
    // A type of the program is an object.
    return value !== null && typeof value === "object";
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
  ctx.assertEquals = assertEquals;
  ctx.assertNotEquals = assertNotEquals;
  ctx.assertDeepEquals = assertDeepEquals;
  ctx.assertContains = assertContains;
  ctx.assertNotContains = assertNotContains;
  ctx.assertThrows = assertThrows;
  ctx.assertMatch = assertMatch;
  ctx.test = test;
  ctx.all = all;
  ctx.race = race;
  ctx.uuid = uuid;
  ctx.random = random;
  ctx.clamp = clamp;
  ctx.ok = ok;
  ctx.err = err;
  ctx.propagate = propagate;
  ctx.failure = err;
  ctx.is = is;
  ctx.todo = todo;
  ctx.unimplemented = unimplemented;

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
    assertEquals: assertEquals,
    assertNotEquals: assertNotEquals,
    assertDeepEquals: assertDeepEquals,
    assertContains: assertContains,
    assertNotContains: assertNotContains,
    assertThrows: assertThrows,
    assertMatch: assertMatch,
    test: test,
    all: all,
    race: race,
    uuid: uuid,
    random: random,
    clamp: clamp,
    ok: ok,
    err: err,
    propagate: propagate,
    failure: err,
    is: is,
    todo: todo,
    unimplemented: unimplemented,
  };
}
