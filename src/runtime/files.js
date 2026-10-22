// The files of the visitor: a blob, a file, a reader and a form.
function veyFiles(ctx) {
  var scope = ctx.scope;
  var doc = ctx.doc;
  var veyError = ctx.veyError;

  function need(name) {
    if (!doc) {
      throw veyError(name + " is only available in the browser.");
    }
    return true;
  }

  // A piece of binary, with the kind of it.
  function Blob(parts, options) {
    need("Blob");
    var ctor = scope.Blob;
    if (!ctor) throw veyError("Blob is not available in this browser.");
    return new ctor(parts || [], options);
  }

  // A file that a visitor chooses, with its name and its kind.
  function File(parts, name, options) {
    need("File");
    var ctor = scope.File;
    if (!ctor) throw veyError("File is not available in this browser.");
    return new ctor(parts || [], name === undefined ? "" : String(name), options);
  }

  // The reading of a file, as a promise.
  function readAs(reader, blob, method) {
    return new Promise(function (resolve, reject) {
      reader.onload = function () { resolve(reader.result); };
      reader.onerror = function () {
        reject(veyError("the file could not be read."));
      };
      reader[method](blob);
    });
  }

  // The reader of a file.
  //
  // The three ways of reading are given as well, because a promise is
  // easier to wait for than a call that is made later.
  function FileReader() {
    need("FileReader");
    var ctor = scope.FileReader;
    if (!ctor) throw veyError("FileReader is not available in this browser.");
    var reader = new ctor();
    reader.readText = function (blob) {
      return readAs(reader, blob, "readAsText");
    };
    reader.readDataUrl = function (blob) {
      return readAs(reader, blob, "readAsDataURL");
    };
    reader.readArrayBuffer = function (blob) {
      return readAs(reader, blob, "readAsArrayBuffer");
    };
    return reader;
  }

  // A form with the fields of a visitor, and the files they choose.
  function FormData() {
    need("FormData");
    var ctor = scope.FormData;
    if (!ctor) throw veyError("FormData is not available in this browser.");
    return new ctor();
  }

  ctx.Blob = Blob;
  ctx.File = File;
  ctx.FileReader = FileReader;
  ctx.FormData = FormData;

  return {
    Blob: Blob,
    File: File,
    FileReader: FileReader,
    FormData: FormData,
  };
}
