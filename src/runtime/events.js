// The events of one's own: a listener, a listener that watches, the life of
// a node and a way to hold it.
function veyEvents(ctx) {
  var doc = ctx.doc;
  var veyError = ctx.veyError;
  var queueMount = ctx.queueMount;
  var queueDestroy = ctx.queueDestroy;

  // The node of the page, when the name of it is a name.
  function elementOf(target) {
    if (!doc) {
      throw veyError("the events of a node are only available in the browser.");
    }
    if (typeof target === "string") {
      var found = doc.querySelector(target);
      if (!found) throw new Error("element not found: " + target);
      return found;
    }
    if (!target) throw new Error("element not found: " + target);
    return target;
  }

  // A listener of an event of a node.
  //
  // The function it returns takes the listener away again.
  function on(target, event, handler) {
    var el = elementOf(target);
    var name = String(event);
    el.addEventListener(name, handler);
    return function () {
      el.removeEventListener(name, handler);
    };
  }

  // A listener that watches the children of a node.
  //
  // The handler is called with the event and the node that was hit, when
  // the node is of the given name.
  function delegate(target, event, selector, handler) {
    var el = elementOf(target);
    var name = String(event);
    function listener(event_) {
      var node = event_.target;
      while (node && node !== el) {
        if (node.matches && node.matches(String(selector))) {
          handler(event_, node);
          return;
        }
        node = node.parentNode;
      }
    }
    el.addEventListener(name, listener);
    return function () {
      el.removeEventListener(name, listener);
    };
  }

  // The work that is done when the page is there.
  //
  // The work belongs to the component that is written now, so that it is
  // done once, and not once for every writing of the page.
  function onMount(fn) {
    var works = ctx.currentComponent;
    if (works) {
      works.mount.push(fn);
      return function () { };
    }
    return queueMount(fn);
  }

  // The work that is done when the page is taken away.
  //
  // The work belongs to the component that is written now, so that it is
  // done when the nodes of the component are taken away.
  function onDestroy(fn) {
    var works = ctx.currentComponent;
    if (works) {
      works.destroy.push(fn);
      return function () { };
    }
    return queueDestroy(fn);
  }

  // A place where a node may be kept.
  //
  // The reference is filled when the node is written in the page.
  function ref() {
    return {
      value: null,
      set: function (node) {
        this.value = node;
      }
    };
  }

  ctx.on = on;
  ctx.delegate = delegate;
  ctx.onMount = onMount;
  ctx.onDestroy = onDestroy;
  ctx.ref = ref;

  return {
    on: on,
    delegate: delegate,
    onMount: onMount,
    onDestroy: onDestroy,
    ref: ref,
  };
}
