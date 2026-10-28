---
title: Home
nav_order: 1
permalink: /
---

<img src="{{ '/assets/notenwart-mark-on-light.svg' | relative_url }}" alt="" width="96" height="96">

# notenwart documentation

notenwart is a sheet music library that a music school, orchestra, choir or band runs itself. PDFs go in, are split
into parts and given to ensembles; every player sees just their own parts – on a phone, a tablet or the music stand,
also offline – and can practise along with recordings.
{: .fs-5 .fw-300 }

**Auf Deutsch:** [Handbuch auf Deutsch]({{ '/de/' | relative_url }}).

| Page | What it covers |
|---|---|
| [User guide]({{ '/guide/' | relative_url }}) | for everyone using notenwart: players, leaders, the library and admins |
| [Running a server]({{ '/operations/' | relative_url }}) | install, HTTPS, mail, updates, backups, recognition, single sign-on |
| [API]({{ '/api/' | relative_url }}) | the HTTP API and its OpenAPI contract |
| [Contributing]({{ '/contributing/' | relative_url }}) | build, test and the rules of the repository |
| [Design (spec)]({{ '/spec/' | relative_url }}) | roles, data model, milestones, architecture |
| [Changelog]({{ '/changelog/' | relative_url }}) | what changed in each version |

**Status:** in development and in use at a pilot music school (see the [changelog]({{ '/changelog/' | relative_url }})).
Source code: [notenwart/notenwart](https://github.com/notenwart/notenwart) (AGPL-3.0-or-later; the API contract in
`api/` is MIT).

notenwart is software only. **Whoever runs a server is responsible for the rights to the sheet music** uploaded and
shared on it; notenwart offers neutral tools and enforces nothing.
