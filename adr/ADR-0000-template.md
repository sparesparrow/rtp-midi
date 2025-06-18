# ADR-0000: Šablona a proces pro architektonická rozhodnutí

## Status

Proposed/Accepted/Deprecated/Superseded by [ADR-XXXX](./ADR-XXXX.md)

## Context

Tento ADR stanovuje proces pro dokumentaci architektonických rozhodnutí v projektu `rtp-midi`. Poskytuje standardizovanou šablonu pro zajištění konzistence, transparentnosti a dohledatelnosti technických rozhodnutí.

## Decision

Budeme používat Markdown soubory pro ADR, uložené v adresáři `/adr` v kořeni repozitáře. Každý ADR bude následovat číslování `ADR-NNNN-nazev-slug.md` a tuto šablonu.

Všechna nová architektonická rozhodnutí, významné technické změny nebo návrhy s dlouhodobým dopadem musí být popsány jako ADR. Proces:

1. **Propose:** Návrh ADR v Markdownu podle této šablony.
2. **Review:** Revize týmem.
3. **Accept:** Po konsenzu změna statusu na "Accepted" a merge do hlavní větve.
4. **Implement:** Implementace rozhodnutí v kódu.
5. **Maintain:** Pokud je rozhodnutí revidováno, původní ADR je označen jako "Deprecated" nebo "Superseded" s odkazem na nový ADR.

## Consequences

* **Pozitivní:**
    * Transparentnost a sdílené porozumění rozhodnutím.
    * Snadnější onboarding nových členů.
    * Jasný historický záznam "proč".
    * Snížení technického dluhu díky promyšlenému návrhu.
* **Negativní:**
    * Počáteční režie s dokumentací.
    * Vyžaduje disciplínu pro údržbu ADR.

## Compliance

Všechny změny v kódu a nové funkce s architektonickým dopadem musí odkazovat na příslušný, přijatý ADR. CI/CD může kontrolovat dodržení ADR pro hlavní změny. 