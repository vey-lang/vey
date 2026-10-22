// The values of a form, and what is wrong with them.
//
// A form is a value that follows the state of a page: the fields, the
// mistakes of the fields, whether the visitor wrote anything and whether the
// form may be sent.
function veyForm(ctx) {
  var log = ctx.log;

  // A copy of the fields, so that a change makes a new value.
  function copy(value) {
    var out = {};
    if (!value || typeof value !== "object") return out;
    for (var name in value) {
      if (Object.prototype.hasOwnProperty.call(value, name)) out[name] = value[name];
    }
    return out;
  }

  // A form with the values it begins with.
  //
  // The work of `check` is given the values and says what is wrong with
  // them: an object of the field and the mistake. A field that is not in it
  // is fine.
  function form(initial, options) {
    var opts = options || {};
    var check = typeof opts.check === "function" ? opts.check : null;
    var started = copy(initial);

    var values = ctx.signal(started, options);
    var errors = ctx.signal({});

    function runCheck() {
      if (!check) return errors.peek();
      var found = null;
      try {
        found = check(values.peek());
      } catch (err) {
        log.error(err);
      }
      errors.set(found && typeof found === "object" ? copy(found) : {});
      return errors.peek();
    }

    runCheck();

    // Whether every field is fine.
    var valid = ctx.computed(function () {
      var found = errors.value || {};
      for (var name in found) {
        if (Object.prototype.hasOwnProperty.call(found, name) && found[name]) return false;
      }
      return true;
    });

    // Whether the visitor changed anything.
    var dirty = ctx.computed(function () {
      return !ctx.deepEquals(values.value, started);
    });

    return {
      // The values of the fields, as a value that tells when it changes.
      values: values,
      // What is wrong with the fields.
      errors: errors,
      // Whether every field is fine.
      valid: valid,
      // Whether the visitor wrote in the form.
      dirty: dirty,
      // One field is written.
      set: function (name, value) {
        var next = copy(values.peek());
        next[String(name)] = value;
        values.set(next);
        runCheck();
        return next;
      },
      // The whole of the form is written.
      replace: function (next) {
        values.set(copy(next));
        runCheck();
        return values.peek();
      },
      // The form is put back the way it was.
      reset: function () {
        values.set(copy(started));
        runCheck();
        return values.peek();
      },
      // The fields are checked again, and the mistakes are given back.
      check: function () {
        runCheck();
        return errors.peek();
      },
      // The form is sent, when it may be sent. The answer is the one of the
      // function that sends it, or nothing when the form is not fine.
      submit: function (send) {
        if (!valid.peek()) return Promise.resolve(null);
        try {
          return Promise.resolve(typeof send === "function" ? send(values.peek()) : values.peek());
        } catch (err) {
          return Promise.reject(err);
        }
      }
    };
  }

  ctx.form = form;

  return {
    form: form,
  };
}
