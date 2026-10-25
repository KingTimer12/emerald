#!/usr/bin/env python3
"""Fills emerald-core/locales/<code>.toml from the translations Discord's web client ships.

Discord bundles its interface text per language as webpack chunks of JSON,
keyed by opaque hashes, with ICU-like syntax trees as values. This script
downloads the web client's chunks (cached, ~100 MB the first time), pairs
every language's bundle with its English one, and looks up each Emerald key
whose English or pt-BR text is also a Discord message. Those keys get Discord's
own wording in every language Discord has.

Keys Discord has no message for are left out of the generated files; the app
falls back to English for them. en-US.toml and pt-BR.toml are written by hand
and never overwritten.

Usage: python3 tools/discord_locales.py [--cache DIR] [--only pt-BR,es-ES]

Only the Python standard library is used.
"""

import argparse
import collections
import concurrent.futures
import json
import re
import sys
import tomllib
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOCALES = ROOT / "emerald-core" / "locales"
HAND_WRITTEN = {"en-US", "pt-BR"}
ORIGIN = "https://discord.com"
USER_AGENT = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36"
)

# `"pt-BR":()=>n.e("286474").then(n.t.bind(n,634449,19))`, or for English
# `n.bind(n,27502)`: one language of a message bundle, as (locale, chunk id,
# module id).
LOADER = re.compile(
    r'"?([A-Za-z]{2,3}(?:-[A-Za-z0-9]{2,4})?)"?:\(\)=>\w+\.e\("(\d+)"\)\.then\(\w+\.(?:t\.)?bind\(\w+,(\d+)(?:,\d+)?\)\)'
)
# A whole loader object: a run of those entries.
LOADER_OBJECT = re.compile(r"\{(?:" + LOADER.pattern + r",?){3,}\}")
# Inside a chunk: `634449(e){e.exports=JSON.parse('{...}')}`, or for English
# `27502(e,s,r){r.r(s),…;let t=JSON.parse('{...}')`.
MODULE = re.compile(r"(\d+)\(\w+(?:,\w+)*\)\{(?:(?!JSON\.parse|\d+\(\w).){0,200}JSON\.parse\('((?:[^'\\]|\\.)*)'\)", re.S)


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read().decode("utf-8", "replace")


def chunk_files(cache):
    """Every JS chunk of the current web client, as {chunk id: file name}."""
    page = fetch(f"{ORIGIN}/app")
    scripts = sorted(set(re.findall(r'src="(/assets/[^"]+\.js)"', page)))
    runtime = next((s for s in scripts if "/web." in s), None)
    if runtime is None:
        sys.exit("could not find the web client's runtime script")
    source = cached(cache, Path(runtime).name, runtime)
    start = source.index(".u=e=>")
    end = source.index('+".js"', start)
    mapping = source[start:end]
    files = {cid: f"{cid}.{h}.js" for cid, h in re.findall(r'"(\d+)"===e\?""\+e\+"\.([0-9a-f]+)\.js"', mapping)}
    files.update({cid: f"{h}.js" for cid, h in re.findall(r'(\d+):"([0-9a-f]{16})"', mapping)})
    for script in scripts:
        files[Path(script).name] = Path(script).name
    return files


def cached(cache, name, path=None):
    file = cache / name
    if not file.exists():
        file.write_text(fetch(ORIGIN + (path or f"/assets/{name}")), "utf-8")
    return file.read_text("utf-8")


def download_all(cache, files):
    missing = [name for name in files.values() if not (cache / name).exists()]
    if missing:
        print(f"downloading {len(missing)} of {len(files)} chunks…", file=sys.stderr)
    with concurrent.futures.ThreadPoolExecutor(8) as pool:
        for done, _ in enumerate(pool.map(lambda name: try_cached(cache, name), missing), 1):
            if done % 500 == 0:
                print(f"  {done}/{len(missing)}", file=sys.stderr)


def try_cached(cache, name):
    try:
        cached(cache, name)
    except Exception as error:  # A missing chunk only loses its messages.
        print(f"  skipped {name}: {error}", file=sys.stderr)


JS_ESCAPE = re.compile(r"\\(x[0-9a-fA-F]{2}|u[0-9a-fA-F]{4}|.)", re.S)
SIMPLE_ESCAPES = {"n": "\n", "t": "\t", "r": "\r", "b": "\b", "f": "\f", "v": "\v", "0": "\0"}


