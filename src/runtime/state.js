// The state of a page: a value that tells when it changes, a state that is
// shared and the work that follows a change.
function veyState(ctx) {
  var log = ctx.log;

  // The effect that is running, so that the values it reads are known.
  var running = null;

  // Effects normally run as soon as a signal changes. A batch gathers the
  // effects touched by several writes and runs each one once at the end.
  var batchDepth = 0;
  var batched = [];

  function flushBatch() {
    while (batched.length) {
      var list = batched;
      batched = [];
      for (var i = 0; i < list.length; i++) list[i]();
    }
  }

  function batch(fn) {
    batchDepth += 1;
    try {
      return fn();
    } finally {
      batchDepth -= 1;
      if (batchDepth === 0) flushBatch();
    }
  }

  // Read values without adding dependencies to the current effect.
  function untrack(fn) {
    var previous = running;
    running = null;
    try {
      return fn();
    } finally {
      running = previous;
    }
  }

  // The work that is waiting to be done.
  //
  // Ten writes of ten signals do the work once, and not ten times, so
  // that a page is written once and not once for every change.
  var scheduled = false;
  var waiting = [];

  function later(fn) {
    waiting.push(fn);
    if (scheduled) return;
    scheduled = true;
    var run = function () {
      scheduled = false;
      var list = waiting;
      waiting = [];
      for (var i = 0; i < list.length; i++) {
        try {
          list[i]();
        } catch (err) {
          log.error(err);
        }
      }
    };
    if (typeof queueMicrotask === "function") {
      queueMicrotask(run);
    } else if (typeof Promise === "function") {
      Promise.resolve().then(run);
    } else {
      run();
    }
  }

  // Do the work that is waiting, now.
  function flush() {
    if (!scheduled && waiting.length === 0) return;
    scheduled = false;
    var list = waiting;
    waiting = [];
    for (var i = 0; i < list.length; i++) {
      try {
        list[i]();
      } catch (err) {
        log.error(err);
      }
    }
  }

  // Whether two values are the same.
  //
  // A value that is the same is not a change, so the page is not written
  // for nothing.
  function defaultEquals(a, b) {
    if (a === b) return true;
    // A number that is not a number is the same as itself, and a zero is
    // not the other zero.
    if (a !== a && b !== b) return true;
    if (a === 0 && b === 0) return 1 / a === 1 / b;
    return false;
  }

  // The equality of a value, by the way it is asked for.
  function equalsOf(options) {
    var opts = options || {};
    if (typeof opts.equals === "function") return opts.equals;
    if (opts.equals === "deep") return deepEquals;
    return defaultEquals;
  }

  // Whether two values have the same content.
  function deepEquals(a, b) {
    if (defaultEquals(a, b)) return true;
    if (a === null || b === null || a === undefined || b === undefined) return false;
    if (typeof a !== "object" || typeof b !== "object") return false;
    if (Array.isArray(a) !== Array.isArray(b)) return false;
    if (Array.isArray(a)) {
      if (a.length !== b.length) return false;
      for (var i = 0; i < a.length; i++) {
        if (!deepEquals(a[i], b[i])) return false;
      }
      return true;
    }
    var keys = Object.keys(a);
    if (keys.length !== Object.keys(b).length) return false;
    for (var k = 0; k < keys.length; k++) {
      if (!Object.prototype.hasOwnProperty.call(b, keys[k])) return false;
      if (!deepEquals(a[keys[k]], b[keys[k]])) return false;
    }
    return true;
  }

  // A signal tells the ones that listen to it that its value has changed.
  function makeSignal(value, options) {
    var equals = equalsOf(options);
    var current = value;

    var sig = {};
    Object.defineProperty(sig, "__veyValue", {
      value: current,
      writable: true,
      enumerable: false,
      configurable: true
    });
    Object.defineProperty(sig, "__veyListeners", {
      value: [],
      writable: true,
      enumerable: false,
      configurable: true
    });
    Object.defineProperty(sig, "__veyPending", {
      value: false,
      writable: true,
      enumerable: false,
      configurable: true
    });

    function setValue(next) {
      // A value that is the same is not a change.
      if (equals(current, next)) return current;
      current = next;
      sig.__veyValue = current;
      // The ones that listen are told at once.
      var listeners = sig.__veyListeners.slice();
      for (var i = 0; i < listeners.length; i++) {
        try {
          listeners[i](current);
        } catch (err) {
          log.error(err);
        }
      }
      return current;
    }

    Object.defineProperty(sig, "value", {
      get: function () {
        watch(sig);
        return current;
      },
      set: setValue,
      enumerable: true
    });

    sig.set = setValue;
    // The signal is written as the value it has, so that a state may be
    // written in the page without its value being taken out of it.
    sig.toString = function () {
      return String(current);
    };
    // The value is changed by a function of the value it has now.
    sig.update = function (fn) {
      return setValue(fn(current));
    };
    // The value is read without listening to it.
    sig.peek = function () {
      return current;
    };
    // The ones that listen are called when the value changes.
    sig.subscribe = function (fn) {
      sig.__veyListeners.push(fn);
      return function () {
        var index = sig.__veyListeners.indexOf(fn);
        if (index !== -1) sig.__veyListeners.splice(index, 1);
      };
    };
    return sig;
  }

  // A signal tells the ones that listen to it that its value has changed.
  function signal(value, options) {
    return makeSignal(value, options);
  }

  // The stores that are shared, by their name.
  var stores = {};

  function makeStore(state, options) {
    var equals = equalsOf(options);
    var current = state;

    var store = {};
    Object.defineProperty(store, "__veyValue", {
      value: current,
      writable: true,
      enumerable: false,
      configurable: true
    });
    Object.defineProperty(store, "__veyListeners", {
      value: [],
      writable: true,
      enumerable: false,
      configurable: true
    });
    Object.defineProperty(store, "__veyPending", {
      value: false,
      writable: true,
      enumerable: false,
      configurable: true
    });

    function setState(next) {
      // A state that is the same is not a change.
      if (equals(current, next)) return current;
      current = next;
      store.__veyValue = current;
      var listeners = store.__veyListeners.slice();
      for (var i = 0; i < listeners.length; i++) {
        try {
          listeners[i](current);
        } catch (err) {
          log.error(err);
        }
      }
      return current;
    }

    Object.defineProperty(store, "value", {
      get: function () {
        watch(store);
        return current;
      },
      set: setState,
      enumerable: true
    });

    // The store is written as the state it has.
    store.toString = function () {
      return String(current);
    };
    // One field of the state is changed.
    store.set = function (key, value) {
      var parts = String(key).split(".");
      var next = Array.isArray(current) ? current.slice() : {};
      for (var name in current) {
        if (Object.prototype.hasOwnProperty.call(current, name)) next[name] = current[name];
      }
      var target = next;
      for (var p = 0; p < parts.length - 1; p++) {
        var part = parts[p];
        var child = target[part];
        var copy = Array.isArray(child) ? child.slice() : {};
        if (child && typeof child === "object") {
          for (var childName in child) {
            if (Object.prototype.hasOwnProperty.call(child, childName)) copy[childName] = child[childName];
          }
        }
        target[part] = copy;
        target = copy;
      }
      target[parts[parts.length - 1]] = value;
      return setState(next);
    };
    // The whole state is changed.
    store.replace = setState;
    // The state is changed by a function of the state it has now.
    store.update = function (fn) {
      return setState(fn(current));
    };
    // The state is read without listening to it.
    store.peek = function () {
      return current;
    };
    // The ones that listen are called when the state changes.
    store.subscribe = function (fn) {
      store.__veyListeners.push(fn);
      return function () {
        var index = store.__veyListeners.indexOf(fn);
        if (index !== -1) store.__veyListeners.splice(index, 1);
      };
    };
    return store;
  }

  // A value that follows other values.
  //
  // The function is called again when one of the values it read has
  // changed. The value is a signal of its own, so it may be read and
  // written like one.
  function computed(fn) {
    var sig = makeSignal(undefined);
    var stop = effect(function () {
      sig.set(fn());
    });
    // The work is let go when the value is not wanted any more.
    sig.dispose = function () {
      if (stop) {
        stop();
        stop = null;
      }
    };
    return sig;
  }

  // A state that is shared.
  //
  // A store with a name is the same store everywhere it is asked for, so
  // that two parts of a page may share it. A store without a name belongs
  // to the one that made it.
  function store(name, initial, options) {
    if (name !== null && typeof name === "object") {
      options = initial;
      initial = name;
      name = null;
    }
    if (name === undefined || name === null) {
      return makeStore(initial === undefined ? {} : initial, options);
    }
    var key = String(name);
    if (stores[key]) return stores[key];
    var made = makeStore(initial === undefined ? {} : initial, options);
    stores[key] = made;
    return made;
  }

  // The effect that is running listens to the values it reads.
  function watch(value) {
    if (!running) return;
    for (var i = 0; i < running.deps.length; i++) {
      if (running.deps[i].value === value) return;
    }
    var off = value.subscribe(running.scheduled);
    running.deps.push({ value: value, off: off });
  }

  // The work that follows a change.
  //
  // The effect runs once at the beginning, and again whenever a value it
  // read has changed. The function it runs may return a function that is
  // called before it runs again.
  function effect(fn, options) {
    var opts = options || {};
    var deps = [];
    var stopped = false;
    var cleanup = null;
    var waiting = false;

    // The work of the effect may be gathered, so that many changes do the
    // work once.
    function scheduledRun() {
      if (stopped) return;
      if (batchDepth > 0) {
        if (batched.indexOf(scheduledRun) === -1) batched.push(scheduledRun);
        return;
      }
      if (!opts.defer) {
        run();
        return;
      }
      if (waiting) return;
      waiting = true;
      later(function () {
        waiting = false;
        run();
      });
    }

    function run() {
      if (stopped) return;
      // The work of the last time is let go, before the new one begins.
      if (typeof cleanup === "function") {
        var done = cleanup;
        cleanup = null;
        done();
      }
      // The values of the last time are let go before the new work
      // begins, so that a value is not watched twice.
      var oldDeps = deps;
      deps = [];
      for (var i = 0; i < oldDeps.length; i++) oldDeps[i].off();
      var previous = running;
      running = { deps: deps, run: run, scheduled: scheduledRun };
      try {
        cleanup = fn();
      } catch (err) {
        // The values of the last time are watched again, so that a mistake
        // does not leave the page dead.
        for (var k = 0; k < oldDeps.length; k++) {
          var value = oldDeps[k].value;
          oldDeps[k] = { value: value, off: value.subscribe(scheduledRun) };
        }
        deps = oldDeps;
        running = previous;
        throw err;
      }
      running = previous;
    }

    run();

    // The effect is ended, and the work it began is let go.
    function stop() {
      if (stopped) return;
      stopped = true;
      for (var i = 0; i < deps.length; i++) deps[i].off();
      deps = [];
      if (typeof cleanup === "function") {
        var done = cleanup;
        cleanup = null;
        done();
      }
    }

    return stop;
  }

  ctx.signal = signal;
  ctx.store = store;
  ctx.effect = effect;
  ctx.computed = computed;
  ctx.batch = batch;
  ctx.untrack = untrack;
  // The comparison of two values by their content, so that a part of the
  // runtime that follows the state may use it.
  ctx.deepEquals = deepEquals;
  // The work that is waiting is done at once, so that a page is written
  // once for many changes.
  ctx.flushState = flush;
  // The work that is waiting is done at the end of the turn, so that many
  // changes do the work once.
  ctx.laterState = later;

  return {
    signal: signal,
    store: store,
    effect: effect,
    computed: computed,
    batch: batch,
    untrack: untrack,
    deepEquals: deepEquals,
  };
}
