# Lokal ScoutID (Keycloak) för utveckling och demo

`docker compose up -d` startar en Keycloak med realmen `scoutid`, som efterliknar nya ScoutID:

- Klienten `scoutnet2-web` (konfidentiell, authorization code + PKCE S256), med redirect till `http://127.0.0.1:8080/auth/callback`.
- Claimet `scoutnet_member_no` i ID-tokenet (som scopet `profile` i ScoutID). Här hämtas det från användarnamnet, som är medlemsnumret.
- Admin-konsol: <http://127.0.0.1:8180/admin> (`admin` / `admin-dev-only`).

Alla lösenord nedan är `scout`. **Endast syntetiska testkonton.** Inga riktiga personer eller lösenord får läggas här.

| Användarnamn (medlemsnr) | Person | Roll i demon |
|---|---|---|
| 9000115 | Klara Forsberg | Medlemsregistrerare, Björkdalens scoutkår |
| 9000033 | Helena Lindgren | Avdelningsledare, Spårhundarna |
| 9000114 | Hugo Lindström | Kårordförande, Björkdalens scoutkår |
| 9000117 | Magnus Jonsson | Distriktsordförande, Älvstadens scoutdistrikt |
| 9000118 | Alma Andersson | Förbundsadministratör |
| 9000078 | Theo Hansson | Ledare, Björnarna |
| 9000080 | Ella Sandberg | Scout (utmanare) |
| 9990001 | (varierar med seedens datum) | Vårdnadshavare med konto, kopplad till en spårare. Fast medlemsnummer. |
| 9000041 | Emma Gustafsson | Scout utan konto i förväg: kontot skapas vid första inloggningen |
| 9999999 | Okänd Testperson | Finns inte i Scoutnet 2: inloggningen nekas med ett begripligt meddelande |

Medlemsnumren förutsätter standardseeden (`cargo run -p sn-server --bin seed`). Realmen läses bara in första gången containern startar. Efter ändringar: `docker compose rm -sf keycloak && docker compose up -d keycloak`.
