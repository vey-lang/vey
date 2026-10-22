// The difference between two trees, so that a page may change in place and
// a list may be put in another order.
function veyMount(ctx) {
  var doc = ctx.doc;
  var veyError = ctx.veyError;
  var flatten = ctx.flatten;
  var toDom = ctx.toDom;
  var log = ctx.log;

  // The work that waits for the page to be there.
  var mountQueue = [];
  // The work that waits for the page to be taken away.
  var destroyQueue = [];
  // The page that is on the screen, so that the routes may write in it.
  var mounted = null;

  // The children of a node, in the order of the page.
  function childrenOf(parent) {
    var out = [];
    var child = parent.firstChild;
    while (child) {
      out.push(child);
      child = child.nextSibling;
    }
    return out;
  }

  function isText(node) {
    return !!node && node.nodeType === 3;
  }

  function isElement(node) {
    return !!node && node.nodeType === 1;
  }

  // The name of an item of a list, when it has one.
  function keyOf(node) {
    if (!isElement(node)) return null;
    var key = node.__veyKey;
    if (key === undefined || key === null) return null;
    return String(key);
  }

  // The marks of a node are set again, so that a change is seen.
  function syncAttributes(old, neu) {
    var attrs = neu.attributes;
    for (var i = 0; i < attrs.length; i++) {
      var name = attrs[i].name;
      var value = attrs[i].value;
      if (old.getAttribute(name) !== value) old.setAttribute(name, value);
    }
    var oldAttrs = old.attributes;
    for (var j = oldAttrs.length - 1; j >= 0; j--) {
      var oldName = oldAttrs[j].name;
      if (!neu.hasAttribute(oldName)) old.removeAttribute(oldName);
    }
    // The value of a field is a mark of the node, but the visitor may have
    // written something else in it.
    if (doc && doc.activeElement !== old) {
      if (typeof neu.value === "string" && old.value !== neu.value) old.value = neu.value;
      if (typeof neu.checked === "boolean" && old.checked !== neu.checked) old.checked = neu.checked;
    }
  }

  // The listener of a node is moved from the new node to the node that
  // stays, so that the page keeps answering.
  function syncListeners(old, neu) {
    var before = old.__veyListeners || {};
    var after = neu.__veyListeners || {};
    for (var name in before) {
      if (!after[name]) old.removeEventListener(name, before[name]);
    }
    for (var key in after) {
      if (before[key] === after[key]) continue;
      if (before[key]) old.removeEventListener(key, before[key]);
      old.addEventListener(key, after[key]);
    }
    old.__veyListeners = Object.assign({}, after);
  }

  // The name of an item of a list and the reference to a node are of the
  // program, so they are moved as well.
  function syncMarks(old, neu) {
    if (neu.__veyKey !== undefined) old.__veyKey = neu.__veyKey;
    if (neu.__veyRef) {
      old.__veyRef = neu.__veyRef;
      if (typeof neu.__veyRef.set === "function") neu.__veyRef.set(old);
    }
  }

  // The name of the tag of a node.
  function tagOf(node) {
    if (!isElement(node)) return null;
    return String(node.tagName || "").toLowerCase();
  }

  // The text of a node.
  function textOf(node) {
    if (node.nodeValue !== undefined && node.nodeValue !== null) return String(node.nodeValue);
    return node.textContent === undefined ? "" : String(node.textContent);
  }

  function setText(node, text) {
    if (node.nodeValue !== undefined) node.nodeValue = text;
    if (node.textContent !== undefined) node.textContent = text;
  }

  // A node is changed so that it looks like the new one.
  //
  // The node that stays is the one that is changed, so that the page keeps
  // its place, its state and its listeners.
  function patchNode(old, neu) {
    if (isText(old) && isText(neu)) {
      if (textOf(old) !== textOf(neu)) setText(old, textOf(neu));
      return old;
    }
    if (isElement(old) && isElement(neu) && tagOf(old) === tagOf(neu)) {
      syncAttributes(old, neu);
      syncListeners(old, neu);
      syncMarks(old, neu);
      patchChildren(old, childrenOf(neu));
      return old;
    }
    // The node is of another kind, so it is left out for the new one.
    var parent = old.parentNode;
    if (parent) parent.replaceChild(neu, old);
    return neu;
  }

  // The children of a node are changed, so that they look like the new
  // ones.
  //
  // An item that has a name is found by that name, so that a list may grow,
  // shrink and be put in another order.
  function patchChildren(parent, nodes) {
    var old = childrenOf(parent);
    var used = [];
    var pairs = [];

    for (var i = 0; i < nodes.length; i++) {
      var neu = nodes[i];
      var key = keyOf(neu);
      var match = -1;
      if (key !== null) {
        for (var j = 0; j < old.length; j++) {
          if (used[j]) continue;
          if (keyOf(old[j]) === key) {
            match = j;
            break;
          }
        }
      } else {
        // Without a name an item stays where it is.
        match = i;
        if (match >= old.length || used[match]) match = -1;
      }
      if (match === -1) {
        pairs.push({ old: null, neu: neu });
      } else {
        used[match] = true;
        pairs.push({ old: old[match], neu: neu });
      }
    }

    // The children that are not wanted any more are taken away.
    for (var k = 0; k < old.length; k++) {
      if (!used[k] && old[k].parentNode === parent) parent.removeChild(old[k]);
    }

    // The children are put in the order of the new ones.
    var cursor = parent.firstChild;
    for (var m = 0; m < pairs.length; m++) {
      var target = pairs[m].old;
      if (target === null) {
        target = pairs[m].neu;
        parent.insertBefore(target, cursor);
      } else {
        target = patchNode(target, pairs[m].neu);
        if (target !== cursor && target.parentNode === parent) {
          parent.insertBefore(target, cursor);
        }
      }
      cursor = target.nextSibling;
    }
  }

  // The work that waits for the page to be there is done.
  function flushMounts() {
    var list = mountQueue.slice();
    mountQueue.length = 0;
    for (var i = 0; i < list.length; i++) {
      try {
        list[i]();
      } catch (err) {
        log.error(err);
      }
    }
  }

  // The work that waits for the page to be taken away is done.
  function runDestroys() {
    var list = destroyQueue.slice();
    destroyQueue.length = 0;
    for (var i = 0; i < list.length; i++) {
      try {
        list[i]();
      } catch (err) {
        log.error(err);
      }
    }
  }

  // The work that waits for the page to be there is remembered.
  function queueMount(fn) {
    mountQueue.push(fn);
    return function () {
      var index = mountQueue.indexOf(fn);
      if (index !== -1) mountQueue.splice(index, 1);
    };
  }

  // The work that waits for the page to be taken away is remembered.
  function queueDestroy(fn) {
    destroyQueue.push(fn);
    return function () {
      var index = destroyQueue.indexOf(fn);
      if (index !== -1) destroyQueue.splice(index, 1);
    };
  }

  // A page is written in a place of the page.
  function render(el, nodes) {
    var existing = childrenOf(el);
    if (existing.length) {
      // The page of the server is taken over instead of being put out, so
      // that the visitor does not see the page twice.
      patchChildren(el, nodes);
    } else {
      el.replaceChildren.apply(el, nodes);
    }
    el.__veyMount = { nodes: nodes };
    mounted = { element: el, nodes: nodes };
    flushMounts();
  }

  // A page is written in a place of the page.
  //
  // The page that is already there is changed instead of being put out, so
  // that a list may change and be put in another order.
  function mount(target, node) {
    if (!doc) {
      throw veyError(
        "mount() is only available in the browser. Open the page with `vey dev` " +
          "or build it with `vey web`."
      );
    }
    var el = typeof target === "string" ? doc.querySelector(target) : target;
    if (!el) throw new Error("element not found: " + target);
    render(el, flatten([node]).map(toDom));
    return {
      element: el,
      // The page is written again, with a new tree.
      update: function (next) {
        render(el, flatten([next]).map(toDom));
        return this;
      },
      // The page is taken away, and the work that waits for it is done.
      destroy: function () {
        while (el.firstChild) el.removeChild(el.firstChild);
        el.__veyMount = null;
        if (mounted && mounted.element === el) mounted = null;
        runDestroys();
        return this;
      }
    };
  }

  // The page that is on the screen is written again.
  function renderMounted(node) {
    if (!mounted) return null;
    render(mounted.element, flatten([node]).map(toDom));
    return mounted;
  }

  ctx.mount = mount;
  ctx.renderMounted = renderMounted;
  ctx.queueMount = queueMount;
  ctx.queueDestroy = queueDestroy;
  ctx.flushMounts = flushMounts;
  ctx.runDestroys = runDestroys;

  return {
    mount: mount,
  };
}
