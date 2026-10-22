# porkDyn glossary

## Language

**Address reconciliation**:
A DNS update that compares an existing A or AAAA record with a desired IP address and leaves it unchanged, updates it, or creates it when absent.
_Avoid_: Upsert, which can imply an atomic create-or-update guarantee.

**Unchanged record**:
An existing A or AAAA record whose address text exactly matches the desired address text. Equivalent IP addresses written differently do not satisfy this definition.

**Reconciliation outcome**:
The result of a successful address reconciliation: the DNS record was unchanged, updated, or created. A failed reconciliation has an error instead of a successful outcome.
