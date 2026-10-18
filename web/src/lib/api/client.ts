import type { components } from './schema';

export type Schemas = components['schemas'];
export type Me = Schemas['MeResponse'];
export type Context = Schemas['ContextDto'];
export type Body = Schemas['BodyDto'];
export type Crumb = Schemas['CrumbDto'];
export type BodyChild = Schemas['BodyChildDto'];
export type OrgNode = Schemas['OrgNodeDto'];
export type OrgNodePage = Schemas['OrgNodePage'];
export type Meeting = Schemas['MeetingDto'];
export type MeetingDetail = Schemas['MeetingDetail'];
export type RosterEntry = Schemas['RosterEntry'];
export type AttendanceMatrix = Schemas['AttendanceMatrix'];
export type AttendanceSummary = Schemas['TermAttendanceSummary'];
export type TroopAttendance = Schemas['TroopAttendanceSummary'];
export type CreateMeetings = Schemas['CreateMeetings'];
export type AttendanceStatus = 'present' | 'absent' | 'excused';
export type SavedView = Schemas['SavedView'];
export type GrantReport = Schemas['GrantReport'];
export type MemberRow = Schemas['MemberRow'];
export type MemberPage = Schemas['MemberPage'];
export type PersonDetail = Schemas['PersonDetail'];
export type Persona = Schemas['Persona'];
export type Permission = Schemas['Permission'];
export type Problem = Schemas['Problem'];
export type BulkRequest = Schemas['BulkRequest'];
export type BulkResult = Schemas['BulkResult'];
export type AuditLog = Schemas['AuditLog'];
export type AuditEntry = Schemas['AuditEntry'];
export type ApiKeyInfo = Schemas['ApiKeyInfo'];
export type CreatedApiKey = Schemas['CreatedApiKey'];
export type ModuleInfo = Schemas['ModuleInfo'];
export type BodyModule = Schemas['BodyModule'];
export type NavItem = Schemas['NavItem'];
export type Statistics = Schemas['Statistics'];

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
	attendanceSummary: (id: string, term?: string) =>
		request<AttendanceSummary>('GET', `/v1/bodies/${id}/attendance/summary${term ? `?term=${encodeURIComponent(term)}` : ''}`),
	attendance: (id: string, term?: string) =>
		request<AttendanceMatrix>('GET', `/v1/bodies/${id}/attendance${term ? `?term=${encodeURIComponent(term)}` : ''}`),
	createMeetings: (id: string, body: CreateMeetings) => request<Meeting[]>('POST', `/v1/bodies/${id}/meetings`, body),
	meeting: (id: string) => request<MeetingDetail>('GET', `/v1/meetings/${id}`),
	saveAttendance: (id: string, entries: { person_id: string; status: AttendanceStatus }[]) =>
		request<MeetingDetail>('PUT', `/v1/meetings/${id}/attendance`, { entries }),
	updateMeeting: (id: string, body: Schemas['UpdateMeeting']) => request<MeetingDetail>('PATCH', `/v1/meetings/${id}`, body),
	deleteMeeting: (id: string) => request<void>('DELETE', `/v1/meetings/${id}`),
	views: (id: string, list: string) => request<SavedView[]>('GET', `/v1/bodies/${id}/views?list=${list}`),
	createView: (id: string, body: Schemas['CreateView']) => request<SavedView>('POST', `/v1/bodies/${id}/views`, body),
	deleteView: (id: string) => request<void>('DELETE', `/v1/views/${id}`),
	grants: (id: string, term?: string) =>
		request<GrantReport>('GET', `/v1/bodies/${id}/grants${term ? `?term=${encodeURIComponent(term)}` : ''}`),
	createCalendar: (id: string) => request<{ url: string }>('POST', `/v1/bodies/${id}/calendar`),
	deleteCalendar: (id: string) => request<void>('DELETE', `/v1/bodies/${id}/calendar`),
	revealNationalId: (id: string) =>
		request<{ national_id: string }>('POST', `/v1/people/${id}/reveal-national-id`),
	personAudit: (id: string) => request<AuditLog>('GET', `/v1/people/${id}/audit`),
	bulk: (id: string, body: BulkRequest) => request<BulkResult>('POST', `/v1/bodies/${id}/bulk`, body),
	apiKeys: (id: string) => request<ApiKeyInfo[]>('GET', `/v1/bodies/${id}/api-keys`),
	createApiKey: (id: string, body: Schemas['CreateApiKey']) =>
		request<CreatedApiKey>('POST', `/v1/bodies/${id}/api-keys`, body),
	revokeApiKey: (id: string) => request<void>('DELETE', `/v1/api-keys/${id}`),
	modules: () => request<ModuleInfo[]>('GET', '/v1/modules'),
	bodyModules: (id: string) => request<BodyModule[]>('GET', `/v1/bodies/${id}/modules`),
	/** `enabled: null` tar bort enhetens eget val, så att valet ärvs igen. */
	setModule: (id: string, module: string, body: { enabled: boolean | null; locked?: boolean }) =>
		request<BodyModule[]>('PUT', `/v1/bodies/${id}/modules/${module}`, body),
	statistics: (id: string) => request<Statistics>('GET', `/v1/bodies/${id}/statistics`),
	personas: () => request<Persona[]>('GET', '/auth/dev/personas'),
	devLogin: (account_id: string) => request<void>('POST', '/auth/dev/login', { account_id }),
	logout: () => request<{ logout_url?: string | null } | undefined>('POST', '/auth/logout'),
	authConfig: () => request<Schemas['AuthConfig']>('GET', '/auth/config')
};

