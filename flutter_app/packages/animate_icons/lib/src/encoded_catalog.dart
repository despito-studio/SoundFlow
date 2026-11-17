part of '../animate_icons.dart';

/// Keeps only encoded entries. Reading a value returns a fresh decoded tree;
/// only AnimateIcons._icons retains parsed trees used by the renderer.
class _EncodedCatalog extends MapBase<String, dynamic> {
  _EncodedCatalog(String source) {
    var cursor = 0;
    void whitespace() {
      while (cursor < source.length && source.codeUnitAt(cursor) <= 32) {
        cursor++;
      }
    }

    Never invalid() =>
        throw FormatException('Invalid icon catalog', source, cursor);
    void expect(int character) {
      whitespace();
      if (cursor >= source.length || source.codeUnitAt(cursor) != character) {
        invalid();
      }
      cursor++;
    }

    // Locate JSON string boundaries without interpreting braces or escaped
    // quotes inside strings. Decode only the small top-level name strings.
    int stringEnd(int start) {
      var i = start + 1;
      while (i < source.length) {
        final character = source.codeUnitAt(i++);
        if (character == 92) {
          i++;
        } else if (character == 34) {
          return i;
        }
      }
      invalid();
    }

    expect(123); // {
    whitespace();
    if (cursor < source.length && source.codeUnitAt(cursor) != 125) {
      while (true) {
        whitespace();
        if (cursor >= source.length || source.codeUnitAt(cursor) != 34) {
          invalid();
        }
        final end = stringEnd(cursor);
        final name = jsonDecode(source.substring(cursor, end)) as String;
        cursor = end;
        expect(58); // :
        whitespace();
        final start = cursor;
        // Catalog entries are SVG node objects. Scan their balanced containers
        // without materializing any of their maps, lists or numeric values.
        if (cursor >= source.length || source.codeUnitAt(cursor) != 123) {
          invalid();
        }
        var depth = 0;
        do {
          if (cursor >= source.length) invalid();
          final character = source.codeUnitAt(cursor);
          if (character == 34) {
            cursor = stringEnd(cursor);
            continue;
          }
          if (character == 123 || character == 91) depth++;
          if (character == 125 || character == 93) depth--;
          cursor++;
        } while (depth > 0);
        _encoded[name] = source.substring(start, cursor);
        whitespace();
        if (cursor >= source.length) invalid();
        if (source.codeUnitAt(cursor) == 125) break;
        expect(44); // ,
      }
    }
    expect(125); // }
    whitespace();
    if (cursor != source.length) invalid();
  }

  final Map<String, String> _encoded = {};

  @override
  dynamic operator [](Object? key) {
    final source = _encoded[key];
    return source == null ? null : jsonDecode(source);
  }

  @override
  void operator []=(String key, dynamic value) =>
      _encoded[key] = jsonEncode(value);

  @override
  Iterable<String> get keys => _encoded.keys;

  @override
  int get length => _encoded.length;

  @override
  bool containsKey(Object? key) => _encoded.containsKey(key);

  @override
  void clear() => _encoded.clear();

  @override
  dynamic remove(Object? key) {
    final source = _encoded.remove(key);
    return source == null ? null : jsonDecode(source);
  }
}
