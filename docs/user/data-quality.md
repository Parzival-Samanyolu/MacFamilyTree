# Data quality

## Plausibility check
Rules cover death before birth, children born before (or too long after) their parents or after a parent's death, parents who are too young or too old, marriages before birth/after death/at a very young age, events after death or before birth, people who are their own ancestors, duplicate or conflicting events, overlapping residences, siblings born impossibly close together, dates that cannot be read, unconnected people, empty families and broken references.
Each finding has a severity, **Open** (jumps to the person), **Ignore** (remembered in the project) and, when safe, **Fix** (for example *remove duplicate event*; undoable).
Comparisons use the full range of imprecise dates, so `about 1850` or a bare year never triggers a false alarm.

## Duplicates
KinTree scores likely duplicates from names (including spelling variants and Turkish letters ı/İ ş ğ ç ö ü), birth and death years and places, shared parent and spouse names. Choose *Keep this one* to merge the other record into it: names, events, notes, citations, media, family roles and associations all move across and exact copies are collapsed. The merge is a single undoable step. *Not a duplicate* hides a pair permanently.

## Find & replace
**Find & replace** changes text in many records at once: given names, surnames, nicknames, place names, event descriptions/values/causes, note text, source titles/authors and citation pages. Choose the field, type what to find and what to replace it with, then **Preview** to see every before/after pair. **Replace** applies it as one change that **Undo** reverses completely. *Whole field only* replaces entire values that equal the search text (for example turning every place named “Angora” into “Ankara” without touching “Angora Road”).
