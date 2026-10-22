// The state of a page: a value that tells when it changes, a state that is
// shared and the work that follows a change.
function veyState(ctx) {
  // The effect that is running, so that the values it reads are known.
  var running = null;

  // A signal tells the ones that listen to it that its value has changed.
  function makeSignal(value) {
    var listeners = [];
    var current = value;

    function setValue(next) {
      // A value that is the same is not a change.
      if (next === current) return current;
      current = next;
      var list = listeners.slice();
      for (var i = 0; i < list.length; i++) list[i](current);
      return current;
    }

    var sig = {};
    Object.defineProperty(sig, "value", {
      get: function () {
        watch(sig);
        return current;
      },
      set: setValue,
      enumerable: true
    });

    sig.set = setValue;
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
      listeners.push(fn);
      return function () {
        var index = listeners.indexOf(fn);
        if (index !== -1) listeners.splice(index, 1);
      };
    };
    return sig;
  }

  // A value that tells when it changes.
  function signal(value) {
    return makeSignal(value);
  }

  // The stores that are shared, by their name.
  var stores = {};

  function makeStore(state) {
    var listeners = [];
    var current = state;

    function setState(next) {
      current = next;
      var list = listeners.slice();
      for (var i = 0; i < list.length; i++) list[i](current);
      return current;
    }

    var store = {};
    Object.defineProperty(store, "value", {
      get: function () {
        watch(store);
        return current;
      },
      set: setState,
      enumerable: true
    });

    // One field of the state is changed.
    store.set = function (key, value) {
      var next = {};
      for (var name in current) {
        if (Object.prototype.hasOwnProperty.call(current, name)) next[name] = current[name];
      }
      next[key] = value;
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
      listeners.push(fn);
      return function () {
        var index = listeners.indexOf(fn);
        if (index !== -1) listeners.splice(index, 1);
      };
    };
    return store;
  }

  // A state that is shared.
  //
  // A store with a name is the same store everywhere it is asked for, so
  // that two parts of a page may share it. A store without a name belongs
  // to the one that made it.
  function store(name, initial) {
    if (name !== null && typeof name === "object") {
      initial = name;
      name = null;
    }
    if (name === undefined || name === null) {
      return makeStore(initial === undefined ? {} : initial);
    }
    var key = String(name);
    if (stores[key]) return stores[key];
    var made = makeStore(initial === undefined ? {} : initial);
    stores[key] = made;
    return made;
  }

  // The effect that is running listens to the values it reads.
  function watch(value) {
    if (!running) return;
    for (var i = 0; i < running.deps.length; i++) {
      if (running.deps[i].value === value) return;
    }
    var off = value.subscribe(running.run);
    running.deps.push({ value: value, off: off });
  }

  // The work that follows a change.
  //
  // The effect runs once at the beginning, and again whenever a value it
  // read has changed. The function it runs may return a function that is
  // called before it runs again.
  function effect(fn) {
    var deps = [];
    var stopped = false;
    var cleanup = null;

    function run() {
      if (stopped) return;
      // The values of the last time are let go, so that a value that is not
      // read any more does not wake the effect up.
      for (var i = 0; i < deps.length; i++) deps[i].off();
      deps = [];
      var previous = running;
      running = { deps: deps, run: run };
      try {
        cleanup = fn();
      } finally {
        running = previous;
      }
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

  return {
    signal: signal,
    store: store,
    effect: effect,
  };
}
