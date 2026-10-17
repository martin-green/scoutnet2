// Läsbara adresser för enheter: /kar/1001, /avdelning/5003, /distrikt/1.
// Numret är unikt per enhetstyp och är för migrerade enheter samma id som i Scoutnet och Skojjt.

/** API-typ → del i webbadressen. */
export const kindSlugs = {
	organisation: 'forbund',
	region: 'region',
	network: 'natverk',
	corps: 'samverkan',
	district: 'distrikt',
	corps_district: 'samverkansdistrikt',
	group: 'kar',
	section: 'sektion',
	troop: 'avdelning',
	patrol: 'patrull'
} as const satisfies Record<string, string>;

export type BodyKind = keyof typeof kindSlugs;

const kindBySlug: Record<string, BodyKind> = Object.fromEntries(
	Object.entries(kindSlugs).map(([kind, slug]) => [slug, kind as BodyKind])
);

export const isKindSlug = (slug: string) => Object.hasOwn(kindBySlug, slug);
export const kindFromSlug = (slug: string): BodyKind | undefined => kindBySlug[slug];

/** Nyckeln som API:t förstår, t.ex. `group-1001`. */
export const bodyKey = (b: { kind: string; number: number }) => `${b.kind}-${b.number}`;

/** Webbadress till en enhet, eventuellt till en undersida (`medlemmar`, `vantelista`, `organisation`). */
export function bodyHref(b: { kind: string; number: number }, sub = ''): string {
	const slug = kindSlugs[b.kind as BodyKind] ?? b.kind;
	return `/${slug}/${b.number}${sub ? `/${sub}` : ''}`;
}

/** Plural för rubriker och filter. */
export const kindPlural: Record<string, string> = {
	region: 'Regioner',
	network: 'Nätverk',
	corps: 'Samverkansorganisationer',
	district: 'Distrikt',
	corps_district: 'Samverkansdistrikt',
	group: 'Kårer',
	section: 'Sektioner',
	troop: 'Avdelningar',
	patrol: 'Patruller'
};

/** Huvudnivåerna uppifrån och ned. Används för att avgöra vilka typer som finns under en enhet. */
export const kindLevels: BodyKind[] = ['organisation', 'district', 'group', 'troop', 'patrol'];

export const kindsBelow = (kind: string): BodyKind[] => {
	const i = kindLevels.indexOf(kind as BodyKind);
	return i < 0 ? [] : kindLevels.slice(i + 1);
};

/** Kort typmärkning i trädet, som bokstäverna i Scoutnets organisationsöversikt. */
export const kindTag: Record<string, string> = {
	organisation: 'F',
	region: 'R',
	network: 'N',
	corps: 'S',
	district: 'D',
	corps_district: 'SD',
	group: 'K',
	section: 'S',
	troop: 'A',
	patrol: 'P'
};
