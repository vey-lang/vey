// The languages and the zones: the way a number, a date and the name of a
// day are written.
function veyIntl(ctx) {
  var scope = ctx.scope;

  // The `Intl` of the host, when it has one.
  var host = scope.Intl || null;

  var DAYS = [
    "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"
  ];
  var MONTHS = [
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December"
  ];

  // The first of January of 2023 was a Sunday, so the days of the week are
  // counted from there.
  var FIRST_SUNDAY = Date.UTC(2023, 0, 1);

  function toDate(value) {
    if (value === undefined || value === null) return new Date();
    if (value instanceof Date) return value;
    if (typeof value === "number") return new Date(value);
    return new Date(String(value));
  }

  function numberFormat(locale) {
    if (!host || !host.NumberFormat) return null;
    try {
      return new host.NumberFormat(locale || undefined);
    } catch (err) {
      return null;
    }
  }

  function dateFormat(locale, options) {
    if (!host || !host.DateTimeFormat) return null;
    try {
      return new host.DateTimeFormat(locale || undefined, options);
    } catch (err) {
      return null;
    }
  }

  function relativeFormat(locale) {
    if (!host || !host.RelativeTimeFormat) return null;
    try {
      return new host.RelativeTimeFormat(locale || undefined, { numeric: "auto" });
    } catch (err) {
      return null;
    }
  }

  // The number as it is written, when the host has no language of its own.
  function plainNumber(value) {
    var text = String(value);
    var parts = text.split(".");
    parts[0] = parts[0].replace(/\B(?=(\d{3})+(?!\d))/g, ",");
    return parts.join(".");
  }

  var intl = {
    // A number with the marks of its language.
    formatNumber: function (value, locale) {
      var number = Number(value);
      if (!isFinite(number)) return String(value);
      var format = numberFormat(locale);
      if (!format) return plainNumber(number);
      return format.format(number);
    },

    // A date with the marks of its language.
    //
    // The options are the marks of the host, such as
    // `{ year: "numeric", month: "long", day: "numeric" }`. The language may
    // be named in them with `locale`.
    formatDate: function (value, options) {
      var date = toDate(value);
      if (isNaN(date.getTime())) return "";
      var opts = options || { year: "numeric", month: "long", day: "numeric" };
      var locale = opts.locale;
      var format = dateFormat(locale, opts);
      if (!format) return date.toDateString();
      return format.format(date);
    },

    // The time of a day with the marks of its language.
    formatTime: function (value, options) {
      var date = toDate(value);
      if (isNaN(date.getTime())) return "";
      var opts = options || { hour: "2-digit", minute: "2-digit" };
      var locale = opts.locale;
      var format = dateFormat(locale, opts);
      if (!format) return date.toTimeString();
      return format.format(date);
    },

    // The name of a day of the week, where the first day is Sunday.
    dayName: function (index, locale) {
      var day = Number(index);
      if (!isFinite(day)) return "";
      var format = dateFormat(locale, { weekday: "long", timeZone: "UTC" });
      if (!format) return DAYS[((day % 7) + 7) % 7];
      return format.format(new Date(FIRST_SUNDAY + ((day % 7) + 7) % 7 * 86400000));
    },

    // The name of a month, where the first month is January.
    monthName: function (index, locale) {
      var month = Number(index);
      if (!isFinite(month)) return "";
      var format = dateFormat(locale, { month: "long", timeZone: "UTC" });
      if (!format) return MONTHS[((month % 12) + 12) % 12];
      return format.format(new Date(Date.UTC(2023, ((month % 12) + 12) % 12, 1)));
    },

    // A time that is told as a distance from now, such as "2 days ago".
    relativeTime: function (value, unit, locale) {
      var amount = Number(value);
      if (!isFinite(amount)) return String(value);
      var kind = String(unit || "second");
      var format = relativeFormat(locale);
      if (!format) {
        var name = kind + (amount === 1 ? "" : "s");
        return amount < 0 ? Math.abs(amount) + " " + name + " ago" : "in " + amount + " " + name;
      }
      return format.format(amount, kind);
    }
  };

  ctx.intl = intl;

  return {
    intl: intl,
  };
}
