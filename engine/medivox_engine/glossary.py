from pathlib import Path


def load_glossary(path: Path) -> list[str]:
    if not path.exists():
        return []
    terms: list[str] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        term = line.strip()
        if term and not term.startswith("#"):
            terms.append(term)
    return terms


def build_initial_prompt(terms: list[str], max_chars: int = 800) -> str:
    if not terms:
        return ""
    prompt = "Fachbegriffe: " + ", ".join(terms)
    return prompt[:max_chars]
