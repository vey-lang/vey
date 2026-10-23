// The helpers for dates and times.
function veyDate(ctx) {
  var toText = ctx.toText;
  var scope = ctx.scope;

  function pad(value) {
    return value < 10 ? "0" + value : String(value);
  }

  function toDate(value) {
    if (value === undefined || value === null) return new Date();
    if (value instanceof Date) return value;
    if (typeof value === "number") return new Date(value);
    return new Date(String(value));
  }

  // The parts of a date, so that they may be used on their own.
  function parts(value, options) {
    var date = toDate(value);
    if (isNaN(date.getTime())) return null;
    var opts = options || {};
    if (opts.timeZone && scope.Intl && scope.Intl.DateTimeFormat) {
      try {
        var formatter = new scope.Intl.DateTimeFormat("en-US", {
          timeZone: opts.timeZone,
          year: "numeric", month: "2-digit", day: "2-digit",
          hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false
        });
        var pieces = {};
        formatter.formatToParts(date).forEach(function (piece) { pieces[piece.type] = Number(piece.value); });
        return {
          year: pieces.year, month: pieces.month, day: pieces.day,
          hour: pieces.hour, minute: pieces.minute, second: pieces.second,
          weekday: new Date(Date.UTC(pieces.year, pieces.month - 1, pieces.day)).getUTCDay()
        };
      } catch (err) { }
    }
    return {
      year: date.getFullYear(),
      month: date.getMonth() + 1,
      day: date.getDate(),
      hour: date.getHours(),
      minute: date.getMinutes(),
      second: date.getSeconds(),
      weekday: date.getDay()
    };
  }

  var date = {
    now: function () {
      return Date.now();
    },
    iso: function () {
      return new Date().toISOString();
    },
    parts: parts,
    // A date written with a pattern, such as "YYYY-MM-DD".
    format: function (value, pattern, options) {
      var dateParts = parts(value, options);
      if (!dateParts) return "";
      var map = {
        YYYY: String(dateParts.year),
        MM: pad(dateParts.month),
        DD: pad(dateParts.day),
        HH: pad(dateParts.hour),
        mm: pad(dateParts.minute),
        ss: pad(dateParts.second)
      };
      return String(pattern || "YYYY-MM-DD").replace(
        /YYYY|MM|DD|HH|mm|ss/g,
        function (key) {
          return map[key];
        }
      );
    },
    // The number of days between two dates.
    daysBetween: function (from, to) {
      var first = toDate(from);
      var second = toDate(to);
      if (isNaN(first.getTime()) || isNaN(second.getTime())) return 0;
      var firstDay = Date.UTC(first.getFullYear(), first.getMonth(), first.getDate());
      var secondDay = Date.UTC(second.getFullYear(), second.getMonth(), second.getDate());
      return Math.round((secondDay - firstDay) / (24 * 60 * 60 * 1000));
    },
    addDays: function (value, days) {
      var date = toDate(value);
      if (isNaN(date.getTime())) return null;
      var result = new Date(date.getTime());
      result.setDate(result.getDate() + Number(days));
      return result;
    }
  };

  ctx.date = date;

  return {
    date: date,
  };
}
