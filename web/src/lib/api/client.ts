import type { components } from './schema';

export type Schemas = components['schemas'];
export type Me = Schemas['MeResponse'];
export type Context = Schemas['ContextDto'];
export type Body = Schemas['BodyDto'];
export type Crumb = Schemas['CrumbDto'];
export type BodyChild = Schemas['BodyChildDto'];
export type OrgNode = Schemas['OrgNodeDto'];
export type OrgNodePage = Schemas['OrgNodePage'];
export type MemberRow = Schemas['MemberRow'];
export type MemberPage = Schemas['MemberPage'];
export type PersonDetail = Schemas['PersonDetail'];
export type Persona = Schemas['Persona'];
export type Permission = Schemas['Permission'];
export type Problem = Schemas['Problem'];

export class ApiError extends Error {
	constructor(
		public status: number,
		public problem: Problem | null
	) {
		super(problem?.title ?? `HTTP ${status}`);
	}
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
	const headers: Record<string, string> = { accept: 'application/json' };
	if (body !== undefined) headers['content-type'] = 'application/json';
	// Muterande anrop kräver CSRF-headern (se servern).
	if (method !== 'GET') headers['x-sn-csrf'] = '1';
	const res = await fetch(path, {
		method,
		headers,
		credentials: 'same-origin',
		body: body === undefined ? undefined : JSON.stringify(body)
	});
	if (!res.ok) {
		const problem = res.headers.get('content-type')?.includes('json') ? await res.json() : null;
		throw new ApiError(res.status, problem);
	}
	if (res.status === 204) return undefined as T;
	return (await res.json()) as T;
}

export const api = {
	me: () => request<Me>('GET', '/v1/me'),
	/** `id` är enhetens UUID eller nyckeln `typ-nummer`, t.ex. `group-1001`. */
	body: (id: string) => request<Body>('GET', `/v1/bodies/${id}`),
	children: (id: string) => request<BodyChild[]>('GET', `/v1/bodies/${id}/children`),
	descendants: (id: string, query: URLSearchParams) =>
		request<OrgNodePage>('GET', `/v1/bodies/${id}/descendants?${query}`),
	members: (id: string, query: URLSearchParams) =>
		request<MemberPage>('GET', `/v1/bodies/${id}/members?${query}`),
	person: (id: string) => request<PersonDetail>('GET', `/v1/people/${id}`),
	revealNationalId: (id: string) =>
		request<{ national_id: string }>('POST', `/v1/people/${id}/reveal-national-id`),
	personas: () => request<Persona[]>('GET', '/auth/dev/personas'),
	devLogin: (account_id: string) => request<void>('POST', '/auth/dev/login', { account_id }),
	logout: () => request<void>('POST', '/auth/logout')
};

export const troopTypes: { key: string; label: string }[] = [
	{ key: 'familj', label: 'Familj' },
	{ key: 'baver', label: 'Bäver' },
	{ key: 'sparare', label: 'Spårare' },
	{ key: 'upptackare', label: 'Upptäckare' },
	{ key: 'aventyrare', label: 'Äventyrare' },
	{ key: 'utmanare', label: 'Utmanare' },
	{ key: 'rover', label: 'Rover' },
	{ key: 'annan', label: 'Annan' }
];

export const troopTypeLabel = (key?: string | null) =>
	troopTypes.find((t) => t.key === key)?.label ?? key ?? '';

export const permissionLabels: Record<string, string> = {
	body_read: 'Se enheten',
	members_read: 'Se medlemmar',
	members_contact_read: 'Se kontaktuppgifter',
	personal_read: 'Se personuppgifter',
	national_id_read: 'Visa personnummer (loggas)',
	members_manage: 'Hantera medlemskap',
	attendance_read: 'Se närvaro',
	attendance_write: 'Registrera närvaro',
	reports_read: 'Rapporter',
	audit_read: 'Revisionslogg',
	economy_read: 'Se avgifter och betalningar'
};

type Tone = '' | 'green' | 'red' | 'accent' | 'blue';
export type Labelled = { label: string; tone: Tone };

export const membershipStatusLabels: Record<string, string> = {
	active: 'Aktiv',
	waiting: 'Väntelista',
	pending: 'Väntar på godkännande'
};

// Samma åtta värden som Scoutnets "Medlemsstatus" på personnivå.
export const personStatusLabels: Record<string, Labelled> = {
	active: { label: 'Aktiv', tone: 'green' },
	new: { label: 'Ny – inte klar för fakturering', tone: 'accent' },
	unregistered: { label: 'Avregistrerad', tone: '' },
	auto_unregistered: { label: 'Automatiskt avregistrerad', tone: '' },
	deceased: { label: 'Avliden', tone: '' },
	active_non_member: { label: 'Aktiv icke-medlem', tone: 'blue' },
	anonymized: { label: 'Anonymiserad', tone: '' },
	locked: { label: 'Inaktiv – låst', tone: 'red' }
};

export const policeCheckLabels: Record<string, Labelled> = {
	missing: { label: 'Saknas', tone: 'red' },
	expired: { label: 'Utgånget', tone: 'red' },
	expiring: { label: 'Går ut snart', tone: 'accent' },
	valid: { label: 'Giltigt', tone: 'green' },
	not_required: { label: 'Krävs ej', tone: '' }
};

export const feeStatusLabels: Record<string, Labelled> = {
	paid: { label: 'Betald', tone: 'green' },
	unpaid: { label: 'Obetald', tone: 'accent' },
	overdue: { label: 'Förfallen', tone: 'red' }
};

export const invoiceDeliveryLabels: Record<string, string> = {
	email: 'E-post',
	alt_email: 'Alternativ e-post',
	post: 'Brev',
	efaktura: 'E-faktura',
	kivra: 'Kivra'
};

export const consentLabels: Record<string, string> = {
	photo_publication: 'Publicering av bilder',
	publish_profile: 'Publicera personlig information',
	publish_roles: 'Publicera roller'
};

export const decidedByLabels: Record<string, string> = {
	self: 'personen själv',
	guardian: 'vårdnadshavare',
	registrar: 'registrerare'
};

export const meritKinds: { key: string; label: string }[] = [
	{ key: 'award', label: 'Utmärkelser' },
	{ key: 'badge', label: 'Märken' },
	{ key: 'course', label: 'Utbildningar' },
	{ key: 'certification', label: 'Certifieringar' }
];

export const addressTypeLabels: Record<string, string> = {
	home: 'Hemadress',
	postal: 'Postadress',
	invoice: 'Fakturaadress',
	temporary: 'Tillfällig adress',
	other: 'Annan adress'
};

export const sexLabels: Record<string, string> = {
	female: 'Flicka/kvinna',
	male: 'Pojke/man',
	other: 'Annat',
	unknown: 'Okänt'
};

export const languageLabels: Record<string, string> = { sv: 'Svenska', en: 'Engelska' };

// Fältnamn i MemberPage.redacted_fields, så som de visas för användaren.
export const redactedLabels: Record<string, string> = {
	email: 'e-post',
	alt_email: 'e-post',
	mobile_phone: 'telefon',
	home_phone: 'telefon',
	national_id_masked: 'personnummer',
	police_check: 'registerutdrag',
	fee_group: 'avgifter',
	fee_status: 'avgifter'
};

export const fmtDate = (d?: string | null) => (d ? new Date(d).toLocaleDateString('sv-SE') : '–');

const sek = new Intl.NumberFormat('sv-SE', { style: 'currency', currency: 'SEK', maximumFractionDigits: 0 });
export const fmtSek = (ore: number) => sek.format(ore / 100);