// Modulkatalogen ändras bara vid ny version, så den hämtas en gång per sidladdning.
let catalog: Promise<ModuleInfo[]> | null = null;
export function moduleCatalog(): Promise<ModuleInfo[]> {
	catalog ??= api.modules().catch((e) => {
		catalog = null;
		throw e;
	});
	return catalog;
}

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
	economy_read: 'Se avgifter och betalningar',
	modules_manage: 'Välja moduler'
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

const weekdayFmt = new Intl.DateTimeFormat('sv-SE', { weekday: 'long', day: 'numeric', month: 'long', timeZone: 'Europe/Stockholm' });
const shortFmt = new Intl.DateTimeFormat('sv-SE', { day: 'numeric', month: 'numeric', timeZone: 'Europe/Stockholm' });
const timeFmt = new Intl.DateTimeFormat('sv-SE', { hour: '2-digit', minute: '2-digit', timeZone: 'Europe/Stockholm' });
/** "torsdag 22 oktober" */
export const fmtWeekday = (d: string) => weekdayFmt.format(new Date(`${d}T12:00:00Z`));
/** "22/10" */
export const fmtShort = (d: string) => shortFmt.format(new Date(`${d}T12:00:00Z`));
/** Datum och tid från en tidsstämpel, i svensk tid: "tors 22 okt 18:30". */
export const fmtWhen = (ts: string) => {
	const d = new Date(ts);
	return `${new Intl.DateTimeFormat('sv-SE', { weekday: 'short', day: 'numeric', month: 'short', timeZone: 'Europe/Stockholm' }).format(d)} ${timeFmt.format(d)}`;
};
const dateTimeFmt = new Intl.DateTimeFormat('sv-SE', { dateStyle: 'short', timeStyle: 'short', timeZone: 'Europe/Stockholm' });
/** Tidpunkt med år, t.ex. "2026-10-17 21:54". */
export const fmtDateTime = (ts: string) => dateTimeFmt.format(new Date(ts));

export const meetingKindLabels: Record<string, string> = {
	meeting: 'Möte',
	hike: 'Hajk',
	camp: 'Läger',
	activity: 'Aktivitet'
};

export const attendanceLabels: Record<AttendanceStatus, Labelled & { icon: string }> = {
	present: { label: 'Närvarande', tone: 'green', icon: '✓' },
	absent: { label: 'Frånvarande', tone: 'red', icon: '✗' },
	excused: { label: 'Anmält förhinder', tone: 'accent', icon: 'F' }
};

/** Termin som visas och används i adresser: "HT2026". */
export const termKey = (label: string) => label.replace(/\s+/g, '');
export function shiftTerm(label: string, delta: number): string {
	const m = /^(VT|HT)\s*(\d{4})$/i.exec(label.trim());
	if (!m) return label;
	let half = m[1].toUpperCase() === 'HT' ? 1 : 0;
	let year = Number(m[2]);
	const idx = year * 2 + half + delta;
	year = Math.floor(idx / 2);
	half = idx % 2;
	return `${half ? 'HT' : 'VT'}${year}`;
}

const sek = new Intl.NumberFormat('sv-SE', { style: 'currency', currency: 'SEK', maximumFractionDigits: 0 });
export const fmtSek = (ore: number) => sek.format(ore / 100);
const sekExact = new Intl.NumberFormat('sv-SE', { style: 'currency', currency: 'SEK', minimumFractionDigits: 2 });
/** Belopp med ören, t.ex. "9,89 kr". */
export const fmtSekExact = (ore: number) => sekExact.format(ore / 100);
