// The values of the enums of the language.
function veyEnum(ctx) {
  var str = ctx.str;
  function enumValue(enumName, variant, displayName) {
    var value = {};
    var shown = displayName === undefined ? enumName : displayName;
    // The bookkeeping fields stay hidden so that they do not show up in
    // `keys`, `values` or `entries`.
    Object.defineProperty(value, "__veyEnum", { value: enumName, enumerable: false });
    Object.defineProperty(value, "__veyVariant", { value: variant, enumerable: false });
    Object.defineProperty(value, "toString", {
      value: function () {
        return shown + "." + variant;
      },
      enumerable: false
    });
    return Object.freeze(value);
  }

  function enumCtor(enumName, variant, fields) {
    return function () {
      var args = Array.prototype.slice.call(arguments);
      var value = {};
      Object.defineProperty(value, "__veyEnum", { value: enumName, enumerable: false });
      Object.defineProperty(value, "__veyVariant", { value: variant, enumerable: false });
      for (var i = 0; i < fields.length; i++) value[fields[i]] = args[i];
      Object.defineProperty(value, "toString", {
        value: function () {
          var parts = [];
          for (var j = 0; j < fields.length; j++) {
            parts.push(fields[j] + "=" + str(value[fields[j]]));
          }
          return variant + "(" + parts.join(", ") + ")";
        },
        enumerable: false
      });
      return Object.freeze(value);
    };
  }

  ctx.enumValue = enumValue;
  ctx.enumCtor = enumCtor;

  return {
    enumValue: enumValue,
    enumCtor: enumCtor,
  };
}
