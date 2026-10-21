// The helpers for dates and times.
function veyDate(ctx) {
  var toText = ctx.toText;

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
  function parts(value) {
    var date = toDate(value);
    if (isNaN(date.getTime())) return null;
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
    format: function (value, pattern) {
      var dateParts = parts(value);
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
      var day = 24 * 60 * 60 * 1000;
      return Math.round((second.getTime() - first.getTime()) / day);
    },
    addDays: function (value, days) {
      var date = toDate(value);
      if (isNaN(date.getTime())) return null;
      return new Date(date.getTime() + days * 24 * 60 * 60 * 1000);
    }
  };

  ctx.date = date;

  return {
    date: date,
  };
}