def unescape_js(literal):
    """The text of a single-quoted JS string literal's body."""

    def one(match):
        escape = match.group(1)
        if escape[0] in "xu" and len(escape) > 1:
            return chr(int(escape[1:], 16))
        return SIMPLE_ESCAPES.get(escape, escape)

    return JS_ESCAPE.sub(one, literal)


def modules(cache, files):
    """Every message bundle, as {module id: {hash: syntax tree}}."""
    found = {}
    for name in set(files.values()):
        file = cache / name
        if not file.exists():
            continue
        for module, body in MODULE.findall(file.read_text("utf-8")):
            try:
                found[module] = json.loads(unescape_js(body))
            except json.JSONDecodeError:
                pass
    return found


def loaders(cache, files):
    """Each bundle's languages, as a list of {locale: module id}."""
    out = []
    for name in set(files.values()):
        file = cache / name
        if not file.exists():
            continue
        for match in LOADER_OBJECT.finditer(file.read_text("utf-8")):
            languages = {locale: module for locale, _, module in LOADER.findall(match.group(0))}
            if "en-US" in languages:
                out.append(languages)
    return out


def render(tree):
    """A message as Emerald's template syntax, with Discord's argument names,
    or None when it holds more than text, arguments and markup (plurals,
    selects). Markup such as `[8,"$b",[...]]` (bold) keeps only its content:
    Emerald styles names itself."""
    if isinstance(tree, str):
        return tree
    if not isinstance(tree, list):
        return None
    parts = []
    for node in tree:
        if isinstance(node, str):
            parts.append(node)
        elif isinstance(node, list) and len(node) == 2 and node[0] == 1 and isinstance(node[1], str):
            parts.append("{" + node[1] + "}")
        elif isinstance(node, list) and len(node) == 3 and node[0] == 8 and isinstance(node[2], list):
            inner = render(node[2])
            if inner is None:
                return None
            parts.append(inner)
        else:
            return None
    return "".join(parts)


# What Discord's language picker shows for each language: its name in that
# language and a flag.
LANGUAGES = {
    "bg": ("български", "🇧🇬"), "cs": ("Čeština", "🇨🇿"), "da": ("Dansk", "🇩🇰"),
    "de": ("Deutsch", "🇩🇪"), "el": ("Ελληνικά", "🇬🇷"), "en-GB": ("English (UK)", "🇬🇧"),
    "es-419": ("Español (América Latina)", "🌎"), "es-ES": ("Español", "🇪🇸"),
    "fi": ("Suomi", "🇫🇮"), "fr": ("Français", "🇫🇷"), "hi": ("हिंदी", "🇮🇳"),
    "hr": ("Hrvatski", "🇭🇷"), "hu": ("Magyar", "🇭🇺"), "id": ("Bahasa Indonesia", "🇮🇩"),
    "it": ("Italiano", "🇮🇹"), "ja": ("日本語", "🇯🇵"), "ko": ("한국어", "🇰🇷"),
    "lt": ("Lietuviškai", "🇱🇹"), "nl": ("Nederlands", "🇳🇱"), "no": ("Norsk", "🇳🇴"),
    "pl": ("Polski", "🇵🇱"), "ro": ("Română", "🇷🇴"), "ru": ("Русский", "🇷🇺"),
    "sv-SE": ("Svenska", "🇸🇪"), "th": ("ไทย", "🇹🇭"), "tr": ("Türkçe", "🇹🇷"),
    "uk": ("Українська", "🇺🇦"), "vi": ("Tiếng Việt", "🇻🇳"), "zh-CN": ("中文", "🇨🇳"),
    "zh-TW": ("繁體中文", "🇹🇼"),
}

ARGUMENT = re.compile(r"\{(\w+)\}")


def shape(text):
    """Text with its arguments blanked, for comparing across naming. Case,
    "…" against "..." and a final period are ignored: Discord uppercases
    headers in CSS, and Emerald writes a real ellipsis."""
    text = ARGUMENT.sub("{}", text).replace("…", "...").strip().lower()
    return text[:-1] if text.endswith(".") and not text.endswith("...") else text


def styled(translation, english):
    """Discord's translation written the way Emerald writes the English: in
    capitals for headers, with "…" for "..."."""
    if english.isupper():
        translation = translation.upper()
    if "…" in english:
        translation = translation.replace("...", "…")
    return translation


