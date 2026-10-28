#!/usr/bin/env python3
"""Rebuilds po/battery-health-center.pot from the strings passed to tr(), trf(),
trn(), trnf() and trc() in src/. (xgettext can't read Rust, so this does it.)

Afterwards, update each translation with:
    msgmerge --update --backup=none po/<lang>.po po/battery-health-center.pot
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CALLS = {"tr": 1, "trf": 1, "trn": 2, "trnf": 2, "trc": 1}
ESCAPES = {"n": "\n", "t": "\t", "r": "\r", "\\": "\\", '"': '"', "'": "'", "0": "\0"}


def unescape(raw):
    out, i = [], 0
    while i < len(raw):
        c = raw[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        n = raw[i + 1]
        if n == "\n":  # a line continuation drops the newline and the next line's indent
            i += 2
            while i < len(raw) and raw[i] in " \t\n":
                i += 1
        elif n == "u":
            end = raw.index("}", i)
            out.append(chr(int(raw[i + 3:end], 16)))
            i = end + 1
        else:
            out.append(ESCAPES[n])
            i += 2
    return "".join(out)


def literal_at(text, pos):
    """The string literal that starts at or after `pos`, as (value, end)."""
    m = re.compile(r'\s*"').match(text, pos)
    if not m:
        return None
    start = m.end()
    i = start
    while text[i] != '"':
        i += 2 if text[i] == "\\" else 1
    return unescape(text[start:i]), i + 1


def po_quote(s):
    lines = s.split("\n")
    parts = [l.replace("\\", "\\\\").replace('"', '\\"').replace("\t", "\\t") for l in lines]
    parts = [p + ("\\n" if k < len(parts) - 1 else "") for k, p in enumerate(parts)]
    parts = [p for p in parts if p]
    if len(parts) <= 1:
        return '"' + (parts[0] if parts else "") + '"'
    return '""\n' + "\n".join(f'"{p}"' for p in parts)


def extract():
    found = {}  # (msgid, plural) -> [file:line]
    for path in sorted((ROOT / "src").rglob("*.rs")):
        if path.name in ("i18n.rs", "bhc-helper.rs"):
            continue
        text = path.read_text(encoding="utf-8")
        cut = text.find("#[cfg(test)]")
        text = text if cut < 0 else text[:cut]
        for m in re.finditer(r'(?<![\w.])(tr|trf|trn|trnf|trc)\(', text):
            first = literal_at(text, m.end())
            if not first:
                continue
            msgid, end = first
            context = None
            if m.group(1) == "trc":  # trc("context", "message")
                context = msgid
                first = literal_at(text, text.index(",", end) + 1)
                msgid, end = first
            plural = None
            if CALLS[m.group(1)] == 2:
                second = literal_at(text, text.index(",", end) + 1)
                plural = second[0] if second else None
            line = text.count("\n", 0, m.start()) + 1
            found.setdefault((msgid, plural, context), []).append(f"{path.relative_to(ROOT)}:{line}")
    return found


def main():
    found = extract()
    out = ['''# Battery Health Center translations template.
# Copyright (C) 2026 Alexis Rios-Negron
# This file is distributed under the same license as the Battery Health Center package.
#
#, fuzzy
msgid ""
msgstr ""
"Project-Id-Version: battery-health-center\\n"
"Report-Msgid-Bugs-To: https://github.com/arios4k/battery-health-center/issues\\n"
"MIME-Version: 1.0\\n"
"Content-Type: text/plain; charset=UTF-8\\n"
"Content-Transfer-Encoding: 8bit\\n"
''']
    for (msgid, plural, context), refs in sorted(found.items(), key=lambda kv: kv[1][0]):
        out.append("#: " + " ".join(refs))
        if re.search(r"\{[A-Za-z0-9_]+\}", msgid):
            out.append("#, no-c-format")
        if context is not None:
            out.append("msgctxt " + po_quote(context))
        out.append("msgid " + po_quote(msgid))
        if plural is not None:
            out.append("msgid_plural " + po_quote(plural))
            out.append('msgstr[0] ""')
            out.append('msgstr[1] ""')
        else:
            out.append('msgstr ""')
        out.append("")
    target = ROOT / "po" / "battery-health-center.pot"
    target.parent.mkdir(exist_ok=True)
    target.write_text("\n".join(out), encoding="utf-8")
    print(f"{len(found)} messages -> {target.relative_to(ROOT)}")


if __name__ == "__main__":
    sys.exit(main())
