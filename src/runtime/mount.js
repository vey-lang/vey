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
    // written something else in it. The value of the program is known, so
    // that the text of the visitor is not wiped.
    if (doc && doc.activeElement !== old) {
      if (neu.__veyHasValue && old.value !== neu.__veyValue) old.value = neu.__veyValue;
      if (neu.__veyHasChecked && old.checked !== neu.__veyChecked) old.checked = neu.__veyChecked;
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
    if (parent) {
      // The visitor who is writing in the node keeps his place, so the
      // focus follows the new node.
      var hadFocus = doc && doc.activeElement === old;
      var start = old.selectionStart;
      var end = old.selectionEnd;
      parent.replaceChild(neu, old);
      if (hadFocus && neu && typeof neu.focus === "function") {
        neu.focus();
        if (typeof start === "number" && typeof neu.setSelectionRange === "function") {
          try {
            neu.setSelectionRange(start, end);
          } catch (err) {
            // A node that has no place to write in does not take one.
          }
        }
      }
    }
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

    // The children that are not wanted any more are taken away. A node
    // that says goodbye is taken away when it is done, so that a transition
    // may be seen.
    for (var k = 0; k < old.length; k++) {
      if (used[k] || old[k].parentNode !== parent) continue;
      var leaving = old[k];
      // A node that is taken away lets go of what it holds.
      if (typeof leaving.__veyUnbind === "function") {
        leaving.__veyUnbind();
      }
      var goodbye = leaving.__veyLeave;
      if (typeof goodbye === "function") {
        var waiting = null;
        try {
          waiting = goodbye(leaving);
        } catch (err) {
          log.error(err);
        }
        if (waiting && typeof waiting.then === "function") {
          waiting.then(
            function () {
              if (leaving.parentNode === parent) parent.removeChild(leaving);
            },
            function () {
              if (leaving.parentNode === parent) parent.removeChild(leaving);
            }
          );
          continue;
        }
      }
      parent.removeChild(leaving);
    }

    // The children are put in the order of the new ones.
    var cursor = parent.firstChild;
    for (var m = 0; m < pairs.length; m++) {
      var target = pairs[m].old;
      if (target === null) {
        target = pairs[m].neu;
        parent.insertBefore(target, cursor);
        // A node that says hello is called after it is in the page.
        if (target && typeof target.__veyEnter === "function") {
          try {
            target.__veyEnter(target);
          } catch (err) {
            log.error(err);
          }
        }
      } else {
        target = patchNode(target, pairs[m].neu);
        if (target !== cursor && target.parentNode === parent) {
          // The node before which the new one is put must be a node of the
          // parent, or the browser refuses it.
          var before = cursor && cursor.parentNode === parent ? cursor : null;
          parent.insertBefore(target, before);
        }
      }
      cursor = target.nextSibling;
    }
  }

  // A node that says hello is called after it is in the page.
  function sayHello(node) {
    if (!node || typeof node.__veyEnter !== "function") return;
    try {
      node.__veyEnter(node);
    } catch (err) {
      log.error(err);
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
      // The nodes of the first writing of the page say hello as well.
      for (var i = 0; i < nodes.length; i++) {
        sayHello(nodes[i]);
      }
    }
    el.__veyMount = { nodes: nodes };
    mounted = { element: el, nodes: nodes };
    flushMounts();
  }

  // A page is written in a place of the page.
  //
  // The page that is already there is changed instead of being put out, so
  // that a list may change and be put in another order.
  //
  // The page may be given as a function. Then the values that the page
  // reads are remembered, and the page is written again when one of them
  // changes.
  function mount(target, node) {
    if (!doc) {
      throw veyError(
        "mount() is only available in the browser. Open the page with `vey dev` " +
          "or build it with `vey web`."
      );
    }
    var el = typeof target === "string" ? doc.querySelector(target) : target;
    if (!el) throw new Error("element not found: " + target);
    // The page is a function, so it is written again when a value it reads
    // changes.
    var isFunction = typeof node === "function";
    var drawing = false;
    var again = false;

    function draw() {
      // A value that is changed while the page is written does not write it
      // again at once, but after the page is done.
      if (drawing) {
        again = true;
        return;
      }
      drawing = true;
      try {
        var tree = isFunction ? node() : node;
        render(el, flatten([tree]).map(toDom));
      } finally {
        drawing = false;
      }
      if (again) {
        again = false;
        draw();
      }
    }

    var stop = null;
    if (isFunction) {
      // The effect listens to the values the page reads.
      stop = ctx.effect(function () {
        draw();
      });
    } else {
      draw();
    }

    return {
      element: el,
      // The page is written again, with a new tree.
      update: function (next) {
        node = next;
        isFunction = typeof next === "function";
        draw();
        return this;
      },
      // The page is taken away, and the work that waits for it is done.
      destroy: function () {
        if (stop) {
          stop();
          stop = null;
        }
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
