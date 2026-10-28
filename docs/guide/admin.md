---
title: Administration
parent: User guide
nav_order: 5
permalink: /guide/admin/
---

# Administration

Admins set up the organisation, add people and keep an eye on the server. Installing a server is described in
[Running a server]({{ '/operations/' | relative_url }}).

## The first start

The first person to open a new server sees *Welcome to notenwart*: name the organisation and create the first admin
account. Do it right after installing – until then, anyone who opens the address can.

## Setting up

*System* (admins only) starts with a checklist; *My music* reminds admins of open steps (*Continue setting up*).

1. **Name and logo of the organisation** (optional) – on the *Organisation* page; the logo appears in the header and
   on the sign-in page. There you also decide whether *people may show a profile picture*. Switched off, nobody's
   picture is shown and none can be uploaded – worth considering when many members are minors.
2. **Check the instruments** – see [Instruments]({{ '/guide/library/#instruments' | relative_url }}); tick it by hand.
3. **Create the ensembles** – choirs, orchestras, classes, courses (*Ensembles*).
4. **Add people** – see below.
5. **Bring in the music** – see [Importing a folder]({{ '/guide/library/#importing-a-folder' | relative_url }}).
6. **Email** – configured on the server; *Send test email* sends one to yourself and ticks the step.
7. **Sign-in with school accounts** (optional) – configured on the server.
8. **Back up regularly** – on the server; tick it by hand.

Below the checklist, *This server* shows the version, address, email settings, storage used, the upload limit, how
sheet music is read and whether single sign-on is on.

## People

*People* (admins only) lists everyone with role, ensembles and status (invited, has a password, last seen).

**Adding one person:** name, email, role → *Add and invite*. When the server sends email, the invitation goes out by
email; you can always copy the link and hand it over yourself. It works once, for seven days; *New sign-in link* makes
a new one.

**Many people at once:** *Import from a table* takes a CSV file – saved from Excel, LibreOffice or Numbers; *Download
a template* shows the columns: name and email, optionally role, ensemble, instrument and part number (German or
English column names; first and last name columns are joined). Every row is checked first; nothing changes unless all
rows are fine. Missing ensembles are created, people who exist already are only added to ensembles. New people get
invitations; *Download invitation links (CSV)* gives you the links to hand out. Up to 2000 rows at a time.

**Selecting several people** in the table: *Add to ensemble …* (with instrument and part number), *Remove from
ensemble …*, *Change role …*, *New invitation links* (as a CSV) or delete.

**Roles:** member, leader, librarian, admin – see the [overview]({{ '/guide/' | relative_url }}). Admins cannot take
their own admin role away or delete themselves.

## Privacy

- **Export:** *Export data* on the people page gives everything stored about a person as a file (also *Download my
  data* on everyone's own account page) – for requests under Art. 15 and 20 GDPR.
- **Deleting** a person removes their account with their memberships, sign-ins, markings and single parts. People
  with printed parts still out cannot be deleted until they are returned. Each deletion is listed under *Deletion
  record* – when, by whom, how many records – without the person's name or email.
- **After a school year:** *Select former members* picks people who are in no ensemble and have not signed in for a
  year (never admins), to check and delete in one go.

notenwart stores names, email addresses, ensembles, instruments and the time of the last sign-in – no tracking, no
telemetry. The organisation running a server is responsible for the data on it and for the rights to the sheet music
shared there.
