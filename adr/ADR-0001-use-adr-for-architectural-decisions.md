# ADR-0001: Use ADRs for Architectural Decisions

## Status

Accepted

## Context

Jak projekt rtp-midi roste, je nutné dokumentovat architektonická rozhodnutí konzistentně a dohledatelně. ADR poskytují lehký, verzovaný způsob, jak zachytit kontext, rozhodnutí a důsledky klíčových voleb.

## Decision

- Projekt bude používat [Architecture Decision Record (ADR)](https://adr.github.io/) pro dokumentaci všech významných rozhodnutí.
- Všechny ADR budou uloženy v adresáři `/adr` v kořeni repozitáře.
- Každý ADR bude následovat šablonu popsanou v [ADR-0000-template.md](./ADR-0000-template.md):
  - **Context:** Pozadí, motivace, souvislosti.
  - **Decision:** Zvolená varianta a její zdůvodnění.
  - **Consequences:** Důsledky, kompromisy, follow-up.
- Workflow schvalování:
  - Nové ADR jsou navrhovány přes pull request.
  - PR musí být schválen tech leadem nebo určeným recenzentem.
  - Po schválení je ADR mergován a považován za oficiální záznam.

## Consequences

- Všechna hlavní rozhodnutí budou transparentní, dohledatelná a recenzovaná.
- Noví přispěvatelé rychle pochopí důvody klíčových voleb.
- Proces ADR zabrání ztrátě znalostí a opakování chyb. 