def rename(text, theirs, ours):
    """Discord's argument names in `text` replaced by ours, by position."""
    names = dict(zip(theirs, ours))
    return ARGUMENT.sub(lambda m: "{" + names.get(m.group(1), m.group(1)) + "}", text)


def matches(english, portuguese, by_english, by_portuguese):
    """The Discord messages that are this Emerald text. Both languages must
    agree, so a short word does not pick up a message from another context
    ("Save" the button, not a goalkeeper's save). A long text is distinctive
    enough to match on one language."""
    in_english = by_english.get(shape(english), [])
    in_portuguese = by_portuguese.get(shape(portuguese), [])
    both = [m for m in in_english if any(m is p for p in in_portuguese)]
    if both:
        return both
    if len(english.split()) >= 4:
        return in_portuguese or in_english
    return []


def flatten(table, prefix=""):
    for key, value in table.items():
        full = f"{prefix}.{key}" if prefix else key
        if isinstance(value, dict):
            yield from flatten(value, full)
        else:
            yield full, value


def write_locale(code, texts, english_order):
    sections = {}
    for key in english_order:
        if key in texts:
            section, rest = key.split(".", 1)
            sections.setdefault(section, []).append((rest, texts[key]))
    lines = [
        f"# {code}, generated by tools/discord_locales.py from the wording of",
        "# Discord's web client. Keys Discord has no message for fall back to",
        "# English. Rerun the tool instead of editing by hand.",
    ]
    for section, entries in sections.items():
        lines.append(f"\n[{section}]")
        lines.extend(f"{rest} = {json.dumps(text, ensure_ascii=False)}" for rest, text in entries)
    (LOCALES / f"{code}.toml").write_text("\n".join(lines) + "\n", "utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--cache", type=Path, default=ROOT / "target" / "discord-intl")
    parser.add_argument("--only", help="comma-separated locale codes to write")
    args = parser.parse_args()
    args.cache.mkdir(parents=True, exist_ok=True)

    files = chunk_files(args.cache)
    download_all(args.cache, files)
    bundles = modules(args.cache, files)

    # Every Discord message as {locale: text}, keyed by (bundle, hash).
    messages = {}
    for languages in loaders(args.cache, files):
        english = bundles.get(languages["en-US"], {})
        for locale, module in languages.items():
            for key, tree in bundles.get(module, {}).items():
                if key in english and (text := render(tree)) is not None:
                    messages.setdefault((languages["en-US"], key), {})[locale] = text
    by_english, by_portuguese = {}, {}
    for message in messages.values():
        if "en-US" in message:
            by_english.setdefault(shape(message["en-US"]), []).append(message)
        if "pt-BR" in message:
            by_portuguese.setdefault(shape(message["pt-BR"]), []).append(message)
    print(f"{len(messages)} Discord messages in {len(bundles)} bundles", file=sys.stderr)

    english = dict(flatten(tomllib.loads((LOCALES / "en-US.toml").read_text("utf-8"))))
    portuguese = dict(flatten(tomllib.loads((LOCALES / "pt-BR.toml").read_text("utf-8"))))
    out = {}
    matched = 0
    for key, text in english.items():
        if key.startswith("language."):
            continue
        candidates = matches(text, portuguese.get(key, ""), by_english, by_portuguese)
        ours = ARGUMENT.findall(text)
        candidates = [m for m in candidates if len(ARGUMENT.findall(m["en-US"])) == len(ours)]
        if not candidates:
            continue
        matched += 1
        locales = {locale for m in candidates for locale in m}
        for locale in locales:
            # The same text can be translated differently in different
            # places; the most common choice wins.
            votes = collections.Counter(
                styled(rename(m[locale], ARGUMENT.findall(m["en-US"]), ours), text)
                for m in candidates
                if locale in m
            )
            out.setdefault(locale, {})[key] = votes.most_common(1)[0][0]
    print(f"{matched} of {len(english) - 2} Emerald keys found in Discord", file=sys.stderr)

    # Each language's own name, from Discord's language picker if it has one.
    only = set(args.only.split(",")) if args.only else None
    for locale, texts in sorted(out.items()):
        if locale in HAND_WRITTEN or (only and locale not in only):
            continue
        texts["language.name"], texts["language.flag"] = LANGUAGES.get(locale, (locale, ""))
        write_locale(locale, texts, english)
        print(f"  {locale}: {len(texts) - 2} keys", file=sys.stderr)


if __name__ == "__main__":
    main()
