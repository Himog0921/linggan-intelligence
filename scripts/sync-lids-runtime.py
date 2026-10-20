#!/usr/bin/env python3
"""Generate the approved runtime values in the one existing Token file; --check is read-only."""
import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "docs/design/lids"
CSS = ROOT / "apps/web/src/styles/lids_tokens.css"
START, END = "  /* LIDS-UI-001 generated: start */", "  /* LIDS-UI-001 generated: end */"


def load(name):
    return json.loads((SOURCE / f"{name}.json").read_text())


def generated():
    type_spec, color = load("typography"), load("color")["values"]
    material_source = load("control-material")
    material = material_source["parameters"]
    shell, ledger = load("shell-layout")["layout"], load("ledger-layout")
    values = {}
    for role, family in type_spec["families"].items():
        values[f"font-{role}"] = family
    for role, spec in type_spec["roles"].items():
        key = re.sub(r"([A-Z])", lambda m: "-" + m[1].lower(), role)
        for prop, value in spec.items():
            suffix = {"size": "size", "weight": "weight", "lineHeight": "leading"}[prop]
            values[f"type-{key}-{suffix}"] = f"{value}px" if prop == "size" else str(value)
    values.update({
        "focus-warm": load("focus")["parameters"]["warmFill"],
        "focus-inset": f'{load("focus")["parameters"]["keyboardInsetPx"]}px',
        "shell-side": f'{shell["sideWidth"]}px',
        "shell-collapsed": f'{shell["collapsedWidth"]}px',
        "shell-icon": f'{shell["iconSize"]}px',
        "ledger-row": f'{ledger["decision"]["rowHeight"]}px',
        "control-tail": f'{material["buttonTail"]}px',
        "control-pitch": f'{material["buttonPitch"]}px',
        "secondary-tail": f'{material["secondaryTail"]}px',
        "secondary-pitch": f'{material["secondaryPitch"]}px',
        "domain-tail": f'{material["domainTail"]}px',
        "shell-pitch": f'{material["shellPitch"]}px',
        "context-major": f'{material["contextMajor"]}px',
        "context-fine": f'{material["contextFine"]}px',
        "action-shadow-idle": f'{material["buttonShadow"]}px',
        "action-shadow-hover": f'{material["buttonShadow"] + material["hoverLift"]}px',
        "action-hover-shift": f'-{material["hoverLift"]}px',
        "action-press-shift": f'{material["pressDepth"]}px',
        "action-line-height": str(type_spec["roles"]["control"]["lineHeight"]),
        "action-lattice-cell": f'{material["buttonPitch"]}px',
        "action-lattice-light": f'repeating-conic-gradient(rgb(255 255 255 / {material["buttonOpacity"]}) 0% 25%,transparent 0% 50%)',
        "action-lattice-ink": f'repeating-conic-gradient(color-mix(in srgb,var(--lgi-ink) {material_source["geometry"]["secondaryMosaicInkOpacity"] * 100:g}%,transparent) 0% 25%,transparent 0% 50%)',
    })
    for key, width in ledger["widths"].items():
        values[f"ledger-col-{key}"] = f"{width}px"
    # These public aliases preserve existing consumers while moving them to one approved role.
    aliases = {
        "font-sans": "font-ui", "font-mono": "font-data", "font-evidence": "font-chinese",
        "text-display": "type-display-size", "text-hero": "type-metric-large-size",
        "text-page-title": "type-page-size", "text-section": "type-section-size",
        "text-longform": "type-reading-size", "text-body": "type-body-size",
        "text-data": "type-data-size", "text-label": "type-label-size",
        "text-calibration": "type-aux-size", "lh-display": "type-display-leading",
        "lh-title": "type-page-leading", "lh-heading": "type-section-leading",
        "lh-data": "type-data-leading", "lh-body": "type-body-leading", "lh-longform": "type-reading-leading",
        "action-font-size": "type-control-size",
        "action-fill": "signal", "action-ink": "ink", "action-ink-shadow": "ink",
        "action-paper": "canvas", "action-secondary": "muted", "action-disabled": "ghost",
        "action-focus": "focus", "action-danger": "danger", "action-mark": "signal-mark",
    }
    values.update({k: f"var(--lgi-{v})" for k, v in aliases.items()})
    values.update({
        "ink": color["ink"], "body": color["body"], "muted": color["secondary"],
        "ghost": color["disabled"], "meta": color["meta"], "signal": color["signal"],
        "signal-hover": color["signal"], "signal-ink": color["signal"], "signal-mark": color["mark"],
        "success": color["success"], "warning": color["warning"], "danger": color["danger"],
        "info": color["info"], "unknown": color["meta"], "border-solid": color["ink"],
        "weight-bold": str(type_spec["weights"]["strong"]),
        "shadow-brutal": f'{material["menuShadow"]}px {material["menuShadow"]}px 0 var(--lgi-ink)',
        "shadow-brutal-lg": f'{material["buttonShadow"] + material["hoverLift"]}px {material["buttonShadow"] + material["hoverLift"]}px 0 var(--lgi-ink)',
        "hairline": "color-mix(in srgb,var(--lgi-ink) 9%,transparent)",
        "border": "color-mix(in srgb,var(--lgi-ink) 15%,transparent)",
        "border-strong": "color-mix(in srgb,var(--lgi-ink) 22%,transparent)",
        "mesh-fine": "color-mix(in srgb,var(--lgi-ink) 5.5%,transparent)",
        "mesh-major": "color-mix(in srgb,var(--lgi-ink) 10%,transparent)",
        "signal-soft": "color-mix(in srgb,var(--lgi-signal) 10%,transparent)",
        "signal-faint": "color-mix(in srgb,var(--lgi-signal) 5%,transparent)",
    })
    return values


def output():
    css = CSS.read_text()
    css = re.sub(re.escape(START) + r"[\s\S]*?" + re.escape(END) + r"\n?", "", css)
    values = generated()
    # Replace existing declarations rather than leaving two definitions of the same token.
    for name in values:
        css = re.sub(r"^  --lgi-" + re.escape(name) + r":.*\n", "", css, flags=re.M)
    lines = "\n".join(f"  --lgi-{key}: {value};" for key, value in values.items())
    css = css.rstrip()
    css = css[:-1].rstrip() + f"\n\n{START}\n{lines}\n{END}\n}}\n"
    return css


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    result = output()
    if args.check:
        if result != CSS.read_text():
            raise SystemExit("LIDS runtime drift: run python3 scripts/sync-lids-runtime.py")
        print("LIDS runtime source mirror: PASS")
    else:
        CSS.write_text(result)
        doc = SOURCE / "tokens.md"
        text = doc.read_text()
        text = re.sub(r"```css\n[\s\S]*?\n```", lambda _: "```css\n" + result.rstrip() + "\n```", text, count=1)
        doc.write_text(text)
